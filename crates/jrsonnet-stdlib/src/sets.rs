use std::cmp::Ordering;

use jrsonnet_evaluator::{
	function::{builtin, FuncVal},
	operator::evaluate_compare_op,
	val::ArrValue,
	Result, Thunk, Val,
};
use jrsonnet_parser::BinaryOpType;

#[builtin]
#[allow(non_snake_case)]
pub fn builtin_set_member(x: Thunk<Val>, arr: ArrValue, keyF: Option<FuncVal>) -> Result<bool> {
	let mut low = 0;
	let mut high = arr.len();

	let keyF = keyF
		.unwrap_or(FuncVal::Id)
		.into_native::<((Thunk<Val>,), Val)>();

	let x = keyF(x)?;

	while low < high {
		let middle = (high + low) / 2;
		let comp = keyF(arr.get_lazy(middle).expect("in bounds"))?;
		match evaluate_compare_op(&comp, &x, BinaryOpType::Lt)? {
			Ordering::Less => low = middle + 1,
			Ordering::Equal => return Ok(true),
			Ordering::Greater => high = middle,
		}
	}
	Ok(false)
}

#[builtin]
#[allow(non_snake_case, clippy::redundant_closure)]
pub fn builtin_set_inter(a: ArrValue, b: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	let mut a = a.iter_lazy();
	let mut b = b.iter_lazy();

	let keyF = keyF
		.unwrap_or(FuncVal::identity())
		.into_native::<((Thunk<Val>,), Val)>();
	let keyF = |v| keyF(v);

	let mut av = a.next();
	let mut bv = b.next();
	let mut ak = av.clone().map(keyF).transpose()?;
	let mut bk = bv.map(keyF).transpose()?;

	let mut out = Vec::new();
	while let (Some(ac), Some(bc)) = (&ak, &bk) {
		match evaluate_compare_op(ac, bc, BinaryOpType::Lt)? {
			Ordering::Less => {
				av = a.next();
				ak = av.clone().map(keyF).transpose()?;
			}
			Ordering::Greater => {
				bv = b.next();
				bk = bv.map(keyF).transpose()?;
			}
			Ordering::Equal => {
				out.push(av.clone().expect("ak != None => av != None"));
				av = a.next();
				ak = av.clone().map(keyF).transpose()?;
				bv = b.next();
				bk = bv.map(keyF).transpose()?;
			}
		};
	}
	Ok(ArrValue::lazy(out))
}
#[builtin]
#[allow(non_snake_case, clippy::redundant_closure)]
pub fn builtin_set_diff(a: ArrValue, b: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	let mut a = a.iter_lazy();
	let mut b = b.iter_lazy();

	let keyF = keyF
		.unwrap_or(FuncVal::identity())
		.into_native::<((Thunk<Val>,), Val)>();
	let keyF = |v| keyF(v);

	let mut av = a.next();
	let mut bv = b.next();
	let mut ak = av.clone().map(keyF).transpose()?;
	let mut bk = bv.map(keyF).transpose()?;

	let mut out = Vec::new();
	while let (Some(ac), Some(bc)) = (&ak, &bk) {
		match evaluate_compare_op(ac, bc, BinaryOpType::Lt)? {
			Ordering::Less => {
				// In a, but not in b
				out.push(av.clone().expect("ak != None"));
				av = a.next();
				ak = av.clone().map(keyF).transpose()?;
			}
			Ordering::Greater => {
				bv = b.next();
				bk = bv.map(keyF).transpose()?;
			}
			Ordering::Equal => {
				av = a.next();
				ak = av.clone().map(keyF).transpose()?;
				bv = b.next();
				bk = bv.map(keyF).transpose()?;
			}
		};
	}
	while let Some(_ac) = &ak {
		// In a, but not in b
		out.push(av.clone().expect("ak != None"));
		av = a.next();
		ak = av.clone().map(keyF).transpose()?;
	}
	Ok(ArrValue::lazy(out))
}
