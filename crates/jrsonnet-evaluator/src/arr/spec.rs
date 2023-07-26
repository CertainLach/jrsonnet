use std::{cell::RefCell, iter, mem::replace, rc::Rc};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::{IBytes, IStr};
use jrsonnet_parser::LocExpr;

use super::ArrValue;
use crate::{
	error::ErrorKind::InfiniteRecursionDetected,
	evaluate,
	function::FuncVal,
	val::{StrValue, ThunkValue},
	Context, Error, Result, Thunk, Val,
};

pub trait ArrayLike: Sized + Into<ArrValue> {
	fn len(&self) -> usize;
	fn is_empty(&self) -> bool {
		self.len() == 0
	}
	fn get(&self, index: usize) -> Result<Option<Val>>;
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>>;
	fn get_cheap(&self, index: usize) -> Option<Val>;

	fn reverse(self) -> ArrValue {
		ArrValue::Reverse(Cc::new(ReverseArray(self.into())))
	}
}

#[derive(Debug, Clone, Trace)]
pub struct SliceArray {
	pub(crate) inner: ArrValue,
	pub(crate) from: u32,
	pub(crate) to: u32,
	pub(crate) step: u32,
}

impl SliceArray {
	fn iter(&self) -> impl Iterator<Item = Result<Val>> + '_ {
		self.inner
			.iter()
			.skip(self.from as usize)
			.take((self.to - self.from) as usize)
			.step_by(self.step as usize)
	}

	fn iter_lazy(&self) -> impl Iterator<Item = Thunk<Val>> + '_ {
		self.inner
			.iter_lazy()
			.skip(self.from as usize)
			.take((self.to - self.from) as usize)
			.step_by(self.step as usize)
	}

	fn iter_cheap(&self) -> Option<impl crate::arr::ArrayLikeIter<Val> + '_> {
		Some(
			self.inner
				.iter_cheap()?
				.skip(self.from as usize)
				.take((self.to - self.from) as usize)
				.step_by(self.step as usize),
		)
	}
}
impl ArrayLike for SliceArray {
	fn len(&self) -> usize {
		iter::repeat(())
			.take((self.to - self.from) as usize)
			.step_by(self.step as usize)
			.count()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		self.iter().nth(index).transpose()
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.iter_lazy().nth(index)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.iter_cheap()?.nth(index)
	}
}
impl From<SliceArray> for ArrValue {
	fn from(value: SliceArray) -> Self {
		Self::Slice(Cc::new(value))
	}
}

#[derive(Trace, Debug, Clone)]
pub struct CharArray(pub Rc<Vec<char>>);
impl ArrayLike for CharArray {
	fn len(&self) -> usize {
		self.0.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		Ok(self.get_cheap(index))
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.get_cheap(index).map(Thunk::evaluated)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.0
			.get(index)
			.map(|v| Val::Str(StrValue::Flat(IStr::from(*v))))
	}
}
impl From<CharArray> for ArrValue {
	fn from(value: CharArray) -> Self {
		ArrValue::Chars(value)
	}
}

#[derive(Trace, Debug, Clone)]
pub struct BytesArray(pub IBytes);
impl ArrayLike for BytesArray {
	fn len(&self) -> usize {
		self.0.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		Ok(self.get_cheap(index))
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.get_cheap(index).map(Thunk::evaluated)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.0.get(index).map(|v| Val::Num(f64::from(*v)))
	}
}
impl From<BytesArray> for ArrValue {
	fn from(value: BytesArray) -> Self {
		ArrValue::Bytes(value)
	}
}

#[derive(Debug, Trace, Clone)]
enum ArrayThunk<T: 'static + Trace> {
	Computed(Val),
	Errored(Error),
	Waiting(T),
	Pending,
}

#[derive(Debug, Trace)]
pub struct ExprArrayInner {
	ctx: Context,
	cached: RefCell<Vec<ArrayThunk<LocExpr>>>,
}
#[derive(Debug, Trace, Clone)]
pub struct ExprArray(pub Cc<ExprArrayInner>);
impl ExprArray {
	pub fn new(ctx: Context, items: impl IntoIterator<Item = LocExpr>) -> Self {
		Self(Cc::new(ExprArrayInner {
			ctx,
			cached: RefCell::new(items.into_iter().map(ArrayThunk::Waiting).collect()),
		}))
	}
}
impl ArrayLike for ExprArray {
	fn len(&self) -> usize {
		self.0.cached.borrow().len()
	}
	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index >= self.len() {
			return Ok(None);
		}
		match &self.0.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Ok(Some(c.clone())),
			ArrayThunk::Errored(e) => return Err(e.clone()),
			ArrayThunk::Pending => return Err(InfiniteRecursionDetected.into()),
			ArrayThunk::Waiting(..) => {}
		};

		let ArrayThunk::Waiting(expr) =
			replace(&mut self.0.cached.borrow_mut()[index], ArrayThunk::Pending)
		else {
			unreachable!()
		};

		let new_value = match evaluate(self.0.ctx.clone(), &expr) {
			Ok(v) => v,
			Err(e) => {
				self.0.cached.borrow_mut()[index] = ArrayThunk::Errored(e.clone());
				return Err(e);
			}
		};
		self.0.cached.borrow_mut()[index] = ArrayThunk::Computed(new_value.clone());
		Ok(Some(new_value))
	}
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		#[derive(Trace)]
		struct ArrayElement {
			arr_thunk: ExprArray,
			index: usize,
		}

		impl ThunkValue for ArrayElement {
			type Output = Val;

			fn get(self: Box<Self>) -> Result<Self::Output> {
				self.arr_thunk
					.get(self.index)
					.transpose()
					.expect("index checked")
			}
		}

		if index >= self.len() {
			return None;
		}
		match &self.0.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Some(Thunk::evaluated(c.clone())),
			ArrayThunk::Errored(e) => return Some(Thunk::errored(e.clone())),
			ArrayThunk::Waiting(_) | ArrayThunk::Pending => {}
		};

		Some(Thunk::new(ArrayElement {
			arr_thunk: self.clone(),
			index,
		}))
	}
	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}
}
impl From<ExprArray> for ArrValue {
	fn from(value: ExprArray) -> Self {
		Self::Expr(value)
	}
}

#[derive(Trace, Debug, Clone)]
pub struct ExtendedArray {
	pub a: ArrValue,
	pub b: ArrValue,
	split: usize,
	len: usize,
}
impl ExtendedArray {
	pub fn new(a: ArrValue, b: ArrValue) -> Self {
		let a_len = a.len();
		let b_len = b.len();
		Self {
			a,
			b,
			split: a_len,
			len: a_len.checked_add(b_len).expect("too large array value"),
		}
	}
}

struct WithExactSize<I>(I, usize);
impl<I, T> Iterator for WithExactSize<I>
where
	I: Iterator<Item = T>,
{
	type Item = T;

	fn next(&mut self) -> Option<Self::Item> {
		self.0.next()
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.0.nth(n)
	}
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.1, Some(self.1))
	}
}
impl<I> DoubleEndedIterator for WithExactSize<I>
where
	I: DoubleEndedIterator,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.0.next_back()
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.0.nth_back(n)
	}
}
impl<I> ExactSizeIterator for WithExactSize<I>
where
	I: Iterator,
{
	fn len(&self) -> usize {
		self.1
	}
}
impl ArrayLike for ExtendedArray {
	fn get(&self, index: usize) -> Result<Option<Val>> {
		if self.split > index {
			self.a.get(index)
		} else {
			self.b.get(index - self.split)
		}
	}
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		if self.split > index {
			self.a.get_lazy(index)
		} else {
			self.b.get_lazy(index - self.split)
		}
	}

	fn len(&self) -> usize {
		self.len
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		if self.split > index {
			self.a.get_cheap(index)
		} else {
			self.b.get_cheap(index - self.split)
		}
	}
}
impl From<ExtendedArray> for ArrValue {
	fn from(value: ExtendedArray) -> Self {
		Self::Extended(Cc::new(value))
	}
}

#[derive(Trace, Debug, Clone)]
pub struct LazyArray(pub Cc<Vec<Thunk<Val>>>);
impl ArrayLike for LazyArray {
	fn len(&self) -> usize {
		self.0.len()
	}
	fn get(&self, index: usize) -> Result<Option<Val>> {
		let Some(v) = self.0.get(index) else {
			return Ok(None);
		};
		v.evaluate().map(Some)
	}
	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.0.get(index).cloned()
	}
}
impl From<LazyArray> for ArrValue {
	fn from(value: LazyArray) -> Self {
		Self::Lazy(value)
	}
}

#[derive(Trace, Debug, Clone)]
pub struct EagerArray(pub Cc<Vec<Val>>);
impl ArrayLike for EagerArray {
	fn len(&self) -> usize {
		self.0.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		Ok(self.0.get(index).cloned())
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.0.get(index).cloned().map(Thunk::evaluated)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.0.get(index).cloned()
	}
}
impl From<EagerArray> for ArrValue {
	fn from(value: EagerArray) -> Self {
		Self::Eager(value)
	}
}

/// Inclusive range type
#[derive(Debug, Trace, Clone, PartialEq, Eq)]
pub struct RangeArray {
	start: i32,
	end: i32,
}
impl RangeArray {
	pub fn empty() -> Self {
		Self::new_exclusive(0, 0)
	}
	pub fn new_exclusive(start: i32, end: i32) -> Self {
		end.checked_sub(1)
			.map_or_else(Self::empty, |end| Self { start, end })
	}
	pub fn new_inclusive(start: i32, end: i32) -> Self {
		Self { start, end }
	}
	fn range(&self) -> impl Iterator<Item = i32> + ExactSizeIterator + DoubleEndedIterator {
		WithExactSize(
			self.start..=self.end,
			(self.end as usize)
				.wrapping_sub(self.start as usize)
				.wrapping_add(1),
		)
	}
}

impl ArrayLike for RangeArray {
	fn len(&self) -> usize {
		self.range().len()
	}
	fn is_empty(&self) -> bool {
		self.range().len() == 0
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		Ok(self.get_cheap(index))
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.get_cheap(index).map(Thunk::evaluated)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.range().nth(index).map(|i| Val::Num(f64::from(i)))
	}
}
impl From<RangeArray> for ArrValue {
	fn from(value: RangeArray) -> Self {
		Self::Range(value)
	}
}

#[derive(Debug, Trace, Clone)]
pub struct ReverseArray(pub ArrValue);
impl ArrayLike for ReverseArray {
	fn len(&self) -> usize {
		self.0.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		self.0.get(self.0.len() - index - 1)
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.0.get_lazy(self.0.len() - index - 1)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.0.get_cheap(self.0.len() - index - 1)
	}
	fn reverse(self) -> ArrValue {
		self.0
	}
}
impl From<ReverseArray> for ArrValue {
	fn from(value: ReverseArray) -> Self {
		Self::Reverse(Cc::new(value))
	}
}

#[derive(Trace, Debug)]
pub struct MappedArrayInner {
	inner: ArrValue,
	cached: RefCell<Vec<ArrayThunk<()>>>,
	mapper: FuncVal,
}
#[derive(Trace, Debug, Clone)]
pub struct MappedArray(Cc<MappedArrayInner>);
impl MappedArray {
	pub fn new(inner: ArrValue, mapper: FuncVal) -> Self {
		let len = inner.len();
		Self(Cc::new(MappedArrayInner {
			inner,
			cached: RefCell::new(vec![ArrayThunk::Waiting(()); len]),
			mapper,
		}))
	}
}
impl ArrayLike for MappedArray {
	fn len(&self) -> usize {
		self.0.cached.borrow().len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index >= self.len() {
			return Ok(None);
		}
		match &self.0.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Ok(Some(c.clone())),
			ArrayThunk::Errored(e) => return Err(e.clone()),
			ArrayThunk::Pending => return Err(InfiniteRecursionDetected.into()),
			ArrayThunk::Waiting(..) => {}
		};

		let ArrayThunk::Waiting(_) =
			replace(&mut self.0.cached.borrow_mut()[index], ArrayThunk::Pending)
		else {
			unreachable!()
		};

		let val = self
			.0
			.inner
			.get(index)
			.transpose()
			.expect("index checked")
			.and_then(|r| self.0.mapper.evaluate_simple(&(r,), false));

		let new_value = match val {
			Ok(v) => v,
			Err(e) => {
				self.0.cached.borrow_mut()[index] = ArrayThunk::Errored(e.clone());
				return Err(e);
			}
		};
		self.0.cached.borrow_mut()[index] = ArrayThunk::Computed(new_value.clone());
		Ok(Some(new_value))
	}
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		#[derive(Trace)]
		struct ArrayElement {
			arr_thunk: MappedArray,
			index: usize,
		}

		impl ThunkValue for ArrayElement {
			type Output = Val;

			fn get(self: Box<Self>) -> Result<Self::Output> {
				self.arr_thunk
					.get(self.index)
					.transpose()
					.expect("index checked")
			}
		}

		if index >= self.len() {
			return None;
		}
		match &self.0.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Some(Thunk::evaluated(c.clone())),
			ArrayThunk::Errored(e) => return Some(Thunk::errored(e.clone())),
			ArrayThunk::Waiting(_) | ArrayThunk::Pending => {}
		};

		Some(Thunk::new(ArrayElement {
			arr_thunk: self.clone(),
			index,
		}))
	}

	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}
}
impl From<MappedArray> for ArrValue {
	fn from(value: MappedArray) -> Self {
		Self::Mapped(value)
	}
}

#[derive(Trace, Debug)]
pub struct RepeatedArrayInner {
	data: ArrValue,
	repeats: usize,
	total_len: usize,
}
#[derive(Trace, Debug, Clone)]
pub struct RepeatedArray(Cc<RepeatedArrayInner>);
impl RepeatedArray {
	pub fn new(data: ArrValue, repeats: usize) -> Option<Self> {
		let total_len = data.len().checked_mul(repeats)?;
		Some(Self(Cc::new(RepeatedArrayInner {
			data,
			repeats,
			total_len,
		})))
	}
	pub fn is_cheap(&self) -> bool {
		self.0.data.is_cheap()
	}
}

impl ArrayLike for RepeatedArray {
	fn len(&self) -> usize {
		self.0.total_len
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index > self.0.total_len {
			return Ok(None);
		}
		self.0.data.get(index % self.0.data.len())
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		if index > self.0.total_len {
			return None;
		}
		self.0.data.get_lazy(index % self.0.data.len())
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		if index > self.0.total_len {
			return None;
		}
		self.0.data.get_cheap(index % self.0.data.len())
	}
}
impl From<RepeatedArray> for ArrValue {
	fn from(value: RepeatedArray) -> Self {
		Self::Repeated(value)
	}
}

macro_rules! pass {
	($t:ident.$m:ident($($ident:ident),*)) => {
		match $t {
			Self::Bytes(e) => e.$m($($ident)*),
			Self::Chars(e) => e.$m($($ident)*),
			Self::Expr(e) => e.$m($($ident)*),
			Self::Lazy(e) => e.$m($($ident)*),
			Self::Eager(e) => e.$m($($ident)*),
			Self::Range(e) => e.$m($($ident)*),
			Self::Slice(e) => e.$m($($ident)*),
			Self::Extended(e) => e.$m($($ident)*),
			Self::Reverse(e) => e.$m($($ident)*),
			Self::Mapped(e) => e.$m($($ident)*),
			Self::Repeated(e) => e.$m($($ident)*),
		}
	};
}
pub(super) use pass;
