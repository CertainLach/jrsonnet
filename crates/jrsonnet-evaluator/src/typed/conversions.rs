use std::{collections::BTreeMap, marker::PhantomData, ops::Deref};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::{IBytes, IStr};
pub use jrsonnet_macros::Typed;
use jrsonnet_types::{ComplexValType, ValType};

use crate::{
	arr::{ArrValue, BytesArray},
	bail,
	function::{native::NativeDesc, FuncDesc, FuncVal},
	typed::CheckType,
	val::{IndexableVal, NumValue, StrValue, ThunkMapper},
	ObjValue, ObjValueBuilder, Result, ResultExt, Thunk, Val,
};

#[derive(Trace)]
struct FromUntyped<K: Trace>(PhantomData<fn() -> K>);
impl<K> ThunkMapper<Val> for FromUntyped<K>
where
	K: Typed + Trace,
{
	type Output = K;

	fn map(self, from: Val) -> Result<Self::Output> {
		K::from_untyped(from)
	}
}
impl<K: Trace> Default for FromUntyped<K> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

pub trait TypedObj: Typed {
	fn serialize(self, out: &mut ObjValueBuilder) -> Result<()>;
	fn parse(obj: &ObjValue) -> Result<Self>;
	fn into_object(self) -> Result<ObjValue> {
		let mut builder = ObjValueBuilder::new();
		self.serialize(&mut builder)?;
		Ok(builder.build())
	}
}

pub trait Typed: Sized {
	const TYPE: &'static ComplexValType;
	fn into_untyped(typed: Self) -> Result<Val>;
	fn into_lazy_untyped(typed: Self) -> Thunk<Val> {
		Thunk::from(Self::into_untyped(typed))
	}
	fn from_untyped(untyped: Val) -> Result<Self>;
	fn from_lazy_untyped(lazy: Thunk<Val>) -> Result<Self> {
		Self::from_untyped(lazy.evaluate()?)
	}

	// Whatever caller should use `into_lazy_untyped` instead of `into_untyped`
	fn provides_lazy() -> bool {
		false
	}

	// Whatever caller should use `from_lazy_untyped` instead of `from_untyped` when possible
	fn wants_lazy() -> bool {
		false
	}

	/// Hack to make builtins be able to return non-result values, and make macros able to convert those values to result
	/// This method returns identity in impl Typed for Result, and should not be overriden
	#[doc(hidden)]
	fn into_result(typed: Self) -> Result<Val> {
		let value = Self::into_untyped(typed)?;
		Ok(value)
	}
}

impl<T> Typed for Thunk<T>
where
	T: Typed + Trace + Clone,
{
	const TYPE: &'static ComplexValType = &ComplexValType::Lazy(T::TYPE);

	fn into_untyped(typed: Self) -> Result<Val> {
		T::into_untyped(typed.evaluate()?)
	}

	fn from_untyped(untyped: Val) -> Result<Self> {
		Self::from_lazy_untyped(Thunk::evaluated(untyped))
	}

	fn provides_lazy() -> bool {
		true
	}

	fn into_lazy_untyped(inner: Self) -> Thunk<Val> {
		#[derive(Trace)]
		struct IntoUntyped<K: Trace>(PhantomData<fn() -> K>);
		impl<K> ThunkMapper<K> for IntoUntyped<K>
		where
			K: Typed + Trace,
		{
			type Output = Val;

			fn map(self, from: K) -> Result<Self::Output> {
				K::into_untyped(from)
			}
		}
		impl<K: Trace> Default for IntoUntyped<K> {
			fn default() -> Self {
				Self(PhantomData)
			}
		}
		inner.map(<IntoUntyped<T>>::default())
	}

	fn wants_lazy() -> bool {
		true
	}

	fn from_lazy_untyped(inner: Thunk<Val>) -> Result<Self> {
		Ok(inner.map(<FromUntyped<T>>::default()))
	}
}

pub const MAX_SAFE_INTEGER: f64 = ((1u64 << (f64::MANTISSA_DIGITS + 1)) - 1) as f64;
pub const MIN_SAFE_INTEGER: f64 = -MAX_SAFE_INTEGER;

macro_rules! impl_int {
	($($ty:ty)*) => {$(
		impl Typed for $ty {
			const TYPE: &'static ComplexValType =
				&ComplexValType::BoundedNumber(Some(Self::MIN as f64), Some(Self::MAX as f64));
			fn from_untyped(value: Val) -> Result<Self> {
				<Self as Typed>::TYPE.check(&value)?;
				match value {
					Val::Num(n) => {
						let n = n.get();
						#[allow(clippy::float_cmp)]
						if n.trunc() != n {
							bail!(
								"cannot convert number with fractional part to {}",
								stringify!($ty)
							)
						}
						Ok(n as Self)
					}
					_ => unreachable!(),
				}
			}
			fn into_untyped(value: Self) -> Result<Val> {
				Ok(Val::Num(value.into()))
			}
		}
	)*};
}

impl_int!(i8 u8 i16 u16 i32 u32);

macro_rules! impl_bounded_int {
	($($name:ident = $ty:ty)*) => {$(
		#[derive(Clone, Copy)]
		pub struct $name<const MIN: $ty, const MAX: $ty>($ty);
		impl<const MIN: $ty, const MAX: $ty> $name<MIN, MAX> {
			pub const fn new(value: $ty) -> Option<$name<MIN, MAX>> {
				if value >= MIN && value <= MAX {
					Some(Self(value))
				} else {
					None
				}
			}
			pub const fn value(self) -> $ty {
				self.0
			}
		}
		impl<const MIN: $ty, const MAX: $ty> Deref for $name<MIN, MAX> {
			type Target = $ty;
			fn deref(&self) -> &Self::Target {
				&self.0
			}
		}

		impl<const MIN: $ty, const MAX: $ty> Typed for $name<MIN, MAX> {
			const TYPE: &'static ComplexValType =
				&ComplexValType::BoundedNumber(
					Some(MIN as f64),
					Some(MAX as f64),
				);

			fn from_untyped(value: Val) -> Result<Self> {
				<Self as Typed>::TYPE.check(&value)?;
				match value {
					Val::Num(n) => {
						let n = n.get();
						#[allow(clippy::float_cmp)]
						if n.trunc() != n {
							bail!(
								"cannot convert number with fractional part to {}",
								stringify!($ty)
							)
						}
						Ok(Self(n as $ty))
					}
					_ => unreachable!(),
				}
			}

			#[allow(clippy::cast_lossless)]
			fn into_untyped(value: Self) -> Result<Val> {
				Ok(Val::try_num(value.0)?)
			}
		}
	)*};
}

impl_bounded_int!(
	BoundedI8 = i8
	BoundedI16 = i16
	BoundedI32 = i32
	BoundedI64 = i64
	BoundedUsize = usize
);

impl Typed for f64 {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Num);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::try_num(value)?)
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Num(n) => Ok(n.get()),
			_ => unreachable!(),
		}
	}
}

pub struct PositiveF64(pub f64);
impl Typed for PositiveF64 {
	const TYPE: &'static ComplexValType = &ComplexValType::BoundedNumber(Some(0.0), None);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::try_num(value.0)?)
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Num(n) => Ok(Self(n.get())),
			_ => unreachable!(),
		}
	}
}
impl Typed for usize {
	const TYPE: &'static ComplexValType =
		&ComplexValType::BoundedNumber(Some(0.0), Some(MAX_SAFE_INTEGER));

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::try_num(value)?)
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Num(n) => {
				let n = n.get();
				#[allow(clippy::float_cmp)]
				if n.trunc() != n {
					bail!("cannot convert number with fractional part to usize")
				}
				Ok(n as Self)
			}
			_ => unreachable!(),
		}
	}
}

impl Typed for IStr {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Str);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::string(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s.into_flat()),
			_ => unreachable!(),
		}
	}
}

impl Typed for String {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Str);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::string(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s.to_string()),
			_ => unreachable!(),
		}
	}
}

impl Typed for StrValue {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Str);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Str(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s),
			_ => unreachable!(),
		}
	}
}

impl Typed for char {
	const TYPE: &'static ComplexValType = &ComplexValType::Char;

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::string(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s.into_flat().chars().next().unwrap()),
			_ => unreachable!(),
		}
	}
}

impl<T> Typed for Vec<T>
where
	T: Typed,
{
	const TYPE: &'static ComplexValType = &ComplexValType::ArrayRef(T::TYPE);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Arr(
			value
				.into_iter()
				.map(T::into_untyped)
				.collect::<Result<ArrValue>>()?,
		))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		let Val::Arr(a) = value else {
			<Self as Typed>::TYPE.check(&value)?;
			unreachable!("typecheck should fail")
		};
		a.iter()
			.enumerate()
			.map(|(i, r)| {
				r.and_then(|t| {
					T::from_untyped(t).with_description(|| format!("parsing elem <{i}>"))
				})
			})
			.collect::<Result<Self>>()
	}
}

impl<K: Typed + Ord, V: Typed> Typed for BTreeMap<K, V> {
	const TYPE: &'static ComplexValType = &ComplexValType::AttrsOf(V::TYPE);

	fn into_untyped(typed: Self) -> Result<Val> {
		let mut out = ObjValueBuilder::with_capacity(typed.len());
		for (k, v) in typed {
			let Some(key) = K::into_untyped(k)?.as_str() else {
				bail!("map key should serialize to string");
			};
			let value = V::into_untyped(v)?;
			out.field(key).value(value);
		}
		Ok(Val::Obj(out.build()))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		Self::TYPE.check(&value)?;
		let obj = value.as_obj().expect("typecheck should fail");

		let mut out = Self::new();
		if V::wants_lazy() {
			for key in obj.fields_ex(
				false,
				#[cfg(feature = "exp-preserve-order")]
				false,
			) {
				let value = obj.get_lazy(key.clone()).expect("field exists");
				let value = V::from_lazy_untyped(value)?;
				let key = K::from_untyped(Val::Str(key.into()))?;
				let _ = out.insert(key, value);
			}
		} else {
			for (key, value) in obj.iter(
				#[cfg(feature = "exp-preserve-order")]
				false,
			) {
				let key = K::from_untyped(Val::Str(key.into()))?;
				let value = V::from_untyped(value?)?;
				let _ = out.insert(key, value);
			}
		}
		Ok(out)
	}
}

impl Typed for Val {
	const TYPE: &'static ComplexValType = &ComplexValType::Any;

	fn into_untyped(typed: Self) -> Result<Val> {
		Ok(typed)
	}
	fn from_untyped(untyped: Val) -> Result<Self> {
		Ok(untyped)
	}
}

// Hack
#[doc(hidden)]
impl<T> Typed for Result<T>
where
	T: Typed,
{
	const TYPE: &'static ComplexValType = &ComplexValType::Any;

	fn into_untyped(_typed: Self) -> Result<Val> {
		panic!("do not use this conversion")
	}

	fn from_untyped(_untyped: Val) -> Result<Self> {
		panic!("do not use this conversion")
	}

	fn into_result(typed: Self) -> Result<Val> {
		typed.map(T::into_untyped)?
	}
}

/// Specialization
impl Typed for IBytes {
	const TYPE: &'static ComplexValType =
		&ComplexValType::ArrayRef(&ComplexValType::BoundedNumber(Some(0.0), Some(255.0)));

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Arr(ArrValue::bytes(value)))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		let Val::Arr(a) = &value else {
			<Self as Typed>::TYPE.check(&value)?;
			unreachable!()
		};
		if let Some(bytes) = a.as_any().downcast_ref::<BytesArray>() {
			return Ok(bytes.0.as_slice().into());
		};
		<Self as Typed>::TYPE.check(&value)?;
		// Any::downcast_ref::<ByteArray>(&a);
		let mut out = Vec::with_capacity(a.len());
		for e in a.iter() {
			let r = e?;
			out.push(u8::from_untyped(r)?);
		}
		Ok(out.as_slice().into())
	}
}

pub struct M1;
impl Typed for M1 {
	const TYPE: &'static ComplexValType = &ComplexValType::BoundedNumber(Some(-1.0), Some(-1.0));

	fn into_untyped(_: Self) -> Result<Val> {
		Ok(Val::Num(NumValue::new(-1.0).expect("finite")))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		Ok(Self)
	}
}

macro_rules! decl_either {
	($($name: ident, $($id: ident)*);*) => {$(
		#[derive(Clone)]
		pub enum $name<$($id),*> {
			$($id($id)),*
		}
		impl<$($id),*> Typed for $name<$($id),*>
		where
			$($id: Typed,)*
		{
			const TYPE: &'static ComplexValType = &ComplexValType::UnionRef(&[$($id::TYPE),*]);

			fn into_untyped(value: Self) -> Result<Val> {
				match value {$(
					$name::$id(v) => $id::into_untyped(v)
				),*}
			}

			fn from_untyped(value: Val) -> Result<Self> {
				$(
					if $id::TYPE.check(&value).is_ok() {
						$id::from_untyped(value).map(Self::$id)
					} else
				)* {
					<Self as Typed>::TYPE.check(&value)?;
					unreachable!()
				}
			}
		}
	)*}
}
decl_either!(
	Either1, A;
	Either2, A B;
	Either3, A B C;
	Either4, A B C D;
	Either5, A B C D E;
	Either6, A B C D E F;
	Either7, A B C D E F G
);
#[macro_export]
macro_rules! Either {
	($a:ty) => {$crate::typed::Either1<$a>};
	($a:ty, $b:ty) => {$crate::typed::Either2<$a, $b>};
	($a:ty, $b:ty, $c:ty) => {$crate::typed::Either3<$a, $b, $c>};
	($a:ty, $b:ty, $c:ty, $d:ty) => {$crate::typed::Either4<$a, $b, $c, $d>};
	($a:ty, $b:ty, $c:ty, $d:ty, $e:ty) => {$crate::typed::Either5<$a, $b, $c, $d, $e>};
	($a:ty, $b:ty, $c:ty, $d:ty, $e:ty, $f:ty) => {$crate::typed::Either6<$a, $b, $c, $d, $e, $f>};
	($a:ty, $b:ty, $c:ty, $d:ty, $e:ty, $f:ty, $g:ty) => {$crate::typed::Either7<$a, $b, $c, $d, $e, $f, $g>};
}
pub use Either;

pub type MyType = Either![u32, f64, String];

impl Typed for ArrValue {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Arr);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Arr(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Arr(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}

impl Typed for FuncVal {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Func);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Func(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Func(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}

impl Typed for Cc<FuncDesc> {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Func);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Func(FuncVal::Normal(value)))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Func(FuncVal::Normal(desc)) => Ok(desc),
			Val::Func(_) => bail!("expected normal function, not builtin"),
			_ => unreachable!(),
		}
	}
}

impl Typed for ObjValue {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Obj);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Obj(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Obj(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}

impl Typed for bool {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Bool);

	fn into_untyped(value: Self) -> Result<Val> {
		Ok(Val::Bool(value))
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Bool(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}
impl Typed for IndexableVal {
	const TYPE: &'static ComplexValType = &ComplexValType::UnionRef(&[
		&ComplexValType::Simple(ValType::Arr),
		&ComplexValType::Simple(ValType::Str),
	]);

	fn into_untyped(value: Self) -> Result<Val> {
		match value {
			Self::Str(s) => Ok(Val::string(s)),
			Self::Arr(a) => Ok(Val::Arr(a)),
		}
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		value.into_indexable()
	}
}

pub struct Null;
impl Typed for Null {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Null);

	fn into_untyped(_: Self) -> Result<Val> {
		Ok(Val::Null)
	}

	fn from_untyped(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		Ok(Self)
	}
}

pub struct NativeFn<D: NativeDesc>(D::Value);
impl<D: NativeDesc> Deref for NativeFn<D> {
	type Target = D::Value;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl<D: NativeDesc> Typed for NativeFn<D> {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Func);

	fn into_untyped(_typed: Self) -> Result<Val> {
		bail!("can only convert functions from jsonnet to native")
	}

	fn from_untyped(untyped: Val) -> Result<Self> {
		Ok(Self(
			untyped
				.as_func()
				.expect("shape is checked")
				.into_native::<D>(),
		))
	}
}

impl Typed for NumValue {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Num);

	fn into_untyped(typed: Self) -> Result<Val> {
		Ok(Val::Num(typed))
	}

	fn from_untyped(untyped: Val) -> Result<Self> {
		Self::TYPE.check(&untyped)?;
		match untyped {
			Val::Num(v) => Ok(v),
			_ => unreachable!(),
		}
	}
}
