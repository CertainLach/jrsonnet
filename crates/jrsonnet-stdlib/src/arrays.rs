#![allow(non_snake_case)]

use jrsonnet_evaluator::{
	bail,
	function::{builtin, FuncVal},
	runtime_error,
	typed::{BoundedI32, BoundedUsize, Either2, NativeFn, Typed},
	val::{equals, ArrValue, IndexableVal},
	Either, IStr, ObjValueBuilder, Result, ResultExt, Thunk, Val,
};

pub fn eval_on_empty(on_empty: Option<Thunk<Val>>) -> Result<Val> {
	if let Some(on_empty) = on_empty {
		on_empty.evaluate()
	} else {
		bail!("expected non-empty array")
	}
}

#[builtin]
pub fn builtin_make_array(sz: BoundedI32<0, { i32::MAX }>, func: FuncVal) -> Result<ArrValue> {
	if *sz == 0 {
		return Ok(ArrValue::empty());
	}
	func.evaluate_trivial().map_or_else(
		|| Ok(ArrValue::range_exclusive(0, *sz).map(func)),
		|trivial| {
			let mut out = Vec::with_capacity(*sz as usize);
			for _ in 0..*sz {
				out.push(trivial.clone());
			}
			Ok(ArrValue::eager(out))
		},
	)
}

#[builtin]
pub fn builtin_repeat(what: Either![IStr, ArrValue], count: usize) -> Result<Val> {
	Ok(match what {
		Either2::A(s) => Val::string(s.repeat(count)),
		Either2::B(arr) => Val::Arr(
			ArrValue::repeated(arr, count)
				.ok_or_else(|| runtime_error!("repeated length overflow"))?,
		),
	})
}

#[builtin]
pub fn builtin_slice(
	indexable: IndexableVal,
	index: Option<i32>,
	end: Option<i32>,
	step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
) -> Result<Val> {
	indexable.slice(index, end, step).map(Val::from)
}

#[builtin]
pub fn builtin_map(func: FuncVal, arr: IndexableVal) -> ArrValue {
	let arr = arr.to_array();
	arr.map(func)
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
					_ => bail!("in std.join all items should be strings"),
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
					_ => bail!("in std.join all items should be arrays"),
				};
			}
			Ok(IndexableVal::Arr(out.into()))
		}
	}
}

#[builtin]
pub fn builtin_filter(func: FuncVal, arr: ArrValue) -> Result<ArrValue> {
	arr.filter(|val| bool::from_untyped(func.evaluate_simple(&(val.clone(),), false)?))
}

#[builtin]
pub fn builtin_filter_map(
	filter_func: FuncVal,
	map_func: FuncVal,
	arr: ArrValue,
) -> Result<ArrValue> {
	Ok(builtin_filter(filter_func, arr)?.map(map_func))
}

#[builtin]
pub fn builtin_foldl(func: FuncVal, arr: ArrValue, init: Val) -> Result<Val> {
	let mut acc = init;
	for i in arr.iter() {
		acc = func.evaluate_simple(&(acc, i?), false)?;
	}
	Ok(acc)
}

#[builtin]
pub fn builtin_foldr(func: FuncVal, arr: ArrValue, init: Val) -> Result<Val> {
	let mut acc = init;
	for i in arr.iter().rev() {
		acc = func.evaluate_simple(&(i?, acc), false)?;
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
					bail!("in std.join all items should be arrays");
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
					write!(out, "{item}").unwrap();
				} else if matches!(item, Val::Null) {
					continue;
				} else {
					bail!("in std.join all items should be strings");
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
pub fn builtin_contains(arr: IndexableVal, elem: Val) -> Result<bool> {
	builtin_member(arr, elem)
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

#[builtin]
pub fn builtin_avg(arr: Vec<f64>, onEmpty: Option<Thunk<Val>>) -> Result<Val> {
	if arr.is_empty() {
		return eval_on_empty(onEmpty);
	}
	Ok(Val::Num(arr.iter().sum::<f64>() / (arr.len() as f64)))
}

#[builtin]
pub fn builtin_remove_at(arr: ArrValue, at: i32) -> Result<ArrValue> {
	let newArrLeft = arr.clone().slice(None, Some(at), None);
	let newArrRight = arr.slice(Some(at + 1), None, None);

	Ok(ArrValue::extended(newArrLeft, newArrRight))
}

#[builtin]
pub fn builtin_remove(arr: ArrValue, elem: Val) -> Result<ArrValue> {
	for (index, item) in arr.iter().enumerate() {
		if equals(&item?, &elem)? {
			return builtin_remove_at(arr.clone(), index as i32);
		}
	}
	Ok(arr)
}

#[builtin]
pub fn builtin_flatten_arrays(arrs: Vec<ArrValue>) -> ArrValue {
	pub fn flatten_inner(values: &[ArrValue]) -> ArrValue {
		if values.len() == 1 {
			return values[0].clone();
		} else if values.len() == 2 {
			return ArrValue::extended(values[0].clone(), values[1].clone());
		}
		let (a, b) = values.split_at(values.len() / 2);
		ArrValue::extended(flatten_inner(a), flatten_inner(b))
	}
	if arrs.is_empty() {
		return ArrValue::empty();
	} else if arrs.len() == 1 {
		return arrs.into_iter().next().expect("single");
	}
	flatten_inner(&arrs)
}

#[builtin]
pub fn builtin_flatten_deep_array(value: Val) -> Result<Vec<Val>> {
	fn process(value: Val, out: &mut Vec<Val>) -> Result<()> {
		match value {
			Val::Arr(arr) => {
				for ele in arr.iter() {
					process(ele?, out)?;
				}
			}
			_ => out.push(value),
		}
		Ok(())
	}
	let mut out = Vec::new();
	process(value, &mut out)?;
	Ok(out)
}

#[builtin]
pub fn builtin_prune(
	a: Val,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<Val> {
	fn is_content(val: &Val) -> bool {
		match val {
			Val::Null => false,
			Val::Arr(a) => !a.is_empty(),
			Val::Obj(o) => !o.is_empty(),
			_ => true,
		}
	}
	Ok(match a {
		Val::Arr(a) => {
			let mut out = Vec::new();
			for (i, ele) in a.iter().enumerate() {
				let ele = ele
					.and_then(|v| {
						builtin_prune(
							v,
							#[cfg(feature = "exp-preserve-order")]
							preserve_order,
						)
					})
					.with_description(|| format!("elem <{i}> pruning"))?;
				if is_content(&ele) {
					out.push(ele);
				}
			}
			Val::Arr(ArrValue::eager(out))
		}
		Val::Obj(o) => {
			let mut out = ObjValueBuilder::new();
			for (name, value) in o.iter(
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			) {
				let value = value
					.and_then(|v| {
						builtin_prune(
							v,
							#[cfg(feature = "exp-preserve-order")]
							preserve_order,
						)
					})
					.with_description(|| format!("field <{name}> pruning"))?;
				if !is_content(&value) {
					continue;
				}
				out.field(name).value(value);
			}
			Val::Obj(out.build())
		}
		_ => a,
	})
}
