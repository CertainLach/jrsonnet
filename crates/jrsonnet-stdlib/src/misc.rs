use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use jrsonnet_evaluator::{
	bail,
	error::{ErrorKind::*, Result},
	function::{builtin, ArgLike, CallLocation, FuncVal},
	manifest::JsonFormat,
	typed::{Either2, Either4},
	val::{equals, ArrValue},
	Context, Either, IStr, ObjValue, ObjValueBuilder, ResultExt, Thunk, Val,
};

use crate::{extvar_source, Settings};

#[builtin]
pub fn builtin_length(x: Either![IStr, ArrValue, ObjValue, FuncVal]) -> usize {
	use Either4::*;
	match x {
		A(x) => x.chars().count(),
		B(x) => x.len(),
		C(x) => x.len(),
		D(f) => f.params_len(),
	}
}

#[builtin]
pub fn builtin_get(
	o: ObjValue,
	f: IStr,
	default: Option<Thunk<Val>>,
	#[default(true)] inc_hidden: bool,
) -> Result<Val> {
	let do_default = move || {
		let Some(default) = default else {
			return Ok(Val::Null);
		};
		default.evaluate()
	};
	// Happy path for invisible fields
	if !inc_hidden && !o.has_field_ex(f.clone(), false) {
		return do_default();
	}
	let Some(v) = o.get(f)? else {
		return do_default();
	};
	Ok(v)
}

#[builtin(fields(
	settings: Rc<RefCell<Settings>>,
))]
pub fn builtin_ext_var(this: &builtin_ext_var, ctx: Context, x: IStr) -> Result<Val> {
	let ctx = ctx.state().create_default_context(extvar_source(&x, ""));
	this.settings
		.borrow()
		.ext_vars
		.get(&x)
		.cloned()
		.ok_or_else(|| UndefinedExternalVariable(x))?
		.evaluate_arg(ctx, true)?
		.evaluate()
}

#[builtin(fields(
	settings: Rc<RefCell<Settings>>,
))]
pub fn builtin_native(this: &builtin_native, x: IStr) -> Val {
	this.settings
		.borrow()
		.ext_natives
		.get(&x)
		.cloned()
		.map_or(Val::Null, Val::Func)
}

#[builtin(fields(
	settings: Rc<RefCell<Settings>>,
))]
pub fn builtin_trace(
	this: &builtin_trace,
	loc: CallLocation,
	str: Val,
	rest: Option<Thunk<Val>>,
) -> Result<Val> {
	this.settings.borrow().trace_printer.print_trace(
		loc,
		match &str {
			Val::Str(s) => s.clone().into_flat(),
			Val::Func(f) => format!("{f:?}").into(),
			v => v.manifest(JsonFormat::debug())?.into(),
		},
	);
	rest.map_or_else(|| Ok(str), |rest| rest.evaluate())
}

#[allow(clippy::comparison_chain)]
#[builtin]
pub fn builtin_starts_with(a: Either![IStr, ArrValue], b: Either![IStr, ArrValue]) -> Result<bool> {
	Ok(match (a, b) {
		(Either2::A(a), Either2::A(b)) => a.starts_with(b.as_str()),
		(Either2::B(a), Either2::B(b)) => {
			if b.len() > a.len() {
				return Ok(false);
			} else if b.len() == a.len() {
				return equals(&Val::Arr(a), &Val::Arr(b));
			}
			for (a, b) in a.iter().take(b.len()).zip(b.iter()) {
				let a = a?;
				let b = b?;
				if !equals(&a, &b)? {
					return Ok(false);
				}
			}
			true
		}
		_ => bail!("both arguments should be of the same type"),
	})
}

#[allow(clippy::comparison_chain)]
#[builtin]
pub fn builtin_ends_with(a: Either![IStr, ArrValue], b: Either![IStr, ArrValue]) -> Result<bool> {
	Ok(match (a, b) {
		(Either2::A(a), Either2::A(b)) => a.ends_with(b.as_str()),
		(Either2::B(a), Either2::B(b)) => {
			if b.len() > a.len() {
				return Ok(false);
			} else if b.len() == a.len() {
				return equals(&Val::Arr(a), &Val::Arr(b));
			}
			let a_len = a.len();
			for (a, b) in a.iter().skip(a_len - b.len()).zip(b.iter()) {
				let a = a?;
				let b = b?;
				if !equals(&a, &b)? {
					return Ok(false);
				}
			}
			true
		}
		_ => bail!("both arguments should be of the same type"),
	})
}

#[builtin]
pub fn builtin_assert_equal(a: Val, b: Val) -> Result<bool> {
	if equals(&a, &b)? {
		return Ok(true);
	}
	let format = JsonFormat::std_to_json("  ".to_owned(), "\n", ": ");
	let a = a.manifest(&format).description("<a> manifestification")?;
	let b = b.manifest(&format).description("<b> manifestification")?;
	bail!("assertion failed: A != B\nA: {a}\nB: {b}")
}

#[builtin]
pub fn builtin_merge_patch(target: Val, patch: Val) -> Result<Val> {
	let Some(patch) = patch.as_obj() else {
		return Ok(patch);
	};
	let Some(target) = target.as_obj() else {
		return Ok(Val::Obj(patch));
	};
	let target_fields = target.fields().into_iter().collect::<BTreeSet<IStr>>();
	let patch_fields = patch.fields().into_iter().collect::<BTreeSet<IStr>>();

	let mut out = ObjValueBuilder::new();
	for field in target_fields.union(&patch_fields) {
		let Some(field_patch) = patch.get(field.clone())? else {
			out.field(field.clone()).value(target.get(field.clone())?.expect("we're iterating over fields union, if field is missing in patch - it exists in target"));
			continue;
		};
		if matches!(field_patch, Val::Null) {
			continue;
		}
		let Some(field_target) = target.get(field.clone())? else {
			out.field(field.clone()).value(field_patch);
			continue;
		};
		out.field(field.clone())
			.value(builtin_merge_patch(field_target, field_patch)?);
	}
	Ok(out.build().into())
}
