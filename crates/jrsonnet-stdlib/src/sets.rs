use std::cmp::Ordering;

use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, FuncVal},
	operator::evaluate_compare_op,
	val::ArrValue,
	Val,
};
use jrsonnet_parser::BinaryOpType;

#[builtin]
#[allow(non_snake_case)]
pub fn builtin_set_member(x: Val, arr: ArrValue, keyF: Option<FuncVal>) -> Result<bool> {
	let mut low = 0;
	let mut high = arr.len();

	let keyF = keyF.unwrap_or(FuncVal::Id).into_native::<((Val,), Val)>();

	let x = keyF(x)?;

	while low < high {
		let middle = (high + low) / 2;
		match evaluate_compare_op(&arr.get(middle)?.expect("in bounds"), &x, BinaryOpType::Lt)? {
			Ordering::Less => low = middle + 1,
			Ordering::Equal => return Ok(true),
			Ordering::Greater => high = middle,
		}
	}
	Ok(false)
}
