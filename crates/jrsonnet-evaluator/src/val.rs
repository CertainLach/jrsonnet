use std::{
	cell::RefCell,
	cmp::Ordering,
	fmt::{self, Debug, Display},
	marker::PhantomData,
	mem::replace,
	num::NonZeroU32,
	ops::Deref,
	rc::Rc,
};

use educe::Educe;
use jrsonnet_gcmodule::{cc_dyn, Cc, Trace, Tracer};
use jrsonnet_interner::IStr;
pub use jrsonnet_macros::Thunk;
use jrsonnet_types::ValType;
use thiserror::Error;

pub use crate::arr::{ArrValue, ArrayLike};
use crate::{
	bail, debug_cyclic,
	error::{Error, ErrorKind::*},
	function::FuncVal,
	gc::GcHashMap,
	manifest::{ManifestFormat, ToStringFormat},
	typed::{BoundedUsize, MAX_SAFE_INTEGER, MIN_SAFE_INTEGER},
	ObjValue, ObjectLayer, Result, SupThis, Unbound, WeakSupThis,
};

pub trait ThunkValueOnce: Trace + Debug {
	type Output;
	fn get(self) -> Result<Self::Output>;
}
#[repr(transparent)]
#[derive(Educe)]
#[educe(Debug)]
pub struct Thunk<O: Debug>(#[educe(Debug(method(debug_cyclic)))] Cc<dyn ThunkValue<Output = O>>);

impl<O: Debug> Clone for Thunk<O> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<O: 'static + Debug> Trace for Thunk<O> {
	fn trace(&self, tracer: &mut Tracer<'_>) {
		Cc::<dyn ThunkValue<Output = O>>::trace(&self.0, tracer);
	}
	#[inline]
	fn is_type_tracked() -> bool {
		true
	}
}

cc_dyn!(CcThunkInternal<O>, ThunkValue<Output = O>);

impl<O: Trace + Clone + Debug> Thunk<O> {
	pub fn new_cached(f: impl ThunkValueOnce<Output = O> + 'static) -> Self {
		Self::new(ThunkValueCached(RefCell::new(
			ThunkValueCachedInner::Waiting(f),
		)))
	}
	pub fn new<T: ThunkValue<Output = O> + Trace>(input: T) -> Self {
		let internal = CcThunkInternal::new(input);
		Self(internal.0)
	}
	pub fn evaluated(v: O) -> Self {
		#[derive(Trace, Debug)]
		struct Inner<T: Trace>(T);
		impl<T: Trace + 'static + Clone + Debug> ThunkValue for Inner<T> {
			type Output = T;

			fn get(&self) -> Result<Self::Output> {
				Ok(self.0.clone())
			}
		}
		Self::new(Inner(v))
	}
	pub fn errored(v: Error) -> Self {
		#[derive(Trace, Debug)]
		#[trace(skip)]
		struct Inner<O: Trace>(Error, PhantomData<O>);
		impl<O: Trace + Debug> ThunkValue for Inner<O> {
			type Output = O;

			fn get(&self) -> Result<Self::Output> {
				Err(self.0.clone())
			}
		}
		Self::new(Inner(v, PhantomData))
	}
	pub fn result(v: Result<O>) -> Self {
		#[derive(Trace, Debug)]
		struct Inner<T: Trace>(Result<T>);
		impl<T: Trace + 'static + Clone + Debug> ThunkValue for Inner<T> {
			type Output = T;

			fn get(&self) -> Result<Self::Output> {
				self.0.clone()
			}
		}
		Self::new(Inner(v))
	}

	/// Evaluate thunk, or return cached value
	///
	/// # Errors
	///
	/// - Lazy value evaluation returned error
	/// - This method was called during inner value evaluation
	pub fn evaluate(&self) -> Result<O> {
		self.0.get()
	}
}
pub trait ThunkValue: Trace + Debug {
	type Output;
	fn get(&self) -> Result<Self::Output>;
}

#[derive(Trace, Debug)]
enum ThunkValueCachedInner<T: Trace + 'static, I: Trace + 'static> {
	Computed(T),
	Waiting(I),
	Errored(Error),
	Pending,
}
#[derive(Trace, Debug)]
struct ThunkValueCached<T: Trace + 'static + Debug, I: Trace + 'static + Debug>(
	RefCell<ThunkValueCachedInner<T, I>>,
);
impl<T: Trace + Clone + Debug, I: Trace + Debug> ThunkValue for ThunkValueCached<T, I>
where
	I: ThunkValueOnce<Output = T>,
{
	type Output = T;

	fn get(&self) -> Result<Self::Output> {
		match &*self.0.borrow() {
			ThunkValueCachedInner::Computed(v) => return Ok(v.clone()),
			ThunkValueCachedInner::Pending => return Err(InfiniteRecursionDetected.into()),
			ThunkValueCachedInner::Waiting(..) => (),
			ThunkValueCachedInner::Errored(error) => return Err(error.clone()),
		}
		let ThunkValueCachedInner::Waiting(value) =
			replace(&mut *self.0.borrow_mut(), ThunkValueCachedInner::Pending)
		else {
			unreachable!();
		};
		let new_value = value.get().inspect_err(|e| {
			*self.0.borrow_mut() = ThunkValueCachedInner::Errored(e.clone());
		})?;
		*self.0.borrow_mut() = ThunkValueCachedInner::Computed(new_value.clone());
		Ok(new_value)
	}
}

#[derive(Trace, educe::Educe)]
#[educe(Debug(bound(D: Debug)))]
pub struct ThunkValueClosure<D: Trace, O: 'static> {
	env: D,
	// Carries no data, as it is not a real closure, all the
	// captured environment is stored in `env` field.
	#[trace(skip)]
	closure: fn(D) -> Result<O>,
}
impl<D: Trace, O: 'static> ThunkValueClosure<D, O> {
	pub fn new(env: D, closure: fn(D) -> Result<O>) -> Self {
		Self { env, closure }
	}
}
impl<D: Trace + Debug, O: 'static> ThunkValueOnce for ThunkValueClosure<D, O> {
	type Output = O;

	fn get(self) -> Result<Self::Output> {
		(self.closure)(self.env)
	}
}

pub trait ThunkMapper<Input: Debug>: Trace + Debug {
	type Output: Debug;
	fn map(self, from: Input) -> Result<Self::Output>;
}
impl<Input> Thunk<Input>
where
	Input: Trace + Clone + Debug,
{
	pub fn map<M>(self, mapper: M) -> Thunk<M::Output>
	where
		M: ThunkMapper<Input>,
		M::Output: Trace + Clone + Debug,
	{
		let inner = self;
		Thunk!(move || {
			let value = inner.evaluate()?;
			let result = mapper.map(value)?;
			Ok(result)
		})
	}
}

impl<T: Trace + 'static + Clone + Debug> From<Result<T>> for Thunk<T> {
	fn from(value: Result<T>) -> Self {
		Self::result(value)
	}
}

#[derive(Trace, Clone)]
pub struct CachedUnbound<I, T>
where
	I: Unbound<Bound = T>,
	T: Trace,
{
	cache: Cc<RefCell<GcHashMap<WeakSupThis, T>>>,
	value: I,
}
impl<I: Unbound<Bound = T>, T: Trace> CachedUnbound<I, T> {
	pub fn new(value: I) -> Self {
		Self {
			cache: Cc::new(RefCell::new(GcHashMap::new())),
			value,
		}
	}
}
impl<I: Unbound<Bound = T>, T: Clone + Trace> Unbound for CachedUnbound<I, T> {
	type Bound = T;
	fn bind(&self, sup_this: SupThis) -> Result<T> {
		let cache_key = sup_this.clone().downgrade();
		{
			if let Some(t) = self.cache.borrow().get(&cache_key) {
				return Ok(t.clone());
			}
		}
		let bound = self.value.bind(sup_this)?;

		{
			let mut cache = self.cache.borrow_mut();
			cache.insert(cache_key, bound.clone());
		}

		Ok(bound)
	}
}

/// Represents a Jsonnet value, which can be sliced or indexed (string or array).
#[derive(Debug)]
pub enum Indexable {
	/// String.
	Str(IStr),
	/// Array.
	Arr(ArrValue),
}
impl Indexable {
	pub fn len(&self) -> usize {
		match self {
			Self::Str(s) => s.chars().count(),
			Self::Arr(a) => a.len(),
		}
	}
	pub fn is_empty(&self) -> bool {
		match self {
			Self::Str(s) => s.is_empty(),
			Self::Arr(s) => s.is_empty(),
		}
	}

	pub fn to_array(self) -> ArrValue {
		match self {
			Self::Str(s) => ArrValue::new(s.chars().collect::<Vec<_>>()),
			Self::Arr(arr) => arr,
		}
	}
	pub fn index(&self, index: usize) -> Result<Option<Val>> {
		match self {
			Self::Str(s) => {
				let Some(ch) = s.chars().skip(index).take(1).next() else {
					return Ok(None);
				};
				Ok(Some(Val::string(ch)))
			}
			Self::Arr(a) => a.get(index),
		}
	}
	pub fn try_index(self, index: usize) -> Result<Val> {
		self.index(index)?
			.ok_or_else(|| IndexBoundsError(index, self.len()).into())
	}
	/// Slice the value.
	///
	/// # Implementation
	///
	/// For strings, will create a copy of specified interval.
	///
	/// For arrays, nothing will be copied on this call, instead [`ArrValue::Slice`] view will be returned.
	pub fn slice(
		self,
		index: Option<i32>,
		end: Option<i32>,
		step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
	) -> Result<Self> {
		match &self {
			Self::Str(s) => {
				let mut computed_len = None;
				let mut get_len = || {
					computed_len.map_or_else(
						|| {
							let len = s.chars().count();
							let _ = computed_len.insert(len);
							len
						},
						|len| len,
					)
				};
				let mut get_idx = |pos: Option<i32>, default| {
					match pos {
						#[expect(
							clippy::cast_sign_loss,
							reason = "value is always positive due to guard and inversion"
						)]
						Some(v) if v < 0 => get_len().saturating_sub((-v) as usize),
						// No need to clamp, as iterator interface is used
						#[expect(
							clippy::cast_sign_loss,
							reason = "value is always positive, as negatives are already handled"
						)]
						Some(v) => v as usize,
						None => default,
					}
				};

				let index = get_idx(index, 0);
				let end = get_idx(end, usize::MAX);
				let step = step.as_deref().copied().unwrap_or(1);

				if index >= end {
					return Ok(Self::Str("".into()));
				}

				Ok(Self::Str(
					(s.chars()
						.skip(index)
						.take(end - index)
						.step_by(step)
						.collect::<String>())
					.into(),
				))
			}
			Self::Arr(arr) => Ok(Self::Arr(arr.clone().slice(
				index,
				end,
				step.map(|v| NonZeroU32::new(v.value() as u32).expect("bounded != 0")),
			))),
		}
	}
}

#[derive(Debug, Clone, Trace)]
pub enum StrValue {
	Flat(IStr),
	Tree(Rc<(StrValue, StrValue, usize)>),
}
impl StrValue {
	pub fn concat(a: Self, b: Self) -> Self {
		// TODO: benchmark for an optimal value, currently just a arbitrary choice
		const STRING_EXTEND_THRESHOLD: usize = 100;

		if a.is_empty() {
			b
		} else if b.is_empty() {
			a
		} else if a.len() + b.len() < STRING_EXTEND_THRESHOLD {
			Self::Flat(format!("{a}{b}").into())
		} else {
			let len = a.len() + b.len();
			Self::Tree(Rc::new((a, b, len)))
		}
	}
	pub fn into_flat(self) -> IStr {
		#[cold]
		fn write_buf(s: &StrValue, out: &mut String) {
			match s {
				StrValue::Flat(f) => out.push_str(f),
				StrValue::Tree(t) => {
					write_buf(&t.0, out);
					write_buf(&t.1, out);
				}
			}
		}
		match self {
			Self::Flat(f) => f,
			Self::Tree(_) => {
				let mut buf = String::with_capacity(self.len());
				write_buf(&self, &mut buf);
				buf.into()
			}
		}
	}
	pub fn len(&self) -> usize {
		match self {
			Self::Flat(v) => v.len(),
			Self::Tree(t) => t.2,
		}
	}
	pub fn is_empty(&self) -> bool {
		match self {
			Self::Flat(v) => v.is_empty(),
			// Can't create non-flat empty string
			Self::Tree(_) => false,
		}
	}
}
impl<T> From<T> for StrValue
where
	IStr: From<T>,
{
	fn from(value: T) -> Self {
		Self::Flat(IStr::from(value))
	}
}
impl Display for StrValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Flat(v) => write!(f, "{v}"),
			Self::Tree(t) => {
				write!(f, "{}", t.0)?;
				write!(f, "{}", t.1)
			}
		}
	}
}
impl PartialEq for StrValue {
	// False positive, into_flat returns not StrValue, but IStr, thus no infinite recursion here.
	#[allow(clippy::unconditional_recursion)]
	fn eq(&self, other: &Self) -> bool {
		let a = self.clone().into_flat();
		let b = other.clone().into_flat();
		a == b
	}
}
impl Eq for StrValue {}
impl PartialOrd for StrValue {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl Ord for StrValue {
	fn cmp(&self, other: &Self) -> Ordering {
		let a = self.clone().into_flat();
		let b = other.clone().into_flat();
		a.cmp(&b)
	}
}

/// Represents jsonnet number
/// Jsonnet numbers are finite f64, with NaNs disallowed
#[derive(Trace, Clone, Copy)]
#[repr(transparent)]
pub struct NumValue(f64);
impl NumValue {
	/// Creates a [`NumValue`], if value is finite and not NaN
	pub fn new(v: f64) -> Option<Self> {
		if !v.is_finite() {
			return None;
		}
		Some(Self(v))
	}
	/// Creates a [`NumValue`], if i64 can be represented as f64 lossless.
	#[allow(
		clippy::cast_precision_loss,
		reason = "no loss happens here, range is checked"
	)]
	#[inline]
	pub fn new_safe_int(v: i64) -> Result<Self, ConvertNumValueError> {
		if v < MIN_SAFE_INTEGER {
			return Err(ConvertNumValueError::Underflow);
		}
		if v > MAX_SAFE_INTEGER {
			return Err(ConvertNumValueError::Overflow);
		}
		Ok(Self(v as f64))
	}
	/// Creates a [`NumValue`], if i64 can be represented as f64 lossless.
	#[allow(
		clippy::cast_precision_loss,
		reason = "no loss happens here, range is checked"
	)]
	#[inline]
	pub fn new_safe_uint(v: u64) -> Result<Self, ConvertNumValueError> {
		if v > MAX_SAFE_INTEGER as u64 {
			return Err(ConvertNumValueError::Overflow);
		}
		Ok(Self(v as f64))
	}
	#[allow(clippy::float_cmp, reason = "comparing integer with 0.0")]
	#[allow(
		clippy::cast_precision_loss,
		reason = "{MIN,MAX}_SAFE_INTEGER is in f64 range"
	)]
	#[inline]
	pub fn get_safe_int(&self) -> Result<i64, ConvertNumValueError> {
		let value = self.get();
		if value.trunc() != value {
			return Err(ConvertNumValueError::FractionalInt);
		}
		if value < MIN_SAFE_INTEGER as f64 {
			return Err(ConvertNumValueError::Underflow);
		}
		if value > MAX_SAFE_INTEGER as f64 {
			return Err(ConvertNumValueError::Overflow);
		}
		Ok(value as i64)
	}

	#[inline]
	pub const fn get(&self) -> f64 {
		self.0
	}

	pub fn get_index(&self) -> Result<usize> {
		let n = self.get();
		if n.fract() > f64::EPSILON {
			bail!(FractionalIndex)
		}
		if n < 0.0 {
			bail!(NegativeIndex);
		}
		#[expect(clippy::cast_sign_loss, reason = "value is not negative")]
		let nu = n as usize;
		Ok(nu)
	}
}
impl PartialEq for NumValue {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}
impl Eq for NumValue {}
impl Ord for NumValue {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering {
		// Can't use `total_cmp`: its behavior for `-0` and `0`
		// is not following wanted.
		unsafe { self.0.partial_cmp(&other.0).unwrap_unchecked() }
	}
}
impl PartialOrd for NumValue {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl Debug for NumValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.0, f)
	}
}
impl Display for NumValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.0, f)
	}
}
impl Deref for NumValue {
	type Target = f64;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
macro_rules! impl_num {
	($($ty:ty),+) => {$(
		impl From<$ty> for NumValue {
			#[inline]
			fn from(value: $ty) -> Self {
				Self(value.into())
			}
		}
	)+};
}
impl_num!(i8, u8, i16, u16, i32, u32);

#[derive(Clone, Copy, Debug, Error, Trace)]
pub enum ConvertNumValueError {
	#[error("overflow")]
	Overflow,
	#[error("underflow")]
	Underflow,
	#[error("non-finite")]
	NonFinite,
	#[error("number with fractional part can't be converted to integer")]
	FractionalInt,
}
impl From<ConvertNumValueError> for Error {
	fn from(e: ConvertNumValueError) -> Self {
		Self::new(ConvertNumValue(e))
	}
}

macro_rules! impl_try_signed {
	($($ty:ty),+) => {$(
		impl TryFrom<$ty> for NumValue {
			type Error = ConvertNumValueError;
			#[inline]
			fn try_from(value: $ty) -> Result<Self, ConvertNumValueError> {
				Self::new_safe_int(value as i64)
			}
		}
	)+};
}
macro_rules! impl_try_unsigned {
	($($ty:ty),+) => {$(
		impl TryFrom<$ty> for NumValue {
			type Error = ConvertNumValueError;
			#[inline]
			fn try_from(value: $ty) -> Result<Self, ConvertNumValueError> {
				Self::new_safe_uint(value as u64)
			}
		}
	)+};
}
impl_try_signed!(isize, i64);
impl_try_unsigned!(usize, u64);

impl TryFrom<f64> for NumValue {
	type Error = ConvertNumValueError;

	#[inline]
	fn try_from(value: f64) -> Result<Self, Self::Error> {
		Self::new(value).ok_or(ConvertNumValueError::NonFinite)
	}
}
impl TryFrom<f32> for NumValue {
	type Error = ConvertNumValueError;

	#[inline]
	fn try_from(value: f32) -> Result<Self, Self::Error> {
		Self::new(f64::from(value)).ok_or(ConvertNumValueError::NonFinite)
	}
}

/// Represents any valid Jsonnet value.
#[derive(Debug, Clone, Trace, Default)]
pub enum Val {
	/// Represents a Jsonnet boolean.
	Bool(bool),
	/// Represents a Jsonnet null value.
	#[default]
	Null,
	/// Represents a Jsonnet string.
	Str(StrValue),
	/// Represents a Jsonnet number.
	/// Should be finite, and not NaN
	/// This restriction isn't enforced by enum, as enum field can't be marked as private
	Num(NumValue),
	/// Experimental bigint
	#[cfg(feature = "exp-bigint")]
	BigInt(#[trace(skip)] Box<num_bigint::BigInt>),
	/// Represents a Jsonnet array.
	Arr(ArrValue),
	/// Represents a Jsonnet object.
	Obj(ObjValue),
	/// Represents a Jsonnet function.
	Func(FuncVal),
}

#[cfg(target_pointer_width = "64")]
static_assertions::assert_eq_size!(Val, [u8; 24]);

impl From<Indexable> for Val {
	fn from(v: Indexable) -> Self {
		match v {
			Indexable::Str(s) => Self::string(s),
			Indexable::Arr(a) => Self::Arr(a),
		}
	}
}

impl Val {
	pub const fn as_bool(&self) -> Option<bool> {
		match self {
			Self::Bool(v) => Some(*v),
			_ => None,
		}
	}
	pub const fn as_null(&self) -> Option<()> {
		match self {
			Self::Null => Some(()),
			_ => None,
		}
	}
	pub const fn is_null(&self) -> bool {
		matches!(self, Self::Null)
	}
	pub fn as_str(&self) -> Option<IStr> {
		match self {
			Self::Str(s) => Some(s.clone().into_flat()),
			_ => None,
		}
	}
	pub const fn as_num(&self) -> Option<f64> {
		match self {
			Self::Num(n) => Some(n.get()),
			_ => None,
		}
	}
	pub fn as_arr(&self) -> Option<ArrValue> {
		match self {
			Self::Arr(a) => Some(a.clone()),
			_ => None,
		}
	}
	pub fn as_obj(&self) -> Option<ObjValue> {
		match self {
			Self::Obj(o) => Some(o.clone()),
			_ => None,
		}
	}
	pub fn as_func(&self) -> Option<FuncVal> {
		match self {
			Self::Func(f) => Some(f.clone()),
			_ => None,
		}
	}

	pub const fn value_type(&self) -> ValType {
		match self {
			Self::Str(..) => ValType::Str,
			Self::Num(..) => ValType::Num,
			#[cfg(feature = "exp-bigint")]
			Self::BigInt(..) => ValType::BigInt,
			Self::Arr(..) => ValType::Arr,
			Self::Obj(..) => ValType::Obj,
			Self::Bool(_) => ValType::Bool,
			Self::Null => ValType::Null,
			Self::Func(..) => ValType::Func,
		}
	}

	pub fn manifest(&self, format: impl ManifestFormat) -> Result<String> {
		fn manifest_dyn(val: &Val, manifest: &dyn ManifestFormat) -> Result<String> {
			manifest.manifest(val.clone())
		}
		manifest_dyn(self, &format)
	}

	pub fn to_string(&self) -> Result<IStr> {
		Ok(match self {
			Self::Bool(true) => "true".into(),
			Self::Bool(false) => "false".into(),
			Self::Null => "null".into(),
			Self::Str(s) => s.clone().into_flat(),
			_ => self.manifest(ToStringFormat).map(IStr::from)?,
		})
	}

	pub fn into_indexable(self) -> Result<Indexable> {
		Ok(match self {
			Self::Str(s) => Indexable::Str(s.into_flat()),
			Self::Arr(arr) => Indexable::Arr(arr),
			_ => bail!(ValueIsNotIndexable(self.value_type())),
		})
	}

	pub fn function(function: impl Into<FuncVal>) -> Self {
		Self::Func(function.into())
	}
	pub fn string(string: impl Into<StrValue>) -> Self {
		Self::Str(string.into())
	}
	pub fn num(num: impl Into<NumValue>) -> Self {
		Self::Num(num.into())
	}
	pub fn object(obj: impl ObjectLayer) -> Self {
		Self::Obj(ObjValue::new(obj))
	}
	pub fn array(arr: impl ArrayLike) -> Self {
		Self::Arr(ArrValue::new(arr))
	}
	pub fn try_num<V, E>(num: V) -> Result<Self, E>
	where
		NumValue: TryFrom<V, Error = E>,
	{
		Ok(Self::Num(num.try_into()?))
	}
}

impl From<IStr> for Val {
	fn from(value: IStr) -> Self {
		Self::string(value)
	}
}
impl From<String> for Val {
	fn from(value: String) -> Self {
		Self::string(value)
	}
}
impl From<&str> for Val {
	fn from(value: &str) -> Self {
		Self::string(value)
	}
}
impl From<ObjValue> for Val {
	fn from(value: ObjValue) -> Self {
		Self::Obj(value)
	}
}

const fn is_function_like(val: &Val) -> bool {
	matches!(val, Val::Func(_))
}

/// Native implementation of `std.primitiveEquals`
pub fn primitive_equals(val_a: &Val, val_b: &Val) -> Result<bool> {
	Ok(match (val_a, val_b) {
		(Val::Bool(a), Val::Bool(b)) => a == b,
		(Val::Null, Val::Null) => true,
		(Val::Str(a), Val::Str(b)) => a == b,
		(Val::Num(a), Val::Num(b)) => (a.get() - b.get()).abs() <= f64::EPSILON,
		#[cfg(feature = "exp-bigint")]
		(Val::BigInt(a), Val::BigInt(b)) => a == b,
		(Val::Arr(_), Val::Arr(_)) => {
			bail!("primitiveEquals operates on primitive types, got array")
		}
		(Val::Obj(_), Val::Obj(_)) => {
			bail!("primitiveEquals operates on primitive types, got object")
		}
		(a, b) if is_function_like(a) && is_function_like(b) => {
			bail!("cannot test equality of functions")
		}
		(_, _) => false,
	})
}

/// Native implementation of `std.equals`
pub fn equals(val_a: &Val, val_b: &Val) -> Result<bool> {
	if val_a.value_type() != val_b.value_type() {
		return Ok(false);
	}
	match (val_a, val_b) {
		(Val::Arr(a), Val::Arr(b)) => {
			if ArrValue::ptr_eq(a, b) {
				return Ok(true);
			}
			if a.len() != b.len() {
				return Ok(false);
			}
			for (a, b) in a.iter().zip(b.iter()) {
				if !equals(&a?, &b?)? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(Val::Obj(a), Val::Obj(b)) => {
			if ObjValue::ptr_eq(a, b) {
				return Ok(true);
			}
			let fields = a.fields(
				#[cfg(feature = "exp-preserve-order")]
				false,
			);
			if fields
				!= b.fields(
					#[cfg(feature = "exp-preserve-order")]
					false,
				) {
				return Ok(false);
			}
			for field in fields {
				if !equals(
					&a.get(field.clone())?.expect("field exists"),
					&b.get(field)?.expect("field exists"),
				)? {
					return Ok(false);
				}
			}
			Ok(true)
		}
		(a, b) => Ok(primitive_equals(a, b)?),
	}
}
