use std::cmp::Ordering;

use jrsonnet_evaluator::{
	function::builtin, operator::evaluate_compare_op, val::ArrValue, Result, Val,
};

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

#[builtin]
#[allow(non_snake_case)]
pub fn builtin___compare_array(arr1: ArrValue, arr2: ArrValue) -> Result<i32> {
	builtin___compare(Val::Arr(arr1), Val::Arr(arr2))
}

macro_rules! arr_comp {
	($name:ident, $operator:expr) => {
		#[builtin]
		#[allow(non_snake_case)]
		pub fn $name(arr1: ArrValue, arr2: ArrValue) -> Result<bool> {
			let ordering = evaluate_compare_op(
				&Val::Arr(arr1),
				&Val::Arr(arr2),
				jrsonnet_parser::BinaryOpType::Lt,
			)?;
			Ok($operator.contains(&ordering))
		}
	};
}
arr_comp!(builtin___array_less, [Ordering::Less]);
arr_comp!(builtin___array_greater, [Ordering::Greater]);
arr_comp!(
	builtin___array_less_or_equal,
	[Ordering::Less, Ordering::Equal]
);
arr_comp!(
	builtin___array_greater_or_equal,
	[Ordering::Greater, Ordering::Equal]
);
