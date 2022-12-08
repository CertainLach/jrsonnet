use jrsonnet_evaluator::{
	error::{ErrorKind::RuntimeError, Result},
	function::{builtin, FuncVal},
	throw,
	typed::{BoundedI32, BoundedUsize, Either2, NativeFn, Typed},
	val::{equals, ArrValue, IndexableVal, StrValue},
	Either, IStr, Val,
};

#[builtin]
pub fn builtin_make_array(sz: BoundedI32<0, { i32::MAX }>, func: FuncVal) -> Result<ArrValue> {
	if *sz == 0 {
		return Ok(ArrValue::empty());
	}
	if let Some(trivial) = func.evaluate_trivial() {
		let mut out = Vec::with_capacity(*sz as usize);
		for _ in 0..*sz {
			out.push(trivial.clone())
		}
		Ok(ArrValue::eager(out))
	} else {
		Ok(ArrValue::range_exclusive(0, *sz).map(func))
	}
}

#[builtin]
pub fn builtin_repeat(what: Either![IStr, ArrValue], count: usize) -> Result<Val> {
	Ok(match what {
		Either2::A(s) => Val::Str(StrValue::Flat(s.repeat(count).into())),
		Either2::B(arr) => Val::Arr(
			ArrValue::repeated(arr, count)
				.ok_or_else(|| RuntimeError("repeated length overflow".into()))?,
		),
	})
}

#[builtin]
pub fn builtin_slice(
	indexable: IndexableVal,
	index: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	end: Option<BoundedUsize<0, { i32::MAX as usize }>>,
	step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
) -> Result<Val> {
	indexable.slice(index, end, step).map(Val::from)
}

#[builtin]
pub fn builtin_map(func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	Ok(arr.map(func))
}

#[builtin]
pub fn builtin_flatmap(
	func: NativeFn<((Either![String, Val],), Val)>,
	arr: IndexableVal,
) -> Result<IndexableVal> {
	use std::fmt::Write;
	match arr {
		IndexableVal::Str(str) => {
			let mut out = String::new();
			for c in str.chars() {
				match func(Either2::A(c.to_string()))? {
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
				match func(Either2::B(el))? {
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
	arr.filter(|val| bool::from_untyped(func.evaluate_simple(&(val.clone(),))?))
}

#[builtin]
pub fn builtin_foldl(func: FuncVal, arr: ArrValue, init: Val) -> Result<Val> {
	let mut acc = init;
	for i in arr.iter() {
		acc = func.evaluate_simple(&(acc, i?))?;
	}
	Ok(acc)
}

#[builtin]
pub fn builtin_foldr(func: FuncVal, arr: ArrValue, init: Val) -> Result<Val> {
	let mut acc = init;
	for i in arr.iter().rev() {
		acc = func.evaluate_simple(&(i?, acc))?;
	}
	Ok(acc)
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
pub fn builtin_reverse(arr: ArrValue) -> ArrValue {
	arr.reversed()
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
pub fn builtin_member(arr: IndexableVal, x: Val) -> Result<bool> {
	match arr {
		IndexableVal::Str(str) => {
			let x: IStr = IStr::from_untyped(x)?;
			Ok(!x.is_empty() && str.contains(&*x))
		}
		IndexableVal::Arr(a) => {
			for item in a.iter() {
				let item = item?;
				if equals(&item, &x)? {
					return Ok(true);
				}
			}
			Ok(false)
		}
	}
}

#[builtin]
pub fn builtin_count(arr: ArrValue, x: Val) -> Result<usize> {
	let mut count = 0;
	for item in arr.iter() {
		if equals(&item?, &x)? {
			count += 1;
		}
	}
	Ok(count)
}
