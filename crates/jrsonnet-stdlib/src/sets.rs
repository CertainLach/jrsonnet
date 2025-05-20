use std::cmp::Ordering;

use jrsonnet_evaluator::{
	function::{builtin, CallLocation, FuncVal, PreparedFuncVal},
	operator::evaluate_compare_op,
	typed::{ComplexValType, FromUntyped, Typed, ValType},
	val::ArrValue,
	BindingValue, Error, Result, Thunk, Val,
};
use jrsonnet_parser::BinaryOpType;

#[derive(Debug)]
pub enum KeyF {
	Identity,
	Prepared(PreparedFuncVal),
	PrepareFailure(Error),
}
impl KeyF {
	pub fn is_identity(&self) -> bool {
		matches!(self, Self::Identity)
	}
	fn new(val: FuncVal) -> Self {
		if val.is_identity() {
			Self::Identity
		} else {
			PreparedFuncVal::new(val, 1, &[]).map_or_else(Self::PrepareFailure, Self::Prepared)
		}
	}
	pub fn eval(&self, val: impl Into<BindingValue>) -> Result<Val> {
		match self {
			KeyF::Identity => val.into().evaluate(),
			KeyF::Prepared(p) => p.call(CallLocation::native(), &[val.into()], &[]),
			KeyF::PrepareFailure(e) => Err(e.clone()),
		}
	}
}
impl Typed for KeyF {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Func);
}
impl FromUntyped for KeyF {
	fn from_untyped(untyped: Val) -> Result<Self> {
		FuncVal::from_untyped(untyped).map(Self::new)
	}
}

#[builtin]
#[allow(non_snake_case)]
pub fn builtin_set_member(
	x: Thunk<Val>,
	arr: ArrValue,
	#[default(KeyF::Identity)] keyF: KeyF,
) -> Result<bool> {
	let mut low = 0;
	let mut high = arr.len();

	let x = keyF.eval(x)?;

	while low < high {
		let middle = (high + low) / 2;
		let comp = keyF.eval(arr.get_lazy(middle).expect("in bounds"))?;
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
pub fn builtin_set_inter(
	a: ArrValue,
	b: ArrValue,
	#[default(KeyF::Identity)] keyF: KeyF,
) -> Result<ArrValue> {
	let mut a = a.iter_lazy();
	let mut b = b.iter_lazy();

	let keyF = |v| keyF.eval(v);

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
	Ok(ArrValue::new(out))
}

#[builtin]
#[allow(non_snake_case, clippy::redundant_closure)]
pub fn builtin_set_diff(
	a: ArrValue,
	b: ArrValue,
	#[default(KeyF::Identity)] keyF: KeyF,
) -> Result<ArrValue> {
	let mut a = a.iter_lazy();
	let mut b = b.iter_lazy();

	let keyF = |v| keyF.eval(v);

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
	Ok(ArrValue::new(out))
}

#[builtin]
#[allow(non_snake_case, clippy::redundant_closure)]
pub fn builtin_set_union(
	a: ArrValue,
	b: ArrValue,
	#[default(KeyF::Identity)] keyF: KeyF,
) -> Result<ArrValue> {
	let mut a = a.iter_lazy();
	let mut b = b.iter_lazy();

	let keyF = |v| keyF.eval(v);

	let mut av = a.next();
	let mut bv = b.next();
	let mut ak = av.clone().map(keyF).transpose()?;
	let mut bk = bv.clone().map(keyF).transpose()?;

	let mut out = Vec::new();
	while let (Some(ac), Some(bc)) = (&ak, &bk) {
		match evaluate_compare_op(ac, bc, BinaryOpType::Lt)? {
			Ordering::Less => {
				out.push(av.clone().expect("ak != None"));
				av = a.next();
				ak = av.clone().map(keyF).transpose()?;
			}
			Ordering::Greater => {
				out.push(bv.clone().expect("bk != None"));
				bv = b.next();
				bk = bv.clone().map(keyF).transpose()?;
			}
			Ordering::Equal => {
				// NOTE: order matters, values in `a` win
				out.push(av.clone().expect("ak != None"));
				av = a.next();
				ak = av.clone().map(keyF).transpose()?;
				bv = b.next();
				bk = bv.clone().map(keyF).transpose()?;
			}
		};
	}
	// a.len() > b.len()
	while let Some(_ac) = &ak {
		out.push(av.clone().expect("ak != None"));
		av = a.next();
		ak = av.clone().map(keyF).transpose()?;
	}
	// b.len() > a.len()
	while let Some(_bc) = &bk {
		out.push(bv.clone().expect("ak != None"));
		bv = b.next();
		bk = bv.clone().map(keyF).transpose()?;
	}
	Ok(ArrValue::new(out))
}
