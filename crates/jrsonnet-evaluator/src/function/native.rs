use crate::{
	function::PreparedFuncVal,
	typed::{FromUntyped, IntoUntyped},
	BindingValue, CallLocation, Result,
};

pub trait Desc {
	const NUM_ARGS: usize;
	type Value;
	fn into_native(val: Result<PreparedFuncVal>) -> Self::Value;
}
macro_rules! impl_native_desc {
	($i:expr; $($gen:ident)*) => {
		impl<$($gen,)* O> Desc for (($($gen,)*), O)
		where
			$($gen: IntoUntyped,)*
			O: FromUntyped,
		{
			type Value = Box<dyn Fn($($gen,)*) -> Result<O>>;
			const NUM_ARGS: usize = $i;

			#[allow(non_snake_case)]
			fn into_native(val: Result<PreparedFuncVal>) -> Self::Value {
				match val {
					Ok(f) => {
						Box::new(move |$($gen),*| {
							let val = f.call(
								CallLocation::native(),
								&[$(BindingValue::new($gen),)*],
								&[],
							)?;
							O::from_untyped(val)
						})
					}
					Err(e) => {
						#[allow(unused_variables, reason = "they are ignored intentionally, this variant always throws")]
						Box::new(move |$($gen),*| {
							Err(e.clone())
						})
					}
				}
			}
		}
	};
	($i:expr; $($cur:ident)* @ $c:ident $($rest:ident)*) => {
		impl_native_desc!($i; $($cur)*);
		impl_native_desc!($i + 1; $($cur)* $c @ $($rest)*);
	};
	($i:expr; $($cur:ident)* @) => {
		impl_native_desc!($i; $($cur)*);
	}
}

impl_native_desc! {
	0; @ A B C D E F G H I J K L
}
