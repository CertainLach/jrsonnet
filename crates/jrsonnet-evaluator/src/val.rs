use std::{
	cell::RefCell,
	fmt::{self, Debug, Display},
	mem::replace,
	rc::Rc,
};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_types::ValType;

pub use crate::arr::ArrValue;
use crate::{
	error::{Error, ErrorKind::*},
	function::FuncVal,
	gc::{GcHashMap, TraceBox},
	manifest::{ManifestFormat, ToStringFormat},
	tb, throw,
	typed::BoundedUsize,
	ObjValue, Result, Unbound, WeakObjValue,
};

pub trait ThunkValue: Trace {
	type Output;
	fn get(self: Box<Self>) -> Result<Self::Output>;
}

#[derive(Trace)]
enum ThunkInner<T: Trace> {
	Computed(T),
	Errored(Error),
	Waiting(TraceBox<dyn ThunkValue<Output = T>>),
	Pending,
}

/// Lazily evaluated value
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Trace)]
pub struct Thunk<T: Trace>(Cc<RefCell<ThunkInner<T>>>);

impl<T: Trace> Thunk<T> {
	pub fn evaluated(val: T) -> Self {
		Self(Cc::new(RefCell::new(ThunkInner::Computed(val))))
	}
	pub fn new(f: impl ThunkValue<Output = T> + 'static) -> Self {
		Self(Cc::new(RefCell::new(ThunkInner::Waiting(tb!(f)))))
	}
	pub fn errored(e: Error) -> Self {
		Self(Cc::new(RefCell::new(ThunkInner::Errored(e))))
	}
}

impl<T> Thunk<T>
where
	T: Clone + Trace,
{
	pub fn force(&self) -> Result<()> {
		self.evaluate()?;
		Ok(())
	}

	/// Evaluate thunk, or return cached value
	///
	/// # Errors
	///
	/// - Lazy value evaluation returned error
	/// - This method was called during inner value evaluation
	pub fn evaluate(&self) -> Result<T> {
		match &*self.0.borrow() {
			ThunkInner::Computed(v) => return Ok(v.clone()),
			ThunkInner::Errored(e) => return Err(e.clone()),
			ThunkInner::Pending => return Err(InfiniteRecursionDetected.into()),
			ThunkInner::Waiting(..) => (),
		};
		let ThunkInner::Waiting(value) = replace(&mut *self.0.borrow_mut(), ThunkInner::Pending)
		else {
			unreachable!();
		};
		let new_value = match value.0.get() {
			Ok(v) => v,
			Err(e) => {
				*self.0.borrow_mut() = ThunkInner::Errored(e.clone());
				return Err(e);
			}
		};
		*self.0.borrow_mut() = ThunkInner::Computed(new_value.clone());
		Ok(new_value)
	}
}

pub trait ThunkMapper<Input>: Trace {
	type Output;
	fn map(self, from: Input) -> Result<Self::Output>;
}
impl<Input> Thunk<Input>
where
	Input: Trace + Clone,
{
	pub fn map<M>(self, mapper: M) -> Thunk<M::Output>
	where
		M: ThunkMapper<Input>,
		M::Output: Trace,
	{
		#[derive(Trace)]
		struct Mapped<Input: Trace, Mapper: Trace> {
			inner: Thunk<Input>,
			mapper: Mapper,
		}
		impl<Input, Mapper> ThunkValue for Mapped<Input, Mapper>
		where
			Input: Trace + Clone,
			Mapper: ThunkMapper<Input>,
		{
			type Output = Mapper::Output;

			fn get(self: Box<Self>) -> Result<Self::Output> {
				let value = self.inner.evaluate()?;
				let mapped = self.mapper.map(value)?;
				Ok(mapped)
			}
		}

		Thunk::new(Mapped::<Input, M> {
			inner: self,
			mapper,
		})
	}
}

impl<T: Trace> From<Result<T>> for Thunk<T> {
	fn from(value: Result<T>) -> Self {
		match value {
			Ok(o) => Self::evaluated(o),
			Err(e) => Self::errored(e),
		}
	}
}

type CacheKey = (Option<WeakObjValue>, Option<WeakObjValue>);

#[derive(Trace, Clone)]
pub struct CachedUnbound<I, T>
where
	I: Unbound<Bound = T>,
	T: Trace,
{
	cache: Cc<RefCell<GcHashMap<CacheKey, T>>>,
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
	fn bind(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<T> {
		let cache_key = (
			sup.as_ref().map(|s| s.clone().downgrade()),
			this.as_ref().map(|t| t.clone().downgrade()),
		);
		{
			if let Some(t) = self.cache.borrow().get(&cache_key) {
				return Ok(t.clone());
			}
		}
		let bound = self.value.bind(sup, this)?;

		{
			let mut cache = self.cache.borrow_mut();
			cache.insert(cache_key, bound.clone());
		}

		Ok(bound)
	}
}

impl<T: Debug + Trace> Debug for Thunk<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Lazy")
	}
}
impl<T: Trace> PartialEq for Thunk<T> {
	fn eq(&self, other: &Self) -> bool {
		Cc::ptr_eq(&self.0, &other.0)
	}
}

/// Represents a Jsonnet value, which can be sliced or indexed (string or array).
#[allow(clippy::module_name_repetitions)]
pub enum IndexableVal {
	/// String.
	Str(IStr),
	/// Array.
	Arr(ArrValue),
}
impl IndexableVal {
	pub fn to_array(self) -> ArrValue {
		match self {
			IndexableVal::Str(s) => ArrValue::chars(s.chars()),
			IndexableVal::Arr(arr) => arr,
		}
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
		index: Option<BoundedUsize<0, { i32::MAX as usize }>>,
		end: Option<BoundedUsize<0, { i32::MAX as usize }>>,
		step: Option<BoundedUsize<1, { i32::MAX as usize }>>,
	) -> Result<Self> {
		match &self {
			IndexableVal::Str(s) => {
				let index = index.as_deref().copied().unwrap_or(0);
				let end = end.as_deref().copied().unwrap_or(usize::MAX);
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
			IndexableVal::Arr(arr) => {
				let index = index.as_deref().copied().unwrap_or(0);
				let end = end.as_deref().copied().unwrap_or(usize::MAX).min(arr.len());
				let step = step.as_deref().copied().unwrap_or(1);

				if index >= end {
					return Ok(Self::Arr(ArrValue::empty()));
				}

				Ok(Self::Arr(
					arr.clone()
						.slice(Some(index), Some(end), Some(step))
						.expect("arguments checked"),
				))
			}
		}
	}
}

#[derive(Debug, Clone, Trace)]
pub enum StrValue {
	Flat(IStr),
	Tree(Rc<(StrValue, StrValue, usize)>),
}
impl StrValue {
	pub fn concat(a: StrValue, b: StrValue) -> Self {
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
			StrValue::Flat(f) => f,
			StrValue::Tree(_) => {
				let mut buf = String::with_capacity(self.len());
				write_buf(&self, &mut buf);
				buf.into()
			}
		}
	}
	pub fn len(&self) -> usize {
		match self {
			StrValue::Flat(v) => v.len(),
			StrValue::Tree(t) => t.2,
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
impl From<&str> for StrValue {
	fn from(value: &str) -> Self {
		Self::Flat(value.into())
	}
}
impl From<String> for StrValue {
	fn from(value: String) -> Self {
		Self::Flat(value.into())
	}
}
impl From<IStr> for StrValue {
	fn from(value: IStr) -> Self {
		Self::Flat(value)
	}
}
impl Display for StrValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			StrValue::Flat(v) => write!(f, "{v}"),
			StrValue::Tree(t) => {
				write!(f, "{}", t.0)?;
				write!(f, "{}", t.1)
			}
		}
	}
}
impl PartialEq for StrValue {
	fn eq(&self, other: &Self) -> bool {
		let a = self.clone().into_flat();
		let b = other.clone().into_flat();
		a == b
	}
}
impl Eq for StrValue {}
impl PartialOrd for StrValue {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}
impl Ord for StrValue {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		let a = self.clone().into_flat();
		let b = other.clone().into_flat();
		a.cmp(&b)
	}
}

/// Represents any valid Jsonnet value.
#[derive(Debug, Clone, Trace)]
pub enum Val {
	/// Represents a Jsonnet boolean.
	Bool(bool),
	/// Represents a Jsonnet null value.
	Null,
	/// Represents a Jsonnet string.
	Str(StrValue),
	/// Represents a Jsonnet number.
	/// Should be finite, and not NaN
	/// This restriction isn't enforced by enum, as enum field can't be marked as private
	Num(f64),
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

impl From<IndexableVal> for Val {
	fn from(v: IndexableVal) -> Self {
		match v {
			IndexableVal::Str(s) => Self::Str(StrValue::Flat(s)),
			IndexableVal::Arr(a) => Self::Arr(a),
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
	pub fn as_str(&self) -> Option<IStr> {
		match self {
			Self::Str(s) => Some(s.clone().into_flat()),
			_ => None,
		}
	}
	pub const fn as_num(&self) -> Option<f64> {
		match self {
			Self::Num(n) => Some(*n),
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

	/// Creates `Val::Num` after checking for numeric overflow.
	/// As numbers are `f64`, we can just check for their finity.
	pub fn new_checked_num(num: f64) -> Result<Self> {
		if num.is_finite() {
			Ok(Self::Num(num))
		} else {
			throw!("overflow")
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

	pub fn into_indexable(self) -> Result<IndexableVal> {
		Ok(match self {
			Val::Str(s) => IndexableVal::Str(s.into_flat()),
			Val::Arr(arr) => IndexableVal::Arr(arr),
			_ => throw!(ValueIsNotIndexable(self.value_type())),
		})
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
		(Val::Num(a), Val::Num(b)) => (a - b).abs() <= f64::EPSILON,
		#[cfg(feature = "exp-bigint")]
		(Val::BigInt(a), Val::BigInt(b)) => a == b,
		(Val::Arr(_), Val::Arr(_)) => {
			throw!("primitiveEquals operates on primitive types, got array")
		}
		(Val::Obj(_), Val::Obj(_)) => {
			throw!("primitiveEquals operates on primitive types, got object")
		}
		(a, b) if is_function_like(a) && is_function_like(b) => {
			throw!("cannot test equality of functions")
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
