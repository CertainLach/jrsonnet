use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, FuncVal},
	throw,
	typed::{Any, BoundedUsize, Either2, NativeFn, Typed, VecVal},
	val::{equals, ArrValue, IndexableVal},
	Either, IStr, Val,
};
use jrsonnet_gcmodule::Cc;

#[builtin]
pub fn builtin_make_array(sz: usize, func: NativeFn<((f64,), Any)>) -> Result<VecVal> {
	let mut out = Vec::with_capacity(sz);
	for i in 0..sz {
		out.push(func(i as f64)?.0);
	}
	Ok(VecVal(Cc::new(out)))
}

#[builtin]
pub fn builtin_slice(
	indexable: IndexableVal,
	index: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	end: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
) -> Result<Any> {
	indexable.slice(index, end, step).map(Val::from).map(Any)
}

#[builtin]
pub fn builtin_map(func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	Ok(arr.map(func))
}

#[builtin]
pub fn builtin_flatmap(
	func: NativeFn<((Either![String, Any],), Any)>,
	arr: IndexableVal,
) -> Result<IndexableVal> {
	use std::fmt::Write;
	match arr {
		IndexableVal::Str(str) => {
			let mut out = String::new();
			for c in str.chars() {
				match func(Either2::A(c.to_string()))?.0 {
					Val::Str(o) => write!(out, "{o}").unwrap(),
					Val::Null => continue,
					_ => throw!("in std.join all items should be strings"),
				};
			}
			Ok(IndexableVal::Str(out.into()))
		}
		IndexableVal::Arr(a) => {
			let mut out = Vec::new();
			for el in a.iter() {
				let el = el?;
				match func(Either2::B(Any(el)))?.0 {
					Val::Arr(o) => {
						for oe in o.iter() {
							out.push(oe?);
						}
					}
					Val::Null => continue,
					_ => throw!("in std.join all items should be arrays"),
				};
			}
			Ok(IndexableVal::Arr(out.into()))
		}
	}
}

#[builtin]
pub fn builtin_filter(func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.filter(|val| bool::from_untyped(func.evaluate_simple(&(Any(val.clone()),))?))
}

#[builtin]
pub fn builtin_foldl(func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter() {
		acc = func.evaluate_simple(&(Any(acc), Any(i?)))?;
	}
	Ok(Any(acc))
}

#[builtin]
pub fn builtin_foldr(func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter().rev() {
		acc = func.evaluate_simple(&(Any(i?), Any(acc)))?;
	}
	Ok(Any(acc))
}

#[builtin]
pub fn builtin_range(from: i32, to: i32) -> Result<ArrValue> {
	if to < from {
		return Ok(ArrValue::empty());
	}
	Ok(ArrValue::range_inclusive(from, to))
}

#[builtin]
pub fn builtin_join(sep: IndexableVal, arr: ArrValue) -> Result<IndexableVal> {
	use std::fmt::Write;
	Ok(match sep {
		IndexableVal::Arr(joiner_items) => {
			let mut out = Vec::new();

			let mut first = true;
			for item in arr.iter() {
				let item = item?.clone();
				if let Val::Arr(items) = item {
					if !first {
						out.reserve(joiner_items.len());
						// TODO: extend
						for item in joiner_items.iter() {
							out.push(item?);
						}
					}
					first = false;
					out.reserve(items.len());
					for item in items.iter() {
						out.push(item?);
					}
				} else if matches!(item, Val::Null) {
					continue;
				} else {
					throw!("in std.join all items should be arrays");
				}
			}

			IndexableVal::Arr(out.into())
		}
		IndexableVal::Str(sep) => {
			let mut out = String::new();

			let mut first = true;
			for item in arr.iter() {
				let item = item?.clone();
				if let Val::Str(item) = item {
					if !first {
						out += &sep;
					}
					first = false;
					write!(out, "{item}").unwrap()
				} else if matches!(item, Val::Null) {
					continue;
				} else {
					throw!("in std.join all items should be strings");
				}
			}

			IndexableVal::Str(out.into())
		}
	})
}

#[builtin]
pub fn builtin_reverse(value: ArrValue) -> Result<ArrValue> {
	Ok(value.reversed())
}

#[builtin]
pub fn builtin_any(arr: ArrValue) -> Result<bool> {
	for v in arr.iter() {
		let v = bool::from_untyped(v?)?;
		if v {
			return Ok(true);
		}
	}
	Ok(false)
}

#[builtin]
pub fn builtin_all(arr: ArrValue) -> Result<bool> {
	for v in arr.iter() {
		let v = bool::from_untyped(v?)?;
		if !v {
			return Ok(false);
		}
	}
	Ok(true)
}

#[builtin]
pub fn builtin_member(arr: IndexableVal, x: Any) -> Result<bool> {
	match arr {
		IndexableVal::Str(str) => {
			let x: IStr = IStr::from_untyped(x.0)?;
			Ok(!x.is_empty() && str.contains(&*x))
		}
		IndexableVal::Arr(a) => {
			for item in a.iter() {
				let item = item?;
				if equals(&item, &x.0)? {
					return Ok(true);
				}
			}
			Ok(false)
		}
	}
}

#[builtin]
pub fn builtin_count(arr: Vec<Any>, v: Any) -> Result<usize> {
	let mut count = 0;
	for item in &arr {
		if equals(&item.0, &v.0)? {
			count += 1;
		}
	}
	Ok(count)
}
