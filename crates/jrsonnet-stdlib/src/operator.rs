//! Some jsonnet operations are desugared to stdlib functions...
//! However, in our case we instead implement them in native, and implement native functions on top of core for backwards compatibility

use jrsonnet_evaluator::{
	error::Result,
	function::builtin,
	operator::evaluate_mod_op,
	stdlib::std_format,
	typed::{Either, Either2},
	val::{equals, primitive_equals, StrValue},
	IStr, Val,
};

#[builtin]
pub fn builtin_mod(a: Either![f64, IStr], b: Val) -> Result<Val> {
	use Either2::*;
	evaluate_mod_op(
		&match a {
			A(v) => Val::Num(v),
			B(s) => Val::Str(StrValue::Flat(s)),
		},
		&b,
	)
}

#[builtin]
pub fn builtin_primitive_equals(x: Val, y: Val) -> Result<bool> {
	primitive_equals(&x, &y)
}

#[builtin]
pub fn builtin_equals(a: Val, b: Val) -> Result<bool> {
	equals(&a, &b)
}

#[builtin]
pub fn builtin_xor(x: bool, y: bool) -> bool {
	x ^ y
}

#[builtin]
pub fn builtin_xnor(x: bool, y: bool) -> bool {
	x == y
}

#[builtin]
pub fn builtin_format(str: IStr, vals: Val) -> Result<String> {
	std_format(&str, vals)
}
