use gcmodule::{Cc, Trace};

use crate::{
	error::{Error, LocError, Result},
	throw,
	typed::Any,
	val::FuncVal,
	State, Val,
};

#[derive(Debug, Clone, thiserror::Error, Trace)]
pub enum SortError {
	#[error("sort key should be string or number")]
	SortKeyShouldBeStringOrNumber,
	#[error("sort elements should have equal types")]
	SortElementsShouldHaveEqualType,
}

impl From<SortError> for LocError {
	fn from(s: SortError) -> Self {
		Self::new(Error::Sort(s))
	}
}

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

fn get_sort_type<T>(
	values: &mut Vec<T>,
	key_getter: impl Fn(&mut T) -> &mut Val,
) -> Result<SortKeyType> {
	let mut sort_type = SortKeyType::Unknown;
	for i in values.iter_mut() {
		let i = key_getter(i);
		match (i, sort_type) {
			(Val::Str(_), SortKeyType::Unknown) => sort_type = SortKeyType::String,
			(Val::Num(_), SortKeyType::Unknown) => sort_type = SortKeyType::Number,
			(Val::Str(_), SortKeyType::String) => {}
			(Val::Str(_), _) => throw!(SortError::SortElementsShouldHaveEqualType),
			(Val::Num(_), SortKeyType::Number) => {}
			(Val::Num(_), _) => throw!(SortError::SortElementsShouldHaveEqualType),
			_ => throw!(SortError::SortKeyShouldBeStringOrNumber),
		}
	}
	Ok(sort_type)
}

pub fn sort(s: State, values: Cc<Vec<Val>>, key_getter: Option<&FuncVal>) -> Result<Cc<Vec<Val>>> {
	if values.len() <= 1 {
		return Ok(values);
	}
	if let Some(key_getter) = key_getter {
		// Slow path, user provided key getter
		let mut vk = Vec::with_capacity(values.len());
		for value in values.iter() {
			vk.push((
				value.clone(),
				key_getter.evaluate_simple(s.clone(), &[Any(value.clone())].as_slice())?,
			));
		}
		let sort_type = get_sort_type(&mut vk, |v| &mut v.1)?;
		match sort_type {
			SortKeyType::Number => vk.sort_by_key(|v| match v.1 {
				Val::Num(n) => NonNaNf64(n),
				_ => unreachable!(),
			}),
			SortKeyType::String => vk.sort_by_key(|v| match &v.1 {
				Val::Str(s) => s.clone(),
				_ => unreachable!(),
			}),
			SortKeyType::Unknown => unreachable!(),
		};
		Ok(Cc::new(vk.into_iter().map(|v| v.0).collect()))
	} else {
		// Fast path, identity key getter
		let mut mvalues = (*values).clone();
		let sort_type = get_sort_type(&mut mvalues, |k| k)?;
		match sort_type {
			SortKeyType::Number => mvalues.sort_unstable_by_key(|v| match v {
				Val::Num(n) => NonNaNf64(*n),
				_ => unreachable!(),
			}),
			SortKeyType::String => mvalues.sort_unstable_by_key(|v| match v {
				Val::Str(s) => s.clone(),
				_ => unreachable!(),
			}),
			SortKeyType::Unknown => unreachable!(),
		};
		Ok(Cc::new(mvalues))
	}
}
