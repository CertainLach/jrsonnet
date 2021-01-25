use crate::{
	error::{Error, LocError, Result},
	throw, Context, FuncVal, Val,
};
use std::rc::Rc;

#[derive(Debug, Clone, thiserror::Error)]
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
struct NonNaNF64(f64);
impl PartialOrd for NonNaNF64 {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.0.partial_cmp(&other.0)
	}
}
impl Eq for NonNaNF64 {}
impl Ord for NonNaNF64 {
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

pub fn sort(ctx: Context, mut values: Rc<Vec<Val>>, key_getter: &FuncVal) -> Result<Rc<Vec<Val>>> {
	if values.len() <= 1 {
		return Ok(values);
	}
	if key_getter.is_ident() {
		let mvalues = Rc::make_mut(&mut values);
		let sort_type = get_sort_type(mvalues, |k| k)?;
		match sort_type {
			SortKeyType::Number => mvalues.sort_by_key(|v| match v {
				Val::Num(n) => NonNaNF64(*n),
				_ => unreachable!(),
			}),
			SortKeyType::String => mvalues.sort_by_key(|v| match v {
				Val::Str(s) => s.clone(),
				_ => unreachable!(),
			}),
			SortKeyType::Unknown => unreachable!(),
		};
		Ok(values)
	} else {
		let mut vk = Vec::with_capacity(values.len());
		for value in values.iter() {
			vk.push((
				value.clone(),
				key_getter.evaluate_values(ctx.clone(), &[value.clone()])?,
			));
		}
		let sort_type = get_sort_type(&mut vk, |v| &mut v.1)?;
		match sort_type {
			SortKeyType::Number => vk.sort_by_key(|v| match v.1 {
				Val::Num(n) => NonNaNF64(n),
				_ => unreachable!(),
			}),
			SortKeyType::String => vk.sort_by_key(|v| match &v.1 {
				Val::Str(s) => s.clone(),
				_ => unreachable!(),
			}),
			SortKeyType::Unknown => unreachable!(),
		};
		Ok(Rc::new(vk.into_iter().map(|v| v.0).collect()))
	}
}
