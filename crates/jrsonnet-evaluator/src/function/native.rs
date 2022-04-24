use super::{arglike::ArgLike, CallLocation, FuncVal};
use crate::{error::Result, typed::Typed, State};

pub trait NativeDesc {
	type Value;
	fn into_native(val: FuncVal) -> Self::Value;
}
macro_rules! impl_native_desc {
	($($gen:ident)*) => {
		impl<$($gen,)* O> NativeDesc for (($($gen,)*), O)
		where
			$($gen: ArgLike,)*
			O: Typed,
		{
			type Value = Box<dyn Fn(State, $($gen,)*) -> Result<O>>;

			#[allow(non_snake_case)]
			fn into_native(val: FuncVal) -> Self::Value {
				Box::new(move |s: State, $($gen),*| {
					let val = val.evaluate(
						s.clone(),
						s.create_default_context(),
						CallLocation::native(),
						&($($gen,)*),
						true
					)?;
					O::from_untyped(val, s.clone())
				})
			}
		}
	};
	($($cur:ident)* @ $c:ident $($rest:ident)*) => {
		impl_native_desc!($($cur)*);
		impl_native_desc!($($cur)* $c @ $($rest)*);
	};
	($($cur:ident)* @) => {
		impl_native_desc!($($cur)*);
	}
}

impl_native_desc! {
	@ A B C D E F G H I J K L
}
