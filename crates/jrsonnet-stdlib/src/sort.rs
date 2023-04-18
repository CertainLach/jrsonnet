use jrsonnet_evaluator::{
	error::Result,
	function::{builtin, FuncVal},
	throw,
	val::{equals, ArrValue},
	Thunk, Val,
};
use jrsonnet_gcmodule::Cc;

#[derive(Copy, Clone)]
enum SortKeyType {
	Number,
	String,
	Unknown,
}

#[derive(PartialEq)]
struct NonNaNf64(f64);
impl PartialOrd for NonNaNf64 {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.0.partial_cmp(&other.0)
	}
}
impl Eq for NonNaNf64 {}
impl Ord for NonNaNf64 {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.partial_cmp(other).expect("non nan")
	}
}

fn get_sort_type<T>(values: &[T], key_getter: impl Fn(&T) -> &Val) -> Result<SortKeyType> {
	let mut sort_type = SortKeyType::Unknown;
	for i in values.iter() {
		let i = key_getter(i);
		match (i, sort_type) {
			(Val::Str(_), SortKeyType::Unknown) => sort_type = SortKeyType::String,
			(Val::Num(_), SortKeyType::Unknown) => sort_type = SortKeyType::Number,
			(Val::Str(_), SortKeyType::String) | (Val::Num(_), SortKeyType::Number) => {}
			(Val::Str(_) | Val::Num(_), _) => {
				throw!("sort elements should have the same types")
			}
			_ => throw!("sort key should either be a string or a number"),
		}
	}
	Ok(sort_type)
}

fn sort_identity(mut values: Vec<Val>) -> Result<Vec<Val>> {
	// Fast path, identity key getter
	let sort_type = get_sort_type(&values, |k| k)?;
	match sort_type {
		SortKeyType::Number => values.sort_unstable_by_key(|v| match v {
			Val::Num(n) => NonNaNf64(*n),
			_ => unreachable!(),
		}),
		SortKeyType::String => values.sort_unstable_by_key(|v| match v {
			Val::Str(s) => s.clone(),
			_ => unreachable!(),
		}),
		SortKeyType::Unknown => unreachable!("list is not empty, as checked in sort"),
	};
	Ok(values)
}

fn sort_keyf(values: ArrValue, keyf: FuncVal) -> Result<Vec<Thunk<Val>>> {
	// Slow path, user provided key getter
	let mut vk = Vec::with_capacity(values.len());
	for value in values.iter_lazy() {
		vk.push((
			value.clone(),
			keyf.evaluate_simple(&(value.clone(),), false)?,
		));
	}
	let sort_type = get_sort_type(&vk, |v| &v.1)?;
	match sort_type {
		SortKeyType::Number => vk.sort_by_key(|v| match v.1 {
			Val::Num(n) => NonNaNf64(n),
			_ => unreachable!(),
		}),
		SortKeyType::String => vk.sort_by_key(|v| match &v.1 {
			Val::Str(s) => s.clone(),
			_ => unreachable!(),
		}),
		SortKeyType::Unknown => unreachable!("list is not empty, as checked in sort"),
	};
	Ok(vk.into_iter().map(|v| v.0).collect())
}

/// * `key_getter` - None, if identity sort required
pub fn sort(values: ArrValue, key_getter: FuncVal) -> Result<ArrValue> {
	if values.len() <= 1 {
		return Ok(values);
	}
	if key_getter.is_identity() {
		Ok(ArrValue::eager(sort_identity(
			values.iter().collect::<Result<Vec<Val>>>()?,
		)?))
	} else {
		Ok(ArrValue::lazy(Cc::new(sort_keyf(values, key_getter)?)))
	}
}

#[builtin]
#[allow(non_snake_case)]
pub fn builtin_sort(arr: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	super::sort::sort(arr, keyF.unwrap_or_else(FuncVal::identity))
}

fn uniq_identity(arr: Vec<Val>) -> Result<Vec<Val>> {
	let mut out = Vec::new();
	let mut last = arr[0].clone();
	out.push(last.clone());
	for next in arr.into_iter().skip(1) {
		if !equals(&last, &next)? {
			out.push(next.clone());
		}
		last = next;
	}
	Ok(out)
}

fn uniq_keyf(arr: ArrValue, keyf: FuncVal) -> Result<Vec<Thunk<Val>>> {
	let mut out = Vec::new();
	let last_value = arr.get_lazy(0).unwrap();
	let mut last_key = keyf.evaluate_simple(&(last_value.clone(),), false)?;
	out.push(last_value);

	for next in arr.iter_lazy().skip(1) {
		let next_key = keyf.evaluate_simple(&(next.clone(),), false)?;
		if !equals(&last_key, &next_key)? {
			out.push(next.clone());
		}
		last_key = next_key;
	}
	Ok(out)
}

#[builtin]
#[allow(non_snake_case)]
pub fn builtin_uniq(arr: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	if arr.len() <= 1 {
		return Ok(arr);
	}
	let keyF = keyF.unwrap_or(FuncVal::identity());
	if keyF.is_identity() {
		Ok(ArrValue::eager(uniq_identity(
			arr.iter().collect::<Result<Vec<Val>>>()?,
		)?))
	} else {
		Ok(ArrValue::lazy(Cc::new(uniq_keyf(arr, keyF)?)))
	}
}

#[builtin]
#[allow(non_snake_case)]
pub fn builtin_set(arr: ArrValue, keyF: Option<FuncVal>) -> Result<ArrValue> {
	if arr.len() <= 1 {
		return Ok(arr);
	}
	let keyF = keyF.unwrap_or(FuncVal::identity());
	if keyF.is_identity() {
		let arr = arr.iter().collect::<Result<Vec<Val>>>()?;
		let arr = sort_identity(arr)?;
		let arr = uniq_identity(arr)?;
		Ok(ArrValue::eager(arr))
	} else {
		let arr = sort_keyf(arr, keyF.clone())?;
		let arr = uniq_keyf(ArrValue::lazy(Cc::new(arr)), keyF)?;
		Ok(ArrValue::lazy(Cc::new(arr)))
	}
}
