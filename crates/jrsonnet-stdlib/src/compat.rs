use std::cmp::Ordering;

use jrsonnet_evaluator::{function::builtin, operator::evaluate_compare_op, Result, Val};

#[builtin]
#[allow(non_snake_case)]
pub fn builtin___compare(v1: Val, v2: Val) -> Result<i32> {
	Ok(
		match evaluate_compare_op(&v1, &v2, jrsonnet_parser::BinaryOpType::Lt)? {
			Ordering::Less => -1,
			Ordering::Equal => 0,
			Ordering::Greater => 1,
		},
	)
}
