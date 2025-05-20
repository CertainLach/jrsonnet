use std::{any::Any, cell::RefCell, fmt::Debug, mem::replace};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::{IBytes, IStr};
use jrsonnet_parser::{LocExpr, Visibility};

use super::ArrValue;
use crate::{
	error::ErrorKind::InfiniteRecursionDetected,
	evaluate,
	function::{CallLocation, FuncVal, PreparedFuncVal},
	strings,
	typed::IntoUntyped,
	val::NumValue,
	BindingValue, Context, EnumFieldsHandler, Error, FieldIndex, ObjValue, ObjectLayer, Result,
	SuperDepth, Thunk, Val, ValueProcess,
};

pub trait ArrayLike: Any + Trace + Debug {
	fn len(&self) -> usize;
	fn is_empty(&self) -> bool {
		self.len() == 0
	}
	fn get(&self, index: usize) -> Result<Option<Val>>;
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>>;
	fn get_cheap(&self, index: usize) -> Option<Val>;

	fn is_cheap(&self) -> bool;

	#[doc(hidden)]
	// Specialization for passing ArrayLike as ArrValue, to get ArrValue back
	fn internal_owned(&self) -> Option<ArrValue> {
		None
	}
}

impl ArrayLike for () {
	fn len(&self) -> usize {
		0
	}

	fn get(&self, _index: usize) -> Result<Option<Val>> {
		Ok(None)
	}

	fn get_lazy(&self, _index: usize) -> Option<Thunk<Val>> {
		None
	}

	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}

	fn is_cheap(&self) -> bool {
		true
	}
}

#[derive(Debug, Trace)]
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
		std::iter::repeat_n((), (self.to - self.from) as usize)
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
	fn is_cheap(&self) -> bool {
		self.inner.is_cheap()
	}
}

impl ArrayLike for IBytes {
	fn len(&self) -> usize {
		self.as_slice().len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		Ok(self.get_cheap(index))
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.get_cheap(index).map(Thunk::evaluated)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		self.as_slice().get(index).map(|v| Val::Num((*v).into()))
	}
	fn is_cheap(&self) -> bool {
		true
	}
}

#[derive(Debug, Trace, Clone)]
enum ArrayThunk<T: 'static + Trace> {
	Computed(Val),
	Errored(Error),
	Waiting(T),
	Pending,
}

#[derive(Debug, Trace, Clone)]
pub struct ExprArray {
	ctx: Context,
	cached: Cc<RefCell<Vec<ArrayThunk<LocExpr>>>>,
}
impl ExprArray {
	pub fn new(ctx: Context, items: impl IntoIterator<Item = LocExpr>) -> Self {
		Self {
			ctx,
			cached: Cc::new(RefCell::new(
				items.into_iter().map(ArrayThunk::Waiting).collect(),
			)),
		}
	}
}
impl ArrayLike for ExprArray {
	fn len(&self) -> usize {
		self.cached.borrow().len()
	}
	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index >= self.len() {
			return Ok(None);
		}
		match &self.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Ok(Some(c.clone())),
			ArrayThunk::Errored(e) => return Err(e.clone()),
			ArrayThunk::Pending => return Err(InfiniteRecursionDetected.into()),
			ArrayThunk::Waiting(..) => {}
		}

		let ArrayThunk::Waiting(expr) =
			replace(&mut self.cached.borrow_mut()[index], ArrayThunk::Pending)
		else {
			unreachable!()
		};

		let new_value = match evaluate(&self.ctx, &expr) {
			Ok(v) => v,
			Err(e) => {
				self.cached.borrow_mut()[index] = ArrayThunk::Errored(e.clone());
				return Err(e);
			}
		};
		self.cached.borrow_mut()[index] = ArrayThunk::Computed(new_value.clone());
		Ok(Some(new_value))
	}
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		if index >= self.len() {
			return None;
		}
		match &self.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Some(Thunk::evaluated(c.clone())),
			ArrayThunk::Errored(e) => return Some(Thunk::errored(e.clone())),
			ArrayThunk::Waiting(_) | ArrayThunk::Pending => {}
		}

		let arr_thunk = self.clone();
		Some(Thunk!(move || {
			arr_thunk.get(index).transpose().expect("index checked")
		}))
	}
	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}
	fn is_cheap(&self) -> bool {
		false
	}
}

#[derive(Trace, Debug)]
pub struct ExtendedArray<A: ArrayLike, B: ArrayLike> {
	pub a: A,
	pub b: B,
	split: usize,
	len: usize,
}
impl<A: ArrayLike, B: ArrayLike> ExtendedArray<A, B> {
	pub fn new(a: A, b: B) -> Self {
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
impl<A: ArrayLike, B: ArrayLike> ArrayLike for ExtendedArray<A, B> {
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
	fn is_cheap(&self) -> bool {
		self.a.is_cheap() && self.b.is_cheap()
	}
}

impl<T: IntoUntyped + Clone + Debug + Trace + 'static> ArrayLike for Vec<T> {
	fn len(&self) -> usize {
		self.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		let Some(elem) = self.as_slice().get(index).cloned() else {
			return Ok(None);
		};

		IntoUntyped::into_untyped(elem).map(Some)
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		self.as_slice()
			.get(index)
			.cloned()
			.map(IntoUntyped::into_lazy_untyped)
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		IntoUntyped::into_untyped_cheap(self.as_slice().get(index).cloned()?)
	}
	fn is_cheap(&self) -> bool {
		T::provides_cheap()
	}
}

/// Inclusive range type
#[derive(Debug, Trace, PartialEq, Eq)]
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
	fn range(&self) -> impl ExactSizeIterator<Item = i32> + DoubleEndedIterator + use<> {
		WithExactSize(
			self.start..=self.end,
			#[expect(
				clippy::cast_sign_loss,
				reason = "sign does not matter for difference calculation"
			)]
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
		self.range().nth(index).map(|i| Val::Num(i.into()))
	}
	fn is_cheap(&self) -> bool {
		true
	}
}

#[derive(Debug, Trace)]
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
	fn is_cheap(&self) -> bool {
		self.0.is_cheap()
	}
}

#[derive(Trace, Debug, Clone)]
pub struct MappedArray<const WITH_INDEX: bool> {
	inner: ArrValue,
	cached: Cc<RefCell<Vec<ArrayThunk<()>>>>,
	mapper: PreparedFuncVal,
}
impl<const WITH_INDEX: bool> MappedArray<WITH_INDEX> {
	pub fn new(inner: ArrValue, mapper: FuncVal) -> Result<Self> {
		let len = inner.len();
		let mapper = PreparedFuncVal::new(mapper, if WITH_INDEX { 2 } else { 1 }, &[])?;
		Ok(Self {
			inner,
			cached: Cc::new(RefCell::new(vec![ArrayThunk::Waiting(()); len])),
			mapper,
		})
	}
	fn evaluate(&self, index: usize, value: Val) -> Result<Val> {
		let loc = CallLocation::native();
		let value = BindingValue::Value(value);
		if WITH_INDEX {
			self.mapper.call(
				loc,
				&[
					BindingValue::Value(Val::Num(
						NumValue::new(index as f64).expect("index can't be that large"),
					)),
					value,
				],
				&[],
			)
		} else {
			self.mapper.call(loc, &[value], &[])
		}
	}
}
impl<const WITH_INDEX: bool> ArrayLike for MappedArray<WITH_INDEX> {
	fn len(&self) -> usize {
		self.cached.borrow().len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index >= self.len() {
			return Ok(None);
		}
		match &self.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Ok(Some(c.clone())),
			ArrayThunk::Errored(e) => return Err(e.clone()),
			ArrayThunk::Pending => return Err(InfiniteRecursionDetected.into()),
			ArrayThunk::Waiting(..) => {}
		}

		let ArrayThunk::Waiting(()) =
			replace(&mut self.cached.borrow_mut()[index], ArrayThunk::Pending)
		else {
			unreachable!()
		};

		let val = self
			.inner
			.get(index)
			.transpose()
			.expect("index checked")
			.and_then(|r| self.evaluate(index, r));

		let new_value = match val {
			Ok(v) => v,
			Err(e) => {
				self.cached.borrow_mut()[index] = ArrayThunk::Errored(e.clone());
				return Err(e);
			}
		};
		self.cached.borrow_mut()[index] = ArrayThunk::Computed(new_value.clone());
		Ok(Some(new_value))
	}
	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		if index >= self.len() {
			return None;
		}
		match &self.cached.borrow()[index] {
			ArrayThunk::Computed(c) => return Some(Thunk::evaluated(c.clone())),
			ArrayThunk::Errored(e) => return Some(Thunk::errored(e.clone())),
			ArrayThunk::Waiting(()) | ArrayThunk::Pending => {}
		}

		let arr_thunk = self.clone();
		Some(Thunk!(move || {
			arr_thunk.get(index).transpose().expect("index checked")
		}))
	}

	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}
	fn is_cheap(&self) -> bool {
		false
	}
}

#[derive(Trace, Debug)]
pub struct RepeatedSingleArray<T>
where
	T: IntoUntyped + Trace,
{
	pub elem: T,
	pub len: usize,
}
impl<T> ArrayLike for RepeatedSingleArray<T>
where
	T: IntoUntyped + Trace + Clone,
{
	fn len(&self) -> usize {
		self.len
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index >= self.len {
			return Ok(None);
		}
		Some(T::into_untyped(self.elem.clone())).transpose()
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		if index >= self.len {
			return None;
		}
		Some(T::into_lazy_untyped(self.elem.clone()))
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		if index >= self.len {
			return None;
		}
		T::into_untyped_cheap(self.elem.clone())
	}

	fn is_cheap(&self) -> bool {
		T::provides_cheap()
	}
}

#[derive(Trace, Debug)]
pub struct RepeatedArray {
	data: ArrValue,
	repeats: usize,
	total_len: usize,
}
impl RepeatedArray {
	pub fn new(data: ArrValue, repeats: usize) -> Option<Self> {
		let total_len = data.len().checked_mul(repeats)?;
		Some(Self {
			data,
			repeats,
			total_len,
		})
	}
}

impl ArrayLike for RepeatedArray {
	fn len(&self) -> usize {
		self.total_len
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		if index > self.total_len {
			return Ok(None);
		}
		self.data.get(index % self.data.len())
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		if index > self.total_len {
			return None;
		}
		self.data.get_lazy(index % self.data.len())
	}

	fn get_cheap(&self, index: usize) -> Option<Val> {
		if index > self.total_len {
			return None;
		}
		self.data.get_cheap(index % self.data.len())
	}
	fn is_cheap(&self) -> bool {
		self.data.is_cheap()
	}
}

#[derive(Trace, Debug)]
pub struct PickObjectValues {
	obj: ObjValue,
	keys: Vec<IStr>,
}

impl PickObjectValues {
	pub fn new(obj: ObjValue, keys: Vec<IStr>) -> Self {
		Self { obj, keys }
	}
}

impl ArrayLike for PickObjectValues {
	fn len(&self) -> usize {
		self.keys.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		let Some(key) = self.keys.as_slice().get(index) else {
			return Ok(None);
		};
		Ok(Some(self.obj.get_or_bail(key.clone())?))
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		let key = self.keys.as_slice().get(index)?;
		Some(self.obj.get_lazy_or_bail(key.clone()))
	}

	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}

	fn is_cheap(&self) -> bool {
		false
	}
}

#[derive(Trace, Debug)]
pub struct PickObjectKeyValues {
	obj: ObjValue,
	keys: Vec<IStr>,
}

impl PickObjectKeyValues {
	pub fn new(obj: ObjValue, keys: Vec<IStr>) -> Self {
		Self { obj, keys }
	}
}

strings! {
	s_key: "key",
	s_value: "value",
}

#[derive(Debug, Trace)]
pub struct KeyValue {
	key: IStr,
	value: Thunk<Val>,
}
impl ObjectLayer for KeyValue {
	fn enum_fields_core(
		&self,
		super_depth: &mut SuperDepth,
		handler: &mut EnumFieldsHandler<'_>,
	) -> bool {
		let mut i = FieldIndex::default();
		if !handler(*super_depth, i, s_key(), Visibility::Normal) {
			return false;
		}
		i.next();
		if !handler(*super_depth, i, s_value(), Visibility::Normal) {
			return false;
		}
		true
	}

	fn has_field_include_hidden(&self, name: IStr) -> bool {
		name == s_key() || name == s_value()
	}

	fn get_for(
		&self,
		key: IStr,
		_sup_this: crate::SupThis,
		_do_cache: &mut bool,
	) -> Result<Option<(Val, crate::ValueProcess)>> {
		Ok(if key == s_key() {
			Some((Val::string(self.key.clone()), ValueProcess { add: false }))
		} else if key == s_value() {
			Some((self.value.evaluate()?, ValueProcess { add: false }))
		} else {
			None
		})
	}

	fn field_visibility(&self, field: IStr) -> Option<Visibility> {
		if field == s_key() || field == s_value() {
			Some(Visibility::Normal)
		} else {
			None
		}
	}

	fn run_assertions_raw(&self, _sup_this: crate::SupThis) -> Result<()> {
		Ok(())
	}
}

impl ArrayLike for PickObjectKeyValues {
	fn len(&self) -> usize {
		self.keys.len()
	}

	fn get(&self, index: usize) -> Result<Option<Val>> {
		let Some(key) = self.keys.as_slice().get(index) else {
			return Ok(None);
		};
		Ok(Some(Val::object(KeyValue {
			key: key.clone(),
			value: Thunk::evaluated(self.obj.get_or_bail(key.clone())?),
		})))
	}

	fn get_lazy(&self, index: usize) -> Option<Thunk<Val>> {
		let key = self.keys.as_slice().get(index)?;
		// Nothing can fail in the key part, yet value is still
		// lazy-evaluated
		Some(Thunk::evaluated(Val::object(KeyValue {
			key: key.clone(),
			value: self.obj.get_lazy_or_bail(key.clone()),
		})))
	}

	fn get_cheap(&self, _index: usize) -> Option<Val> {
		None
	}

	fn is_cheap(&self) -> bool {
		false
	}
}
