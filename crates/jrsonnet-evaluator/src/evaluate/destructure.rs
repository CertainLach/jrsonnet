use jrsonnet_interner::IStr;
use jrsonnet_parser::{BindSpec, Destruct};

use crate::{
	bail,
	error::{ErrorKind::*, Result},
	evaluate, evaluate_method, evaluate_named,
	gc::GcHashMap,
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

			let min_len = start.len() + end.len();
			let has_rest = rest.is_some();
			let full = Thunk!(move || {
				let v = parent.evaluate()?;
				let Val::Arr(arr) = v else {
					bail!("expected array");
				};
				if !has_rest {
					if arr.len() != min_len {
						bail!("expected {} elements, got {}", min_len, arr.len())
					}
				} else if arr.len() < min_len {
					bail!(
						"expected at least {} elements, but array was only {}",
						min_len,
						arr.len()
					)
				}
				Ok(arr)
			});

			{
				for (i, d) in start.iter().enumerate() {
					let full = full.clone();
					destruct(
						d,
						Thunk!(move || Ok(full.evaluate()?.get(i)?.expect("length is checked"))),
						fctx.clone(),
						new_bindings,
					)?;
				}
			}

			match rest {
				Some(DestructRest::Keep(v)) => {
					let start = start.len();
					let end = end.len();
					let full = full.clone();
					destruct(
						&Destruct::Full(v.clone()),
						Thunk!(move || {
							let full = full.evaluate()?;
							let to = full.len() - end;
							Ok(Val::Arr(full.slice(
								Some(start as i32),
								Some(to as i32),
								None,
							)))
						}),
						fctx.clone(),
						new_bindings,
					)?;
				}
				Some(DestructRest::Drop) | None => {}
			}

			{
				for (i, d) in end.iter().enumerate() {
					let full = full.clone();
					let end = end.len();
					destruct(
						d,
						Thunk!(move || {
							let full = full.evaluate()?;
							Ok(full.get(full.len() - end + i)?.expect("length is checked"))
						}),
						fctx.clone(),
						new_bindings,
					)?;
				}
			}
		}
		#[cfg(feature = "exp-destruct")]
		Destruct::Object { fields, rest } => {
			let field_names: Vec<_> = fields
				.iter()
				.map(|f| (f.0.clone(), f.2.is_some()))
				.collect();
			let has_rest = rest.is_some();
			let full = Thunk!(move || {
				let v = parent.evaluate()?;
				let Val::Obj(obj) = v else {
					bail!("expected object");
				};
				for (field, has_default) in &field_names {
					if !has_default && !obj.has_field_ex(field.clone(), true) {
						bail!("missing field: {field}");
					}
				}
				if !has_rest {
					let len = obj.len();
					if len > field_names.len() {
						bail!("too many fields, and rest not found");
					}
				}
				Ok(obj)
			});

			for (field, d, default) in fields {
				let default = default.clone().map(|e| (fctx.clone(), e));
				let value = {
					let field = field.clone();
					let full = full.clone();
					Thunk!(move || {
						let full = full.evaluate()?;
						if let Some(field) = full.get(field)? {
							Ok(field)
						} else {
							let (fctx, expr) = default.as_ref().expect("shape is checked");
							Ok(evaluate(fctx.clone().unwrap(), expr)?)
						}
					})
				};

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
			let name = into.name();
			let value = value.clone();
			let data = {
				let fctx = fctx.clone();
				Thunk!(move || name.map_or_else(
					|| evaluate(fctx.unwrap(), &value),
					|name| evaluate_named(fctx.unwrap(), &value, name),
				))
			};
			destruct(into, data, fctx, new_bindings)?;
		}
		BindSpec::Function {
			name,
			params,
			value,
		} => {
			let params = params.clone();
			let name = name.clone();
			let value = value.clone();
			let old = new_bindings.insert(name.clone(), {
				let name = name.clone();
				Thunk!(move || Ok(evaluate_method(fctx.unwrap(), name, params, value)))
			});
			if old.is_some() {
				bail!(DuplicateLocalVar(name))
			}
		}
	}
	Ok(())
}
