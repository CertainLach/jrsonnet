use gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{BindSpec, Destruct, LocExpr, ParamsDesc};

use crate::{
	error::{Error::*, Result},
	evaluate, evaluate_method, evaluate_named,
	gc::GcHashMap,
	tb, throw,
	val::ThunkValue,
	Context, Pending, State, Thunk, Val,
};

#[allow(clippy::too_many_lines)]
fn destruct(
	d: &Destruct,
	parent: Thunk<Val>,
	new_bindings: &mut GcHashMap<IStr, Thunk<Val>>,
) -> Result<()> {
	match d {
		Destruct::Full(v) => {
			let old = new_bindings.insert(v.clone(), parent);
			if old.is_some() {
				throw!(DuplicateLocalVar(v.clone()))
			}
		}
		#[cfg(feature = "exp-destruct")]
		Destruct::Skip => {}
		#[cfg(feature = "exp-destruct")]
		Destruct::Array { start, rest, end } => {
			use jrsonnet_parser::DestructRest;

			use crate::{throw_runtime, val::ArrValue};

			#[derive(Trace)]
			struct DataThunk {
				parent: Thunk<Val>,
				min_len: usize,
				has_rest: bool,
			}
			impl ThunkValue for DataThunk {
				type Output = ArrValue;

				fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
					let v = self.parent.evaluate(s)?;
					let arr = match v {
						Val::Arr(a) => a,
						_ => throw_runtime!("expected array"),
					};
					if !self.has_rest {
						if arr.len() != self.min_len {
							throw_runtime!("expected {} elements, got {}", self.min_len, arr.len())
						}
					} else if arr.len() < self.min_len {
						throw_runtime!(
							"expected at least {} elements, but array was only {}",
							self.min_len,
							arr.len()
						)
					}
					Ok(arr)
				}
			}

			let full = Thunk::new(tb!(DataThunk {
				min_len: start.len() + end.len(),
				has_rest: rest.is_some(),
				parent,
			}));

			{
				#[derive(Trace)]
				struct BaseThunk {
					full: Thunk<ArrValue>,
					index: usize,
				}
				impl ThunkValue for BaseThunk {
					type Output = Val;

					fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
						let full = self.full.evaluate(s.clone())?;
						Ok(full.get(s, self.index)?.expect("length is checked"))
					}
				}
				for (i, d) in start.iter().enumerate() {
					destruct(
						d,
						Thunk::new(tb!(BaseThunk {
							full: full.clone(),
							index: i,
						})),
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

						fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
							let full = self.full.evaluate(s)?;
							let to = full.len() - self.end;
							Ok(Val::Arr(full.slice(Some(self.start), Some(to), None)))
						}
					}

					destruct(
						&Destruct::Full(v.clone()),
						Thunk::new(tb!(RestThunk {
							full: full.clone(),
							start: start.len(),
							end: end.len(),
						})),
						new_bindings,
					)?;
				}
				Some(DestructRest::Drop) => {}
				None => {}
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

					fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
						let full = self.full.evaluate(s.clone())?;
						Ok(full
							.get(s, full.len() - self.end + self.index)?
							.expect("length is checked"))
					}
				}
				for (i, d) in end.iter().enumerate() {
					destruct(
						d,
						Thunk::new(tb!(EndThunk {
							full: full.clone(),
							index: i,
							end: end.len(),
						})),
						new_bindings,
					)?;
				}
			}
		}
		#[cfg(feature = "exp-destruct")]
		Destruct::Object { fields, rest } => {
			use crate::{obj::ObjValue, throw_runtime};

			#[derive(Trace)]
			struct DataThunk {
				parent: Thunk<Val>,
				field_names: Vec<IStr>,
				has_rest: bool,
			}
			impl ThunkValue for DataThunk {
				type Output = ObjValue;

				fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
					let v = self.parent.evaluate(s)?;
					let obj = match v {
						Val::Obj(o) => o,
						_ => throw_runtime!("expected object"),
					};
					for field in &self.field_names {
						if !obj.has_field_ex(field.clone(), true) {
							throw_runtime!("missing field: {}", field);
						}
					}
					if !self.has_rest {
						let len = obj.len();
						if len != self.field_names.len() {
							throw_runtime!("too many fields, and rest not found");
						}
					}
					Ok(obj)
				}
			}
			let field_names: Vec<_> = fields.iter().map(|f| f.0.clone()).collect();
			let full = Thunk::new(tb!(DataThunk {
				parent,
				field_names: field_names.clone(),
				has_rest: rest.is_some()
			}));

			for (field, d) in fields {
				#[derive(Trace)]
				struct FieldThunk {
					full: Thunk<ObjValue>,
					field: IStr,
				}
				impl ThunkValue for FieldThunk {
					type Output = Val;

					fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
						let full = self.full.evaluate(s.clone())?;
						let field = full.get(s, self.field)?.expect("shape is checked");
						Ok(field)
					}
				}
				let value = Thunk::new(tb!(FieldThunk {
					full: full.clone(),
					field: field.clone()
				}));
				if let Some(d) = d {
					destruct(d, value, new_bindings)?;
				} else {
					destruct(&Destruct::Full(field.clone()), value, new_bindings)?;
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
				fn get(self: Box<Self>, s: State) -> Result<Self::Output> {
					if let Some(name) = self.name {
						evaluate_named(s, self.fctx.unwrap(), &self.expr, name)
					} else {
						evaluate(s, self.fctx.unwrap(), &self.expr)
					}
				}
			}
			let data = Thunk::new(tb!(EvaluateThunkValue {
				name: into.name(),
				fctx,
				expr: value.clone(),
			}));
			destruct(into, data, new_bindings)?;
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

				fn get(self: Box<Self>, _s: State) -> Result<Self::Output> {
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
				Thunk::new(tb!(MethodThunk {
					fctx,
					name: name.clone(),
					params: params.clone(),
					value: value.clone()
				})),
			);
			if old.is_some() {
				throw!(DuplicateLocalVar(name.clone()))
			}
		}
	}
	Ok(())
}
