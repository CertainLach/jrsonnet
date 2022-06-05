use jrsonnet_gcmodule::{Cc, Trace};

use crate::{
	error::{Error, LocError, Result},
	function::FuncVal,
	throw,
	typed::Any,
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
			(Val::Str(_), SortKeyType::String) | (Val::Num(_), SortKeyType::Number) => {}
			(Val::Str(_) | Val::Num(_), _) => {
				throw!(SortError::SortElementsShouldHaveEqualType)
			}
			_ => throw!(SortError::SortKeyShouldBeStringOrNumber),
		}
	}
	Ok(sort_type)
}

/// * `key_getter` - None, if identity sort required
pub fn sort(s: State, values: Cc<Vec<Val>>, key_getter: FuncVal) -> Result<Cc<Vec<Val>>> {
	if values.len() <= 1 {
		return Ok(values);
	}
	if key_getter.is_identity() {
		// Fast path, identity key getter
		let mut values = (*values).clone();
		let sort_type = get_sort_type(&mut values, |k| k)?;
		match sort_type {
			SortKeyType::Number => values.sort_unstable_by_key(|v| match v {
				Val::Num(n) => NonNaNf64(*n),
				_ => unreachable!(),
			}),
			SortKeyType::String => values.sort_unstable_by_key(|v| match v {
				Val::Str(s) => s.clone(),
				_ => unreachable!(),
			}),
			SortKeyType::Unknown => unreachable!(),
		};
		Ok(Cc::new(values))
	} else {
		// Slow path, user provided key getter
		let mut vk = Vec::with_capacity(values.len());
		for value in values.iter() {
			vk.push((
				value.clone(),
				key_getter.evaluate_simple(s.clone(), &(Any(value.clone()),))?,
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
	}
}
