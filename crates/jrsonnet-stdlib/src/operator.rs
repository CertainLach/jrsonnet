//! Some jsonnet operations are desugared to stdlib functions...
//! However, in our case we instead implement them in native, and implement native functions on top of core for backwards compatibility

use jrsonnet_evaluator::{
	error::Result,
	function::builtin,
	operator::evaluate_mod_op,
	stdlib::std_format,
	typed::{Any, Either, Either2},
	val::{equals, primitive_equals, StrValue},
	IStr, Val,
};

#[builtin]
pub fn builtin_mod(a: Either![f64, IStr], b: Any) -> Result<Any> {
	use Either2::*;
	Ok(Any(evaluate_mod_op(
		&match a {
			A(v) => Val::Num(v),
			B(s) => Val::Str(StrValue::Flat(s)),
		},
		&b.0,
	)?))
}

#[builtin]
pub fn builtin_primitive_equals(a: Any, b: Any) -> Result<bool> {
	primitive_equals(&a.0, &b.0)
}

#[builtin]
pub fn builtin_equals(a: Any, b: Any) -> Result<bool> {
	equals(&a.0, &b.0)
}

#[builtin]
pub fn builtin_format(str: IStr, vals: Any) -> Result<String> {
	std_format(&str, vals.0)
}
