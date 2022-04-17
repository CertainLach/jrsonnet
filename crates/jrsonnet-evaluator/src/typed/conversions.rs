use std::convert::{TryFrom, TryInto};

use jrsonnet_interner::IStr;
pub use jrsonnet_macros::Typed;
use jrsonnet_types::{ComplexValType, ValType};

use crate::{
	error::{Error::*, LocError, Result},
	throw,
	typed::CheckType,
	ArrValue, FuncVal, IndexableVal, ObjValue, ObjValueBuilder, Val,
};

pub trait TypedObj: Typed {
	fn serialize(self, out: &mut ObjValueBuilder) -> Result<()>;
	fn parse(obj: &ObjValue) -> Result<Self>;
	fn into_object(self) -> Result<ObjValue> {
		let mut builder = ObjValueBuilder::new();
		self.serialize(&mut builder)?;
		Ok(builder.build())
	}
}

pub trait Typed: TryFrom<Val, Error = LocError> + TryInto<Val, Error = LocError> {
	const TYPE: &'static ComplexValType;
}

macro_rules! impl_int {
	($($ty:ty)*) => {$(
		impl Typed for $ty {
			const TYPE: &'static ComplexValType =
				&ComplexValType::BoundedNumber(Some(Self::MIN as f64), Some(Self::MAX as f64));
		}
		impl TryFrom<Val> for $ty {
			type Error = LocError;

			fn try_from(value: Val) -> Result<Self> {
				<Self as Typed>::TYPE.check(&value)?;
				match value {
					Val::Num(n) => {
						if n.trunc() != n {
							throw!(RuntimeError(
								format!(
									"cannot convert number with fractional part to {}",
									stringify!($ty)
								)
								.into()
							))
						}
						Ok(n as Self)
					}
					_ => unreachable!(),
				}
			}
		}
		impl TryFrom<$ty> for Val {
			type Error = LocError;

			fn try_from(value: $ty) -> Result<Self> {
				Ok(Self::Num(value as f64))
			}
		}
	)*};
}

impl_int!(i8 u8 i16 u16 i32 u32);

impl Typed for f64 {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Num);
}
impl TryFrom<Val> for f64 {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Num(n) => Ok(n),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<f64> for Val {
	type Error = LocError;

	fn try_from(value: f64) -> Result<Self> {
		Ok(Self::Num(value))
	}
}

pub struct PositiveF64(pub f64);
impl Typed for PositiveF64 {
	const TYPE: &'static ComplexValType = &ComplexValType::BoundedNumber(Some(0.0), None);
}
impl TryFrom<Val> for PositiveF64 {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Num(n) => Ok(Self(n)),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<PositiveF64> for Val {
	type Error = LocError;

	fn try_from(value: PositiveF64) -> Result<Self> {
		Ok(Self::Num(value.0))
	}
}

impl Typed for usize {
	// It is possible to store 54 bits of precision in f64, but leaving u32::MAX here for compatibility
	const TYPE: &'static ComplexValType =
		&ComplexValType::BoundedNumber(Some(0.0), Some(4294967295.0));
}
impl TryFrom<Val> for usize {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Num(n) => {
				if n.trunc() != n {
					throw!(RuntimeError(
						"cannot convert number with fractional part to usize".into()
					))
				}
				Ok(n as Self)
			}
			_ => unreachable!(),
		}
	}
}
impl TryFrom<usize> for Val {
	type Error = LocError;

	fn try_from(value: usize) -> Result<Self> {
		if value > u32::MAX as usize {
			throw!(RuntimeError("number is too large".into()))
		}
		Ok(Self::Num(value as f64))
	}
}

impl Typed for IStr {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Str);
}
impl TryFrom<Val> for IStr {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<IStr> for Val {
	type Error = LocError;

	fn try_from(value: IStr) -> Result<Self> {
		Ok(Self::Str(value))
	}
}

impl Typed for String {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Str);
}
impl TryFrom<Val> for String {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s.to_string()),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<String> for Val {
	type Error = LocError;

	fn try_from(value: String) -> Result<Self> {
		Ok(Self::Str(value.into()))
	}
}

impl Typed for char {
	const TYPE: &'static ComplexValType = &ComplexValType::Char;
}
impl TryFrom<Val> for char {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Str(s) => Ok(s.chars().next().unwrap()),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<char> for Val {
	type Error = LocError;

	fn try_from(value: char) -> Result<Self> {
		Ok(Self::Str(value.to_string().into()))
	}
}

impl<T> Typed for Vec<T>
where
	T: Typed,
	T: TryFrom<Val, Error = LocError>,
	T: TryInto<Val, Error = LocError>,
{
	const TYPE: &'static ComplexValType = &ComplexValType::ArrayRef(T::TYPE);
}
impl<T> TryFrom<Val> for Vec<T>
where
	T: Typed,
	T: TryFrom<Val, Error = LocError>,
	T: TryInto<Val, Error = LocError>,
{
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Arr(a) => {
				let mut o = Self::with_capacity(a.len());
				for i in a.iter() {
					o.push(T::try_from(i?)?);
				}
				Ok(o)
			}
			_ => unreachable!(),
		}
	}
}
impl<T> TryFrom<Vec<T>> for Val
where
	T: Typed,
	T: TryFrom<Self, Error = LocError>,
	T: TryInto<Self, Error = LocError>,
{
	type Error = LocError;

	fn try_from(value: Vec<T>) -> Result<Self> {
		let mut o = Vec::with_capacity(value.len());
		for i in value {
			o.push(i.try_into()?);
		}
		Ok(Self::Arr(o.into()))
	}
}

/// To be used in Vec<Any>
/// Regular Val can't be used here, because it has wrong TryFrom::Error type
#[derive(Clone)]
pub struct Any(pub Val);

impl Typed for Any {
	const TYPE: &'static ComplexValType = &ComplexValType::Any;
}
impl TryFrom<Val> for Any {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		Ok(Self(value))
	}
}
impl TryFrom<Any> for Val {
	type Error = LocError;

	fn try_from(value: Any) -> Result<Self> {
		Ok(value.0)
	}
}

/// Specialization, provides faster TryFrom<VecVal> for Val
pub struct VecVal(pub Vec<Val>);

impl Typed for VecVal {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Arr);
}
impl TryFrom<Val> for VecVal {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Arr(a) => Ok(Self(a.evaluated()?.to_vec())),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<VecVal> for Val {
	type Error = LocError;

	fn try_from(value: VecVal) -> Result<Self> {
		Ok(Self::Arr(value.0.into()))
	}
}

pub struct M1;
impl Typed for M1 {
	const TYPE: &'static ComplexValType = &ComplexValType::BoundedNumber(Some(-1.0), Some(-1.0));
}
impl TryFrom<Val> for M1 {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		Ok(Self)
	}
}
impl TryFrom<M1> for Val {
	type Error = LocError;

	fn try_from(_: M1) -> Result<Self> {
		Ok(Self::Num(-1.0))
	}
}

macro_rules! decl_either {
	($($name: ident, $($id: ident)*);*) => {$(
		pub enum $name<$($id),*> {
			$($id($id)),*
		}
		impl<$($id),*> Typed for $name<$($id),*>
		where
			$($id: Typed,)*
		{
			const TYPE: &'static ComplexValType = &ComplexValType::UnionRef(&[$($id::TYPE),*]);
		}
		impl<$($id),*> TryFrom<Val> for $name<$($id),*>
		where
			$($id: Typed,)*
		{
			type Error = LocError;

			fn try_from(value: Val) -> Result<Self> {
				$(
					if $id::TYPE.check(&value).is_ok() {
						$id::try_from(value).map(Self::$id)
					} else
				)* {
					<Self as Typed>::TYPE.check(&value)?;
					unreachable!()
				}
			}
		}
		impl<$($id),*> TryFrom<$name<$($id),*>> for Val
		where
			$($id: Typed,)*
		{
			type Error = LocError;
			fn try_from(value: $name<$($id),*>) -> Result<Self> {
				match value {$(
					$name::$id(v) => v.try_into()
				),*}
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
	($a:ty) => {Either1<$a>};
	($a:ty, $b:ty) => {Either2<$a, $b>};
	($a:ty, $b:ty, $c:ty) => {Either3<$a, $b, $c>};
	($a:ty, $b:ty, $c:ty, $d:ty) => {Either4<$a, $b, $c, $d>};
	($a:ty, $b:ty, $c:ty, $d:ty, $e:ty) => {Either5<$a, $b, $c, $d, $e>};
	($a:ty, $b:ty, $c:ty, $d:ty, $e:ty, $f:ty) => {Either6<$a, $b, $c, $d, $e, $f>};
	($a:ty, $b:ty, $c:ty, $d:ty, $e:ty, $f:ty, $g:ty) => {Either7<$a, $b, $c, $d, $e, $f, $g>};
}

pub type MyType = Either![u32, f64, String];

impl Typed for ArrValue {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Arr);
}
impl TryFrom<Val> for ArrValue {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Arr(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<ArrValue> for Val {
	type Error = LocError;

	fn try_from(value: ArrValue) -> Result<Self> {
		Ok(Self::Arr(value))
	}
}

impl Typed for FuncVal {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Func);
}
impl TryFrom<Val> for FuncVal {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Func(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<FuncVal> for Val {
	type Error = LocError;

	fn try_from(value: FuncVal) -> Result<Self> {
		Ok(Self::Func(value))
	}
}
impl Typed for ObjValue {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Obj);
}
impl TryFrom<Val> for ObjValue {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Obj(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<ObjValue> for Val {
	type Error = LocError;

	fn try_from(value: ObjValue) -> Result<Self> {
		Ok(Self::Obj(value))
	}
}

impl Typed for bool {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Bool);
}
impl TryFrom<Val> for bool {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		match value {
			Val::Bool(a) => Ok(a),
			_ => unreachable!(),
		}
	}
}
impl TryFrom<bool> for Val {
	type Error = LocError;

	fn try_from(value: bool) -> Result<Self> {
		Ok(Self::Bool(value))
	}
}

impl Typed for IndexableVal {
	const TYPE: &'static ComplexValType = &ComplexValType::UnionRef(&[
		&ComplexValType::Simple(ValType::Arr),
		&ComplexValType::Simple(ValType::Str),
	]);
}
impl TryFrom<Val> for IndexableVal {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		value.into_indexable()
	}
}
impl TryFrom<IndexableVal> for Val {
	type Error = LocError;

	fn try_from(value: IndexableVal) -> Result<Self> {
		match value {
			IndexableVal::Str(s) => Ok(Self::Str(s)),
			IndexableVal::Arr(a) => Ok(Self::Arr(a)),
		}
	}
}

pub struct Null;
impl Typed for Null {
	const TYPE: &'static ComplexValType = &ComplexValType::Simple(ValType::Null);
}
impl TryFrom<Val> for Null {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self> {
		<Self as Typed>::TYPE.check(&value)?;
		Ok(Self)
	}
}
impl TryFrom<Null> for Val {
	type Error = LocError;

	fn try_from(_: Null) -> Result<Self> {
		Ok(Self::Null)
	}
}
