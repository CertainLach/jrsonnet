use jrsonnet_parser::LocExpr;

mod structdump_import {
	pub(super) use std::{borrow::Cow, option::Option, rc::Rc, vec};

	pub(super) use jrsonnet_parser::*;
}

pub fn stdlib_expr() -> LocExpr {
	#[cfg(feature = "serialized-stdlib")]
	{
		use bincode::{BincodeRead, DefaultOptions, Options};
		use serde::{Deserialize, Deserializer};

		struct LocDeserializer<R, O: Options> {
			source: Source,
			wrapped: bincode::Deserializer<R, O>,
		}
		macro_rules! delegate {
			($(fn $name:ident($($arg:ident: $ty:ty),*))+) => {$(
				fn $name<V>(mut self $(, $arg: $ty)*, visitor: V) -> Result<V::Value, Self::Error>
				where V: serde::de::Visitor<'de>,
				{
					self.wrapped.$name($($arg,)* visitor)
				}
			)+};
		}
		impl<'de, R, O> Deserializer<'de> for LocDeserializer<R, O>
		where
			R: BincodeRead<'de>,
			O: Options,
		{
			type Error = <&'de mut bincode::Deserializer<R, O> as Deserializer<'de>>::Error;

			delegate! {
				fn deserialize_any()
				fn deserialize_bool()
				fn deserialize_u16()
				fn deserialize_u32()
				fn deserialize_u64()
				fn deserialize_i16()
				fn deserialize_i32()
				fn deserialize_i64()
				fn deserialize_f32()
				fn deserialize_f64()
				fn deserialize_u128()
				fn deserialize_i128()
				fn deserialize_u8()
				fn deserialize_i8()
				fn deserialize_unit()
				fn deserialize_char()
				fn deserialize_str()
				fn deserialize_string()
				fn deserialize_bytes()
				fn deserialize_byte_buf()
				fn deserialize_enum(name: &'static str, variants: &'static [&'static str])
				fn deserialize_tuple(len: usize)
				fn deserialize_option()
				fn deserialize_seq()
				fn deserialize_map()
				fn deserialize_struct(name: &'static str, fields: &'static [&'static str])
				fn deserialize_identifier()
				fn deserialize_newtype_struct(name: &'static str)
				fn deserialize_unit_struct(name: &'static str)
				fn deserialize_tuple_struct(name: &'static str, len: usize)
				fn deserialize_ignored_any()
			}

			fn is_human_readable(&self) -> bool {
				false
			}
		}

		// In build.rs, Source object is populated with empty values, deserializer wrapper loads correct values on deserialize
		let mut deserializer = bincode::Deserializer::from_slice(
			include_bytes!(concat!(env!("OUT_DIR"), "/stdlib.bincode")),
			DefaultOptions::new()
				.with_fixint_encoding()
				.allow_trailing_bytes(),
		);

		// Should not panic, stdlib.bincode is generated in build.rs
		LocExpr::deserialize(&mut deserializer).unwrap()
	}

	#[cfg(feature = "codegenerated-stdlib")]
	{
		include!(concat!(env!("OUT_DIR"), "/stdlib.rs"))
	}

	#[cfg(not(feature = "codegenerated-stdlib"))]
	{
		jrsonnet_parser::parse(
			STDLIB_STR,
			&ParserSettings {
				file_name: Source::new_virtual(Cow::Borrowed("<std>"), STDLIB_STR.into()),
			},
		)
		.unwrap()
	}
}
