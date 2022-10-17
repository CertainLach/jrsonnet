use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, FuncVal},
	throw,
	typed::{Any, BoundedUsize, Typed, VecVal},
	val::{equals, ArrValue, IndexableVal},
	IStr, State, Val,
};
use jrsonnet_gcmodule::Cc;

#[builtin]
pub fn builtin_make_array(s: State, sz: usize, func: FuncVal) -> Result<VecVal> {
	let mut out = Vec::with_capacity(sz);
	for i in 0..sz {
		out.push(func.evaluate_simple(s.clone(), &(i as f64,))?);
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
pub fn builtin_map(s: State, func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.map(s.clone(), |val| {
		func.evaluate_simple(s.clone(), &(Any(val),))
	})
}

#[builtin]
pub fn builtin_flatmap(s: State, func: FuncVal, arr: IndexableVal) -> Result<IndexableVal> {
	match arr {
		IndexableVal::Str(str) => {
			let mut out = String::new();
			for c in str.chars() {
				match func.evaluate_simple(s.clone(), &(c.to_string(),))? {
					Val::Str(o) => out.push_str(&o),
					Val::Null => continue,
					_ => throw!("in std.join all items should be strings"),
				};
			}
			Ok(IndexableVal::Str(out.into()))
		}
		IndexableVal::Arr(a) => {
			let mut out = Vec::new();
			for el in a.iter(s.clone()) {
				let el = el?;
				match func.evaluate_simple(s.clone(), &(Any(el),))? {
					Val::Arr(o) => {
						for oe in o.iter(s.clone()) {
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
pub fn builtin_filter(s: State, func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.filter(s.clone(), |val| {
		bool::from_untyped(
			func.evaluate_simple(s.clone(), &(Any(val.clone()),))?,
			s.clone(),
		)
	})
}

#[builtin]
pub fn builtin_foldl(s: State, func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter(s.clone()) {
		acc = func.evaluate_simple(s.clone(), &(Any(acc), Any(i?)))?;
	}
	Ok(Any(acc))
}

#[builtin]
pub fn builtin_foldr(s: State, func: FuncVal, arr: ArrValue, init: Any) -> Result<Any> {
	let mut acc = init.0;
	for i in arr.iter(s.clone()).rev() {
		acc = func.evaluate_simple(s.clone(), &(Any(i?), Any(acc)))?;
	}
	Ok(Any(acc))
}

#[builtin]
pub fn builtin_range(from: i32, to: i32) -> Result<ArrValue> {
	if to < from {
		return Ok(ArrValue::new_eager());
	}
	Ok(ArrValue::new_range(from, to))
}

#[builtin]
pub fn builtin_join(s: State, sep: IndexableVal, arr: ArrValue) -> Result<IndexableVal> {
	Ok(match sep {
		IndexableVal::Arr(joiner_items) => {
			let mut out = Vec::new();

			let mut first = true;
			for item in arr.iter(s.clone()) {
				let item = item?.clone();
				if let Val::Arr(items) = item {
					if !first {
						out.reserve(joiner_items.len());
						// TODO: extend
						for item in joiner_items.iter(s.clone()) {
							out.push(item?);
						}
					}
					first = false;
					out.reserve(items.len());
					for item in items.iter(s.clone()) {
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
			for item in arr.iter(s) {
				let item = item?.clone();
				if let Val::Str(item) = item {
					if !first {
						out += &sep;
					}
					first = false;
					out += &item;
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
pub fn builtin_any(s: State, arr: ArrValue) -> Result<bool> {
	for v in arr.iter(s.clone()) {
		let v = bool::from_untyped(v?, s.clone())?;
		if v {
			return Ok(true);
		}
	}
	Ok(false)
}

#[builtin]
pub fn builtin_all(s: State, arr: ArrValue) -> Result<bool> {
	for v in arr.iter(s.clone()) {
		let v = bool::from_untyped(v?, s.clone())?;
		if !v {
			return Ok(false);
		}
	}
	Ok(true)
}

#[builtin]
pub fn builtin_member(s: State, arr: IndexableVal, x: Any) -> Result<bool> {
	match arr {
		IndexableVal::Str(str) => {
			let x: IStr = IStr::from_untyped(x.0, s)?;
			Ok(!x.is_empty() && str.contains(&*x))
		}
		IndexableVal::Arr(a) => {
			for item in a.iter(s.clone()) {
				let item = item?;
				if equals(s.clone(), &item, &x.0)? {
					return Ok(true);
				}
			}
			Ok(false)
		}
	}
}

#[builtin]
pub fn builtin_count(s: State, arr: Vec<Any>, v: Any) -> Result<usize> {
	let mut count = 0;
	for item in &arr {
		if equals(s.clone(), &item.0, &v.0)? {
			count += 1;
		}
	}
	Ok(count)
}
