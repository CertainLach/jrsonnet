use std::{cell::RefCell, path::Path};

use async_trait::async_trait;
use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BindSpec, CompSpec, Destruct, Expr, FieldMember, FieldName, ForSpecData,
	IfSpecData, LocExpr, Member, ObjBody, Param, ParamsDesc, ParserSettings, SliceDesc, Source,
	SourcePath,
};

use crate::{bail, gc::GcHashMap, FileData, ImportResolver, State};

pub struct Import {
	path: IStr,
	expression: bool,
}

pub struct FoundImports(Vec<Import>);

// Visits all nodes, trying to find import statements
#[allow(clippy::too_many_lines)]
pub fn find_imports(expr: &LocExpr, out: &mut FoundImports) {
	fn in_destruct(dest: &Destruct, #[allow(unused_variables)] out: &mut FoundImports) {
		match dest {
			#[cfg(feature = "exp-destruct")]
			Destruct::Array {
				start,
				rest: _,
				end,
			} => {
				for dest in start {
					in_destruct(dest, out);
				}
				for dest in end {
					in_destruct(dest, out);
				}
			}
			#[cfg(feature = "exp-destruct")]
			Destruct::Object { fields, rest: _ } => {
				for (_, dest, default) in fields {
					if let Some(dest) = dest {
						in_destruct(dest, out);
					}
					if let Some(expr) = default {
						find_imports(expr, out);
					}
				}
			}
			#[cfg(feature = "exp-destruct")]
			Destruct::Skip => {}
			Destruct::Full(_) => {}
		}
	}
	fn in_compspec(specs: &[CompSpec], out: &mut FoundImports) {
		for spec in specs {
			match spec {
				CompSpec::IfSpec(IfSpecData(expr)) => find_imports(expr, out),
				CompSpec::ForSpec(ForSpecData(destruct, expr)) => {
					in_destruct(destruct, out);
					find_imports(expr, out);
				}
			}
		}
	}
	fn in_params(params: &ParamsDesc, out: &mut FoundImports) {
		for Param(dest, default) in &*params.0 {
			in_destruct(dest, out);
			if let Some(expr) = default {
				find_imports(expr, out);
			}
		}
	}
	fn in_bind(specs: &[BindSpec], out: &mut FoundImports) {
		for spec in specs {
			match spec {
				BindSpec::Field {
					into: dest,
					value: expr,
				} => {
					in_destruct(dest, out);
					find_imports(expr, out);
				}
				BindSpec::Function {
					name: _,
					params,
					value: expr,
				} => {
					in_params(params, out);
					find_imports(expr, out);
				}
			}
		}
	}
	fn in_args(ArgsDesc { unnamed, named }: &ArgsDesc, out: &mut FoundImports) {
		for expr in unnamed {
			find_imports(expr, out);
		}
		for (_, expr) in named {
			find_imports(expr, out);
		}
	}
	fn in_obj(obj: &ObjBody, out: &mut FoundImports) {
		match obj {
			ObjBody::MemberList(v) => {
				for member in v {
					match member {
						Member::Field(FieldMember {
							name,
							params,
							value,
							..
						}) => {
							match name {
								FieldName::Fixed(_) => {}
								FieldName::Dyn(expr) => find_imports(expr, out),
							}
							if let Some(params) = params {
								in_params(params, out);
							}
							find_imports(value, out);
						}
						Member::BindStmt(_) => todo!(),
						Member::AssertStmt(AssertStmt(expr, expr2)) => {
							find_imports(expr, out);
							if let Some(expr) = expr2 {
								find_imports(expr, out);
							}
						}
					}
				}
			}
			ObjBody::ObjComp(_) => todo!(),
		}
	}
	match &*expr.0 {
		Expr::Import(v) | Expr::ImportStr(v) | Expr::ImportBin(v) => {
			if let Expr::Str(s) = &*v.0 {
				out.0.push(Import {
					path: s.clone(),
					expression: matches!(&*expr.0, Expr::Import(_)),
				});
			}
			// Non-string import will fail in runtime
		}

		Expr::Literal(_) | Expr::Str(_) | Expr::Num(_) | Expr::Var(_) => {}

		Expr::Arr(arr) => {
			for expr in arr {
				find_imports(expr, out);
			}
		}
		Expr::ArrComp(expr, specs) => {
			find_imports(expr, out);
			in_compspec(specs, out);
		}
		Expr::Obj(obj) => in_obj(obj, out),
		Expr::ObjExtend(expr, obj) => {
			find_imports(expr, out);
			in_obj(obj, out);
		}
		Expr::BinaryOp(a, _, b) => {
			find_imports(a, out);
			find_imports(b, out);
		}
		Expr::AssertExpr(AssertStmt(expr, expr2), then) => {
			find_imports(expr, out);
			if let Some(expr) = expr2 {
				find_imports(expr, out);
			}
			find_imports(then, out);
		}
		Expr::LocalExpr(specs, expr) => {
			in_bind(specs, out);
			find_imports(expr, out);
		}
		Expr::Apply(expr, args, _) => {
			find_imports(expr, out);
			in_args(args, out);
		}
		Expr::Index { indexable, parts } => {
			find_imports(indexable, out);
			for part in parts {
				find_imports(&part.value, out);
			}
		}
		Expr::Function(params, expr) => {
			in_params(params, out);
			find_imports(expr, out);
		}
		Expr::IfElse {
			cond: IfSpecData(expr),
			cond_then,
			cond_else,
		} => {
			find_imports(expr, out);
			find_imports(cond_then, out);
			if let Some(expr) = cond_else {
				find_imports(expr, out);
			}
		}
		Expr::Slice(expr, SliceDesc { start, end, step }) => {
			find_imports(expr, out);
			if let Some(expr) = start {
				find_imports(expr, out);
			}
			if let Some(expr) = end {
				find_imports(expr, out);
			}
			if let Some(expr) = step {
				find_imports(expr, out);
			}
		}
		Expr::Parened(expr) | Expr::UnaryOp(_, expr) | Expr::ErrorStmt(expr) => {
			find_imports(expr, out);
		}
	}
}

#[async_trait(?Send)]
pub trait AsyncImportResolver {
	type Error;
	/// Resolves file path, e.g. `(/home/user/manifests, b.libjsonnet)` can correspond
	/// both to `/home/user/manifests/b.libjsonnet` and to `/home/user/${vendor}/b.libjsonnet`
	/// where `${vendor}` is a library path.
	///
	/// `from` should only be returned from [`ImportResolver::resolve`], or from other defined file, any other value
	/// may result in panic
	async fn resolve_from(&self, from: &SourcePath, path: &str) -> Result<SourcePath, Self::Error>;
	async fn resolve_from_default(&self, path: &str) -> Result<SourcePath, Self::Error> {
		self.resolve_from(&SourcePath::default(), path).await
	}
	/// Resolves absolute path, doesn't supports jpath and other fancy things
	async fn resolve(&self, path: &Path) -> Result<SourcePath, Self::Error>;

	/// Load resolved file
	/// This should only be called with value returned from [`ImportResolver::resolve_file`]/[`ImportResolver::resolve`],
	/// this cannot be resolved using associated type, as evaluator uses object instead of generic for [`ImportResolver`]
	async fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>, Self::Error>;
}

#[derive(Trace)]
struct ResolvedImportResolver {
	resolved: RefCell<GcHashMap<(SourcePath, IStr), (SourcePath, bool)>>,
}
impl ImportResolver for ResolvedImportResolver {
	fn load_file_contents(&self, _resolved: &SourcePath) -> crate::Result<Vec<u8>> {
		unreachable!("all files should be loaded at this point");
	}

	fn resolve_from(&self, from: &SourcePath, path: &str) -> crate::Result<SourcePath> {
		Ok(self
			.resolved
			.borrow()
			.get(&(from.clone(), path.into()))
			.expect("all imports should be resolved at this point")
			.0
			.clone())
	}

	fn resolve_from_default(&self, path: &str) -> crate::Result<SourcePath> {
		self.resolve_from(&SourcePath::default(), path)
	}

	fn resolve(&self, path: &Path) -> crate::Result<SourcePath> {
		bail!(crate::error::ErrorKind::AbsoluteImportNotSupported(
			path.to_owned()
		))
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

enum Job {
	LoadFile { path: SourcePath, parse: bool },
	ParseFile(SourcePath),
	ResolveImport { from: SourcePath, import: Import },
}

#[allow(clippy::future_not_send)]
pub async fn async_import<H>(s: State, handler: H, path: impl AsRef<Path>) -> Result<(), H::Error>
where
	H: AsyncImportResolver,
{
	let mut resolved = s
		.import_resolver()
		.as_any()
		.downcast_ref::<ResolvedImportResolver>()
		.map_or_else(GcHashMap::new, |resolver| {
			std::mem::take(&mut *resolver.resolved.borrow_mut())
		});
	let mut queue = vec![Job::LoadFile {
		path: handler.resolve(path.as_ref()).await?,
		parse: true,
	}];
	while let Some(job) = queue.pop() {
		match job {
			Job::LoadFile { path, parse } => {
				if !s.0.file_cache.borrow().contains_key(&path) {
					let data = handler.load_file_contents(&path).await?;
					s.0.file_cache
						.borrow_mut()
						.insert(path.clone(), FileData::new_bytes(data.as_slice().into()));
				}
				if parse {
					queue.push(Job::ParseFile(path));
				}
			}
			Job::ParseFile(path) => {
				if let Some(file) = s.0.file_cache.borrow_mut().get_mut(&path) {
					if file.parsed.is_none() {
						let Some(code) = file.get_string() else {
							continue;
						};
						let source = Source::new(path.clone(), code.clone());
						// If failed - then skip import
						file.parsed =
							jrsonnet_parser::parse(&code, &ParserSettings { source }).ok();
						if let Some(parsed) = &file.parsed {
							let mut imports = FoundImports(vec![]);
							find_imports(parsed, &mut imports);
							for import in imports.0 {
								queue.push(Job::ResolveImport {
									from: path.clone(),
									import,
								});
							}
						}
					}
				}
			}
			Job::ResolveImport { from, import } => {
				if let Some((resolved, expression)) =
					resolved.get_mut(&(from.clone(), import.path.clone()))
				{
					if import.expression && !*expression {
						*expression = true;
						queue.push(Job::ParseFile(resolved.clone()));
					}
					continue;
				}
				let resolved = handler.resolve_from(&from, &import.path).await?;
				queue.push(Job::LoadFile {
					path: resolved,
					parse: import.expression,
				});
			}
		}
	}
	s.set_import_resolver(ResolvedImportResolver {
		resolved: RefCell::new(resolved),
	});
	Ok(())
}
