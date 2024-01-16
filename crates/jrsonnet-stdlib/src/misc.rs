use std::{cell::RefCell, rc::Rc};

use boa_gc::{Gc, GcRefCell};
use jrsonnet_evaluator::{
	bail,
	error::{ErrorKind::*, Result},
	function::{builtin, ArgLike, CallLocation, FuncVal},
	manifest::JsonFormat,
	typed::{Either2, Either4},
	val::{equals, ArrValue},
	Context, Either, IStr, ObjValue, Thunk, Val,
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

#[builtin(fields(
	settings: Gc<GcRefCell<Settings>>,
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
	settings: Gc<GcRefCell<Settings>>,
))]
pub fn builtin_native(this: &builtin_native, x: IStr) -> Val {
	this.settings
		.borrow()
		.ext_natives
		.get(&x)
		.cloned()
		.map_or(Val::Null, |v| Val::Func(v))
}

#[builtin(fields(
	settings: Gc<GcRefCell<Settings>>,
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
	if let Some(rest) = rest {
		rest.evaluate()
	} else {
		Ok(str)
	}
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
			} else {
				for (a, b) in a.iter().take(b.len()).zip(b.iter()) {
					let a = a?;
					let b = b?;
					if !equals(&a, &b)? {
						return Ok(false);
					}
				}
				true
			}
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
			} else {
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
		}
		_ => bail!("both arguments should be of the same type"),
	})
}
