use std::{
	any::Any,
	fmt::{self},
	num::NonZeroU32,
};

use jrsonnet_gcmodule::{cc_dyn, Cc};
use jrsonnet_interner::IBytes;
use jrsonnet_parser::LocExpr;

use crate::{function::FuncVal, Context, Result, Thunk, Val};

mod spec;
pub use spec::{ArrayLike, *};

cc_dyn!(
	#[doc = "Represents a Jsonnet array value."]
	#[derive(Clone)]
	ArrValue,
	ArrayLike,
	pub fn new() {...}
);
impl fmt::Debug for ArrValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

pub trait ArrayLikeIter<T>: Iterator<Item = T> + DoubleEndedIterator + ExactSizeIterator {}
impl<I, T> ArrayLikeIter<T> for I where
	I: Iterator<Item = T> + DoubleEndedIterator + ExactSizeIterator
{
}

impl ArrValue {
	pub fn empty() -> Self {
		Self::new(RangeArray::empty())
	}

	pub fn expr(ctx: Context, exprs: impl IntoIterator<Item = LocExpr>) -> Self {
		Self::new(ExprArray::new(ctx, exprs))
	}

	pub fn lazy(thunks: Vec<Thunk<Val>>) -> Self {
		Self::new(LazyArray(thunks))
	}

	pub fn eager(values: Vec<Val>) -> Self {
		Self::new(EagerArray(values))
	}

	pub fn repeated(data: Self, repeats: usize) -> Option<Self> {
		Some(Self::new(RepeatedArray::new(data, repeats)?))
	}

	pub fn bytes(bytes: IBytes) -> Self {
		Self::new(BytesArray(bytes))
	}
	pub fn chars(chars: impl Iterator<Item = char>) -> Self {
		Self::new(CharArray(chars.collect()))
	}

	#[must_use]
	pub fn map(self, mapper: FuncVal) -> Self {
		Self::new(<MappedArray<false>>::new(self, mapper))
	}

	#[must_use]
	pub fn map_with_index(self, mapper: FuncVal) -> Self {
		Self::new(<MappedArray<true>>::new(self, mapper))
	}

	pub fn filter(self, filter: impl Fn(&Val) -> Result<bool>) -> Result<Self> {
		// TODO: ArrValue::Picked(inner, indexes) for large arrays
		let mut out = Vec::new();
		for i in self.iter() {
			let i = i?;
			if filter(&i)? {
				out.push(i);
			};
		}
		Ok(Self::eager(out))
	}

	pub fn extended(a: Self, b: Self) -> Self {
		// TODO: benchmark for an optimal value, currently just a arbitrary choice
		const ARR_EXTEND_THRESHOLD: usize = 100;

		if a.is_empty() {
			b
		} else if b.is_empty() {
			a
		} else if a.len() + b.len() > ARR_EXTEND_THRESHOLD {
			Self::new(ExtendedArray::new(a, b))
		} else if let (Some(a), Some(b)) = (a.iter_cheap(), b.iter_cheap()) {
			let mut out = Vec::with_capacity(a.len() + b.len());
			out.extend(a);
			out.extend(b);
			Self::eager(out)
		} else {
			let mut out = Vec::with_capacity(a.len() + b.len());
			out.extend(a.iter_lazy());
			out.extend(b.iter_lazy());
			Self::lazy(out)
		}
	}

	pub fn range_exclusive(a: i32, b: i32) -> Self {
		Self::new(RangeArray::new_exclusive(a, b))
	}
	pub fn range_inclusive(a: i32, b: i32) -> Self {
		Self::new(RangeArray::new_inclusive(a, b))
	}

	#[must_use]
	pub fn slice(self, index: Option<i32>, end: Option<i32>, step: Option<NonZeroU32>) -> Self {
		let get_idx = |pos: Option<i32>, len: usize, default| match pos {
			Some(v) if v < 0 => len.saturating_sub((-v) as usize),
			Some(v) => (v as usize).min(len),
			None => default,
		};
		let index = get_idx(index, self.len(), 0);
		let end = get_idx(end, self.len(), self.len());
		let step = step.unwrap_or_else(|| NonZeroU32::new(1).expect("1 != 0"));

		if index >= end {
			return Self::empty();
		}

		Self::new(SliceArray {
			inner: self,
			from: index as u32,
			to: end as u32,
			step: step.get(),
		})
	}

	/// Array length.
	pub fn len(&self) -> usize {
		self.0.len()
	}

	/// Is array contains no elements?
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	/// Get array element by index, evaluating it, if it is lazy.
	///
	/// Returns `None` on out-of-bounds condition.
	pub fn get(&self, index: usize) -> Result<Option<Val>> {
		self.0.get(index)
	}

	/// Returns None if get is either non cheap, or out of bounds
	/// Note that non-cheap access includes errorable values
	///
	/// Prefer it to `get_lazy`, but use `get` when you can.
	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.0.get_cheap(index)
	}

	/// Get array element by index, without evaluation.
	///
	/// Returns `None` on out-of-bounds condition.
	pub fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.0.get_lazy(index)
	}

	pub fn iter(&self) -> impl ArrayLikeIter<Result<Val>> + '_ {
		(0..self.len()).map(|i| self.get(i).transpose().expect("length checked"))
	}

	/// Iterate over elements, returning lazy values.
	pub fn iter_lazy(&self) -> impl ArrayLikeIter<Thunk<Val>> + '_ {
		(0..self.len()).map(|i| self.get_lazy(i).expect("length checked"))
	}

	/// Prefer it over `iter_lazy`, but do not use it where `iter` will do.
	pub fn iter_cheap(&self) -> Option<impl ArrayLikeIter<Val> + '_> {
		if self.is_cheap() {
			Some((0..self.len()).map(|i| self.get_cheap(i).expect("length and is_cheap checked")))
		} else {
			None
		}
	}

	/// Return a reversed view on current array.
	#[must_use]
	pub fn reversed(self) -> Self {
		Self::new(ReverseArray(self))
	}

	pub fn ptr_eq(a: &Self, b: &Self) -> bool {
		Cc::ptr_eq(&a.0, &b.0)
	}

	/// Is this vec supports `.get_cheap()?`
	pub fn is_cheap(&self) -> bool {
		self.0.is_cheap()
	}

	pub fn as_any(&self) -> &dyn Any {
		&self.0
	}
}
impl From<Vec<Val>> for ArrValue {
	fn from(value: Vec<Val>) -> Self {
		Self::eager(value)
	}
}
impl From<Vec<Thunk<Val>>> for ArrValue {
	fn from(value: Vec<Thunk<Val>>) -> Self {
		Self::lazy(value)
	}
}
impl FromIterator<Val> for ArrValue {
	fn from_iter<T: IntoIterator<Item = Val>>(iter: T) -> Self {
		Self::eager(iter.into_iter().collect())
	}
}
impl ArrayLike for ArrValue {
	fn len(&self) -> usize {
		self.0.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		self.0.get(index)
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.0.get_lazy(index)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.0.get_cheap(index)
	}

	fn is_cheap(&self) -> bool {
		self.0.is_cheap()
	}
}
