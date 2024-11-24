use super::{
	arglike::{ArgLike, OptionalContext},
	FuncVal,
};
use crate::{function::builtin::Builtin, typed::Typed, Result, State};

pub trait NativeDesc {
	type Value;
	fn into_native(val: FuncVal) -> Self::Value;
}
macro_rules! impl_native_desc {
	($($gen:ident)*) => {
		impl<$($gen,)* O> NativeDesc for (($($gen,)*), O)
		where
			$($gen: ArgLike + OptionalContext,)*
			O: Typed,
		{
			type Value = Box<dyn Fn(State, $($gen,)*) -> Result<O>>;

			#[allow(non_snake_case)]
			fn into_native(val: FuncVal) -> Self::Value {
				Box::new(move |state, $($gen),*| {
					let val = val.call(
						state,
						&($($gen,)*),
						false,
					)?;
					O::from_untyped(val)
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
