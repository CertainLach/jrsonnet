use std::{
	cell::RefCell,
	iter::{self, Rev},
	mem::replace,
};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IBytes;
use jrsonnet_parser::LocExpr;

use super::ArrValue;
use crate::{
	error::ErrorKind::InfiniteRecursionDetected, evaluate, function::FuncVal, tb, typed::Any,
	val::ThunkValue, Context, Error, Result, Thunk, Val,
};

pub trait ArrayLike {
	type Iter<'t>
	where
		Self: 't;
	type IterLazy<'t>
	where
		Self: 't;
	type IterCheap<'t>
	where
		Self: 't;

	fn len(&self) -> usize;
	fn is_empty(&self) -> bool {
		self.len() == 0
	}
	fn get(&self, index: usize) -> Result<Option<Val>>;
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>>;
	fn get_cheap(&self, index: usize) -> Option<Val>;
	fn evaluated(&self) -> Result<Vec<Val>>;
	#[allow(clippy::iter_not_returning_iterator)]
	fn iter(&self) -> Self::Iter<'_>;
	fn iter_lazy(&self) -> Self::IterLazy<'_>;
	fn iter_cheap(&self) -> Option<Self::IterCheap<'_>>;
}

#[derive(Debug, Clone, Trace)]
pub struct SliceArray {
	pub(crate) inner: ArrValue,
	pub(crate) from: u32,
	pub(crate) to: u32,
	pub(crate) step: u32,
}
type SliceArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type SliceArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type SliceArrayCheapIter<'t> = impl DoubleEndedIterator<Item = Val> + ExactSizeIterator + 't;
impl ArrayLike for SliceArray {
	type Iter<'t> = SliceArrayIter<'t>;

	type IterLazy<'t> = SliceArrayLazyIter<'t>;

	type IterCheap<'t> = SliceArrayCheapIter<'t>;

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

	fn evaluated(&self) -> Result<Vec<Val>> {
		self.iter().collect()
	}

	fn iter(&self) -> SliceArrayIter<'_> {
		self.inner
			.iter()
			.skip(self.from as usize)
			.take((self.to - self.from) as usize)
			.step_by(self.step as usize)
	}

	fn iter_lazy(&self) -> SliceArrayLazyIter<'_> {
		self.inner
			.iter_lazy()
			.skip(self.from as usize)
			.take((self.to - self.from) as usize)
			.step_by(self.step as usize)
	}

	fn iter_cheap(&self) -> Option<SliceArrayCheapIter<'_>> {
		Some(
			self.inner
				.iter_cheap()?
				.skip(self.from as usize)
				.take((self.to - self.from) as usize)
				.step_by(self.step as usize),
		)
	}
}

#[derive(Trace, Debug, Clone)]
pub struct BytesArray(pub IBytes);
type BytesArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type BytesArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type BytesArrayCheapIter<'t> = impl DoubleEndedIterator<Item = Val> + ExactSizeIterator + 't;
impl ArrayLike for BytesArray {
	type Iter<'t> = BytesArrayIter<'t>;

	type IterLazy<'t> = BytesArrayLazyIter<'t>;

	type IterCheap<'t> = BytesArrayCheapIter<'t>;

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

	fn evaluated(&self) -> Result<Vec<Val>> {
		self.iter().collect()
	}

	fn iter(&self) -> BytesArrayIter<'_> {
		self.0.iter().map(|v| Ok(Val::Num(f64::from(*v))))
	}

	fn iter_lazy(&self) -> BytesArrayLazyIter<'_> {
		self.0
			.iter()
			.map(|v| Thunk::evaluated(Val::Num(f64::from(*v))))
	}

	fn iter_cheap(&self) -> Option<BytesArrayCheapIter<'_>> {
		Some(self.0.iter().map(|v| Val::Num(f64::from(*v))))
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
type ExprArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type ExprArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type ExprArrayCheapIter<'t> = iter::Empty<Val>;
impl ExprArray {
	pub fn new(ctx: Context, items: impl IntoIterator<Item = LocExpr>) -> Self {
		Self(Cc::new(ExprArrayInner {
			ctx,
			cached: RefCell::new(items.into_iter().map(ArrayThunk::Waiting).collect()),
		}))
	}
}
impl ArrayLike for ExprArray {
	type Iter<'t> = ExprArrayIter<'t>;

	type IterLazy<'t> = ExprArrayLazyIter<'t>;

	type IterCheap<'t> = ExprArrayCheapIter<'t>;

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

		let ArrayThunk::Waiting(expr) = replace(&mut self.0.cached.borrow_mut()[index], ArrayThunk::Pending) else {
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

		Some(Thunk::new(tb!(ArrayElement {
			arr_thunk: self.clone(),
			index,
		})))
	}
	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}

	fn iter(&self) -> ExprArrayIter<'_> {
		(0..self.len()).map(|i| self.get(i).transpose().expect("index checked"))
	}
	fn iter_lazy(&self) -> ExprArrayLazyIter<'_> {
		(0..self.len()).map(|i| self.get_lazy(i).expect("index checked"))
	}
	fn iter_cheap(&self) -> Option<ExprArrayCheapIter<'_>> {
		None
	}

	fn evaluated(&self) -> Result<Vec<Val>> {
		self.iter().collect()
	}
}

#[derive(Trace, Debug, Clone)]
pub struct ExtendedArray {
	pub a: ArrValue,
	pub b: ArrValue,
	split: usize,
	len: usize,
}
type ExtendedArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + 't;
type ExtendedArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + 't;
type ExtendedArrayCheapIter<'t> = impl DoubleEndedIterator<Item = Val> + 't;
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
impl ArrayLike for ExtendedArray {
	type Iter<'t> = ExtendedArrayIter<'t>;

	type IterLazy<'t> = ExtendedArrayLazyIter<'t>;

	type IterCheap<'t> = ExtendedArrayCheapIter<'t>;

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

	fn evaluated(&self) -> Result<Vec<Val>> {
		let mut out = self.a.evaluated()?;
		out.extend(self.b.evaluated()?.into_iter());
		Ok(out)
	}

	fn iter(&self) -> ExtendedArrayIter<'_> {
		self.a.iter().chain(self.b.iter())
	}
	fn iter_lazy(&self) -> ExtendedArrayLazyIter<'_> {
		self.a.iter_lazy().chain(self.b.iter_lazy())
	}
	fn iter_cheap(&self) -> Option<ExtendedArrayCheapIter<'_>> {
		let a = self.a.iter_cheap()?;
		let b = self.b.iter_cheap()?;
		Some(a.chain(b))
	}
}

#[derive(Trace, Debug, Clone)]
pub struct LazyArray(pub Cc<Vec<Thunk<Val>>>);
type LazyArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type LazyArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type LazyArrayCheapIter<'t> = iter::Empty<Val>;
impl ArrayLike for LazyArray {
	type Iter<'t> = LazyArrayIter<'t>;

	type IterLazy<'t> = LazyArrayLazyIter<'t>;

	type IterCheap<'t> = LazyArrayCheapIter<'t>;

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
	fn evaluated(&self) -> Result<Vec<Val>> {
		let mut out = Vec::with_capacity(self.len());
		for i in self.0.iter() {
			out.push(i.evaluate()?);
		}
		Ok(out)
	}
	fn iter(&self) -> LazyArrayIter<'_> {
		self.0.iter().map(Thunk::evaluate)
	}
	fn iter_lazy(&self) -> LazyArrayLazyIter<'_> {
		self.0.iter().cloned()
	}
	fn iter_cheap(&self) -> Option<LazyArrayCheapIter<'_>> {
		None
	}
}

#[derive(Trace, Debug, Clone)]
pub struct EagerArray(pub Cc<Vec<Val>>);
type EagerArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type EagerArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type EagerArrayCheapIter<'t> = impl DoubleEndedIterator<Item = Val> + ExactSizeIterator + 't;
impl ArrayLike for EagerArray {
	type Iter<'t> = EagerArrayIter<'t>;

	type IterLazy<'t> = EagerArrayLazyIter<'t>;

	type IterCheap<'t> = EagerArrayCheapIter<'t>;

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

	fn evaluated(&self) -> Result<Vec<Val>> {
		Ok((*self.0).clone())
	}

	fn iter(&self) -> EagerArrayIter<'_> {
		self.0.iter().cloned().map(Ok)
	}

	fn iter_lazy(&self) -> EagerArrayLazyIter<'_> {
		self.0.iter().cloned().map(Thunk::evaluated)
	}

	fn iter_cheap(&self) -> Option<EagerArrayCheapIter<'_>> {
		Some(self.0.iter().cloned())
	}
}

/// Inclusive range type
#[derive(Debug, Trace, Clone, PartialEq, Eq)]
pub struct RangeArray {
	start: i32,
	end: i32,
}
struct RangeIter {
	start: i32,
	end: i32,
}
impl RangeIter {
	fn finished(&self) -> bool {
		self.end < self.start
	}
	fn finish(&mut self) {
		self.start = 0;
		self.end = -1;
	}
}
impl Iterator for RangeIter {
	type Item = i32;

	fn next(&mut self) -> Option<Self::Item> {
		if self.finished() {
			return None;
		}
		let v = self.start;
		if v == self.end {
			self.finish();
		} else {
			self.start = v + 1;
		}
		Some(v)
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		let v = (self.start as usize) + n;
		if v > self.end as usize {
			self.finish();
			None
		} else {
			self.start = v as i32;
			self.next()
		}
	}
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.len();
		(len, Some(len))
	}
}
impl DoubleEndedIterator for RangeIter {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.finished() {
			return None;
		}
		let v = self.end;
		if v == self.start {
			self.finish();
		} else {
			self.end = v - 1;
		}
		Some(v)
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		let v = (self.end as usize) - n;
		if v < self.start as usize {
			self.finish();
			None
		} else {
			self.end = v as i32;
			self.next_back()
		}
	}
}
impl ExactSizeIterator for RangeIter {
	fn len(&self) -> usize {
		if self.finished() {
			0
		} else {
			(self.end as isize - self.start as isize + 1) as usize
		}
	}
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
	fn range(&self) -> RangeIter {
		RangeIter {
			start: self.start,
			end: self.end,
		}
	}
}

type RangeArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type RangeArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type RangeArrayCheapIter<'t> = impl DoubleEndedIterator<Item = Val> + ExactSizeIterator + 't;
impl ArrayLike for RangeArray {
	type Iter<'t> = RangeArrayIter<'t>;

	type IterLazy<'t> = RangeArrayLazyIter<'t>;

	type IterCheap<'t> = RangeArrayCheapIter<'t>;

	fn len(&self) -> usize {
		self.range().len()
	}
	fn is_empty(&self) -> bool {
		self.range().finished()
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

	fn evaluated(&self) -> Result<Vec<Val>> {
		Ok(self.range().map(|i| Val::Num(f64::from(i))).collect())
	}

	fn iter(&self) -> RangeArrayIter<'_> {
		self.range().map(|i| Ok(Val::Num(f64::from(i))))
	}

	fn iter_lazy(&self) -> RangeArrayLazyIter<'_> {
		self.range()
			.map(|i| Thunk::evaluated(Val::Num(f64::from(i))))
	}

	fn iter_cheap(&self) -> Option<RangeArrayCheapIter<'_>> {
		Some(self.range().map(|i| Val::Num(f64::from(i))))
	}
}

#[derive(Debug, Trace, Clone)]
pub struct ReverseArray(pub ArrValue);
impl ArrayLike for ReverseArray {
	type Iter<'t> = Rev<UnknownArrayIter<'t>>;

	type IterLazy<'t> = Rev<UnknownArrayIterLazy<'t>>;

	type IterCheap<'t> = Rev<UnknownArrayIterCheap<'t>>;

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

	fn evaluated(&self) -> Result<Vec<Val>> {
		let mut v = self.0.evaluated()?;
		v.reverse();
		Ok(v)
	}

	fn iter(&self) -> Rev<UnknownArrayIter<'_>> {
		self.0.iter().rev()
	}

	fn iter_lazy(&self) -> Rev<UnknownArrayIterLazy<'_>> {
		self.0.iter_lazy().rev()
	}

	fn iter_cheap(&self) -> Option<Rev<UnknownArrayIterCheap<'_>>> {
		Some(self.0.iter_cheap()?.rev())
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
type MappedArrayIter<'t> = impl DoubleEndedIterator<Item = Result<Val>> + ExactSizeIterator + 't;
type MappedArrayLazyIter<'t> = impl DoubleEndedIterator<Item = Thunk<Val>> + ExactSizeIterator + 't;
type MappedArrayCheapIter<'t> = iter::Empty<Val>;
impl ArrayLike for MappedArray {
	type Iter<'t> = MappedArrayIter<'t>;
	type IterLazy<'t> = MappedArrayLazyIter<'t>;
	type IterCheap<'t> = MappedArrayCheapIter<'t>;

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

		let ArrayThunk::Waiting(_) = replace(&mut self.0.cached.borrow_mut()[index], ArrayThunk::Pending) else {
			unreachable!()
		};

		let val = self
			.0
			.inner
			.get(index)
			.transpose()
			.expect("index checked")
			.and_then(|r| self.0.mapper.evaluate_simple(&(Any(r),)));

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

		Some(Thunk::new(tb!(ArrayElement {
			arr_thunk: self.clone(),
			index,
		})))
	}

	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}

	fn evaluated(&self) -> Result<Vec<Val>> {
		self.iter().collect()
	}

	fn iter(&self) -> MappedArrayIter<'_> {
		(0..self.len()).map(|i| self.get(i).transpose().expect("length checked"))
	}

	fn iter_lazy(&self) -> MappedArrayLazyIter<'_> {
		(0..self.len()).map(|i| self.get_lazy(i).expect("length checked"))
	}

	fn iter_cheap(&self) -> Option<Self::IterCheap<'_>> {
		None
	}
}
// impl MappedArray

macro_rules! impl_iter_enum {
	($n:ident => $v:ident) => {
		pub enum $n<'t> {
			Bytes(<BytesArray as ArrayLike>::$v<'t>),
			Expr(<ExprArray as ArrayLike>::$v<'t>),
			Lazy(<LazyArray as ArrayLike>::$v<'t>),
			Eager(<EagerArray as ArrayLike>::$v<'t>),
			Range(<RangeArray as ArrayLike>::$v<'t>),
			Slice(Box<<SliceArray as ArrayLike>::$v<'t>>),
			Extended(Box<<ExtendedArray as ArrayLike>::$v<'t>>),
			Reverse(Box<<ReverseArray as ArrayLike>::$v<'t>>),
			Mapped(Box<<MappedArray as ArrayLike>::$v<'t>>),
		}
	};
}

macro_rules! pass {
	($t:ident.$m:ident($($ident:ident),*)) => {
		match $t {
			Self::Bytes(e) => e.$m($($ident)*),
			Self::Expr(e) => e.$m($($ident)*),
			Self::Lazy(e) => e.$m($($ident)*),
			Self::Eager(e) => e.$m($($ident)*),
			Self::Range(e) => e.$m($($ident)*),
			Self::Slice(e) => e.$m($($ident)*),
			Self::Extended(e) => e.$m($($ident)*),
			Self::Reverse(e) => e.$m($($ident)*),
			Self::Mapped(e) => e.$m($($ident)*),
		}
	};
}
pub(super) use pass;

macro_rules! pass_iter_call {
	($t:ident.$c:ident $(in $wrap:ident)? => $e:ident) => {
		match $t {
			ArrValue::Bytes(e) => $e::Bytes($($wrap!)?(e.$c())),
			ArrValue::Lazy(e) => $e::Lazy($($wrap!)?(e.$c())),
			ArrValue::Expr(e) => $e::Expr($($wrap!)?(e.$c())),
			ArrValue::Eager(e) => $e::Eager($($wrap!)?(e.$c())),
			ArrValue::Range(e) => $e::Range($($wrap!)?(e.$c())),
			ArrValue::Slice(e) => $e::Slice(Box::new($($wrap!)?(e.$c()))),
			ArrValue::Extended(e) => $e::Extended(Box::new($($wrap!)?(e.$c()))),
			ArrValue::Reverse(e) => $e::Reverse(Box::new($($wrap!)?(e.$c()))),
			ArrValue::Mapped(e) => $e::Mapped(Box::new($($wrap!)?(e.$c()))),
		}
	};
}
pub(super) use pass_iter_call;

macro_rules! impl_iter {
	($t:ident => $out:ty) => {
		impl Iterator for $t<'_> {
			type Item = $out;

			fn next(&mut self) -> Option<Self::Item> {
				pass!(self.next())
			}
			fn nth(&mut self, count: usize) -> Option<Self::Item> {
				pass!(self.nth(count))
			}
			fn size_hint(&self) -> (usize, Option<usize>) {
				pass!(self.size_hint())
			}
		}
		impl DoubleEndedIterator for $t<'_> {
			fn next_back(&mut self) -> Option<Self::Item> {
				pass!(self.next_back())
			}
			fn nth_back(&mut self, count: usize) -> Option<Self::Item> {
				pass!(self.nth_back(count))
			}
		}
		impl ExactSizeIterator for $t<'_> {
			fn len(&self) -> usize {
				match self {
					Self::Bytes(e) => e.len(),
					Self::Expr(e) => e.len(),
					Self::Lazy(e) => e.len(),
					Self::Eager(e) => e.len(),
					Self::Range(e) => e.len(),
					Self::Slice(e) => e.len(),
					Self::Extended(e) => {
						e.size_hint().1.expect("overflow is checked in constructor")
					}
					Self::Reverse(e) => e.len(),
					Self::Mapped(e) => e.len(),
				}
			}
		}
	};
}

impl_iter_enum!(UnknownArrayIter => Iter);
impl_iter_enum!(UnknownArrayIterLazy => IterLazy);
impl_iter_enum!(UnknownArrayIterCheap => IterCheap);
impl_iter!(UnknownArrayIter => Result<Val>);
impl_iter!(UnknownArrayIterLazy => Thunk<Val>);
impl_iter!(UnknownArrayIterCheap => Val);
