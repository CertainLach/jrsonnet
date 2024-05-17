use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{BindSpec, Destruct, LocExpr, ParamsDesc};

use crate::{
	bail,
	error::{ErrorKind::*, Result},
	evaluate, evaluate_method, evaluate_named,
	gc::GcHashMap,
	val::ThunkValue,
	Context, Pending, Thunk, Val,
};

#[allow(clippy::too_many_lines)]
#[allow(unused_variables)]
pub fn destruct(
	d: &Destruct,
	parent: Thunk<Val>,
	fctx: Pending<Context>,
	new_bindings: &mut GcHashMap<IStr, Thunk<Val>>,
) -> Result<()> {
	match d {
		Destruct::Full(v) => {
			let old = new_bindings.insert(v.clone(), parent);
			if old.is_some() {
				bail!(DuplicateLocalVar(v.clone()))
			}
		}
		#[cfg(feature = "exp-destruct")]
		Destruct::Skip => {}
		#[cfg(feature = "exp-destruct")]
		Destruct::Array { start, rest, end } => {
			use jrsonnet_parser::DestructRest;

			use crate::arr::ArrValue;

			#[derive(Trace)]
			struct DataThunk {
				parent: Thunk<Val>,
				min_len: usize,
				has_rest: bool,
			}
			impl ThunkValue for DataThunk {
				type Output = ArrValue;

				fn get(self: Box<Self>) -> Result<Self::Output> {
					let v = self.parent.evaluate()?;
					let Val::Arr(arr) = v else {
						bail!("expected array");
					};
					if !self.has_rest {
						if arr.len() != self.min_len {
							bail!("expected {} elements, got {}", self.min_len, arr.len())
						}
					} else if arr.len() < self.min_len {
						bail!(
							"expected at least {} elements, but array was only {}",
							self.min_len,
							arr.len()
						)
					}
					Ok(arr)
				}
			}

			let full = Thunk::new(DataThunk {
				min_len: start.len() + end.len(),
				has_rest: rest.is_some(),
				parent,
			});

			{
				#[derive(Trace)]
				struct BaseThunk {
					full: Thunk<ArrValue>,
					index: usize,
				}
				impl ThunkValue for BaseThunk {
					type Output = Val;

					fn get(self: Box<Self>) -> Result<Self::Output> {
						let full = self.full.evaluate()?;
						Ok(full.get(self.index)?.expect("length is checked"))
					}
				}
				for (i, d) in start.iter().enumerate() {
					destruct(
						d,
						Thunk::new(BaseThunk {
							full: full.clone(),
							index: i,
						}),
						fctx.clone(),
						new_bindings,
					)?;
				}
			}

			match rest {
				Some(DestructRest::Keep(v)) => {
					#[derive(Trace)]
					struct RestThunk {
						full: Thunk<ArrValue>,
						start: usize,
						end: usize,
					}
					impl ThunkValue for RestThunk {
						type Output = Val;

						fn get(self: Box<Self>) -> Result<Self::Output> {
							let full = self.full.evaluate()?;
							let to = full.len() - self.end;
							Ok(Val::Arr(full.slice(
								Some(self.start as i32),
								Some(to as i32),
								None,
							)))
						}
					}

					destruct(
						&Destruct::Full(v.clone()),
						Thunk::new(RestThunk {
							full: full.clone(),
							start: start.len(),
							end: end.len(),
						}),
						fctx.clone(),
						new_bindings,
					)?;
				}
				Some(DestructRest::Drop) | None => {}
			}

			{
				#[derive(Trace)]
				struct EndThunk {
					full: Thunk<ArrValue>,
					index: usize,
					end: usize,
				}
				impl ThunkValue for EndThunk {
					type Output = Val;

					fn get(self: Box<Self>) -> Result<Self::Output> {
						let full = self.full.evaluate()?;
						Ok(full
							.get(full.len() - self.end + self.index)?
							.expect("length is checked"))
					}
				}
				for (i, d) in end.iter().enumerate() {
					destruct(
						d,
						Thunk::new(EndThunk {
							full: full.clone(),
							index: i,
							end: end.len(),
						}),
						fctx.clone(),
						new_bindings,
					)?;
				}
			}
		}
		#[cfg(feature = "exp-destruct")]
		Destruct::Object { fields, rest } => {
			use crate::obj::ObjValue;

			#[derive(Trace)]
			struct DataThunk {
				parent: Thunk<Val>,
				field_names: Vec<IStr>,
				has_rest: bool,
			}
			impl ThunkValue for DataThunk {
				type Output = ObjValue;

				fn get(self: Box<Self>) -> Result<Self::Output> {
					let v = self.parent.evaluate()?;
					let Val::Obj(obj) = v else {
						bail!("expected object");
					};
					for field in &self.field_names {
						if !obj.has_field_ex(field.clone(), true) {
							bail!("missing field: {field}");
						}
					}
					if !self.has_rest {
						let len = obj.len();
						if len != self.field_names.len() {
							bail!("too many fields, and rest not found");
						}
					}
					Ok(obj)
				}
			}
			let field_names: Vec<_> = fields
				.iter()
				.filter(|f| f.2.is_none())
				.map(|f| f.0.clone())
				.collect();
			let full = Thunk::new(DataThunk {
				parent,
				field_names,
				has_rest: rest.is_some(),
			});

			for (field, d, default) in fields {
				#[derive(Trace)]
				struct FieldThunk {
					full: Thunk<ObjValue>,
					field: IStr,
					default: Option<(Pending<Context>, LocExpr)>,
				}
				impl ThunkValue for FieldThunk {
					type Output = Val;

					fn get(self: Box<Self>) -> Result<Self::Output> {
						let full = self.full.evaluate()?;
						if let Some(field) = full.get(self.field)? {
							Ok(field)
						} else {
							let (fctx, expr) = self.default.as_ref().expect("shape is checked");
							Ok(evaluate(fctx.clone().unwrap(), expr)?)
						}
					}
				}
				let value = Thunk::new(FieldThunk {
					full: full.clone(),
					field: field.clone(),
					default: default.clone().map(|e| (fctx.clone(), e)),
				});
				if let Some(d) = d {
					destruct(d, value, fctx.clone(), new_bindings)?;
				} else {
					destruct(
						&Destruct::Full(field.clone()),
						value,
						fctx.clone(),
						new_bindings,
					)?;
				}
			}
		}
	}
	Ok(())
}

pub fn evaluate_dest(
	d: &BindSpec,
	fctx: Pending<Context>,
	new_bindings: &mut GcHashMap<IStr, Thunk<Val>>,
) -> Result<()> {
	match d {
		BindSpec::Field { into, value } => {
			#[derive(Trace)]
			struct EvaluateThunkValue {
				name: Option<IStr>,
				fctx: Pending<Context>,
				expr: LocExpr,
			}
			impl ThunkValue for EvaluateThunkValue {
				type Output = Val;
				fn get(self: Box<Self>) -> Result<Self::Output> {
					self.name.map_or_else(
						|| evaluate(self.fctx.unwrap(), &self.expr),
						|name| evaluate_named(self.fctx.unwrap(), &self.expr, name),
					)
				}
			}
			let data = Thunk::new(EvaluateThunkValue {
				name: into.name(),
				fctx: fctx.clone(),
				expr: value.clone(),
			});
			destruct(into, data, fctx, new_bindings)?;
		}
		BindSpec::Function {
			name,
			params,
			value,
		} => {
			#[derive(Trace)]
			struct MethodThunk {
				fctx: Pending<Context>,
				name: IStr,
				params: ParamsDesc,
				value: LocExpr,
			}
			impl ThunkValue for MethodThunk {
				type Output = Val;

				fn get(self: Box<Self>) -> Result<Self::Output> {
					Ok(evaluate_method(
						self.fctx.unwrap(),
						self.name,
						self.params,
						self.value,
					))
				}
			}

			let old = new_bindings.insert(
				name.clone(),
				Thunk::new(MethodThunk {
					fctx,
					name: name.clone(),
					params: params.clone(),
					value: value.clone(),
				}),
			);
			if old.is_some() {
				bail!(DuplicateLocalVar(name.clone()))
			}
		}
	}
	Ok(())
}
