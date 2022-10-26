use std::borrow::Cow;

use jrsonnet_gcmodule::Cc;
use serde::{
	de::Visitor,
	ser::{Error, SerializeMap, SerializeSeq},
	Deserialize, Serialize,
};

use crate::{error::Result, val::ArrValue, ObjValueBuilder, State, Val};

impl<'de> Deserialize<'de> for Val {
	fn deserialize<D>(deserializer: D) -> Result<Val, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		struct ValVisitor;

		// macro_rules! visit_num {
		// 	($($method:ident => $ty:ty),* $(,)?) => {$(
		// 		fn $method<E>(self, v: $ty) -> Result<Self::Value, E>
		// 		where
		// 			E: serde::de::Error,
		// 		{
		// 			Ok(Val::Num(f64::from(v)))
		// 		}
		// 	)*};
		// }

		impl<'de> Visitor<'de> for ValVisitor {
			type Value = Val;

			fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Bool(v))
			}
			fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				if !v.is_finite() {
					return Err(E::custom("only finite numbers are supported"));
				}
				Ok(Val::Num(v))
			}
			fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Str(v.into()))
			}

			// visit_num! {
			// 	visit_i8 => i8,
			// 	visit_i16 => i16,
			// 	visit_i32 => i32,
			// 	visit_u8 => u8,
			// 	visit_u16 => u16,
			// 	visit_u32 => u32,
			// }
			fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Num(v as f64))
			}
			fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Num(v as f64))
			}

			fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Arr(ArrValue::Bytes(v.into())))
			}

			fn visit_none<E>(self) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Null)
			}
			fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
			where
				D: serde::Deserializer<'de>,
			{
				deserializer.deserialize_any(self)
			}

			fn visit_unit<E>(self) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Val::Null)
			}

			fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
			where
				D: serde::Deserializer<'de>,
			{
				deserializer.deserialize_any(self)
			}

			fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
			where
				A: serde::de::SeqAccess<'de>,
			{
				let mut out = seq.size_hint().map_or_else(Vec::new, Vec::with_capacity);

				while let Some(val) = seq.next_element::<Val>()? {
					out.push(val);
				}

				Ok(Val::Arr(ArrValue::Eager(Cc::new(out))))
			}

			fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
			where
				A: serde::de::MapAccess<'de>,
			{
				let mut out = map
					.size_hint()
					.map_or_else(ObjValueBuilder::new, ObjValueBuilder::with_capacity);

				while let Some((k, v)) = map.next_entry::<Cow<'de, str>, Val>()? {
					// Jsonnet ignores duplicate keys
					out.member(k.into()).value_unchecked(v);
				}

				Ok(Val::Obj(out.build()))
			}

			fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				write!(formatter, "any valid jsonnet value")
			}
		}
		deserializer.deserialize_any(ValVisitor)
	}
}

impl Serialize for Val {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		match self {
			Val::Bool(v) => serializer.serialize_bool(*v),
			Val::Null => serializer.serialize_none(),
			Val::Str(s) => serializer.serialize_str(s),
			Val::Num(n) => serializer.serialize_f64(*n),
			Val::Arr(arr) => {
				let mut seq = serializer.serialize_seq(Some(arr.len()))?;
				for (i, element) in arr.iter().enumerate() {
					let mut serde_error = None;
					// TODO: rewrite using try{} after stabilization
					State::push_description(
						|| format!("array index [{i}]"),
						|| {
							let e = element?;
							if let Err(e) = seq.serialize_element(&e) {
								serde_error = Some(e);
							}
							Ok(())
						},
					)
					.map_err(|e| S::Error::custom(e.to_string()))?;
					if let Some(e) = serde_error {
						return Err(e);
					}
				}
				seq.end()
			}
			Val::Obj(obj) => {
				let mut map = serializer.serialize_map(Some(obj.len()))?;
				for (field, value) in obj.iter(
					#[cfg(feature = "exp-preserve-order")]
					true,
				) {
					let mut serde_error = None;
					// TODO: rewrite using try{} after stabilization
					State::push_description(
						|| format!("object field {field:?}"),
						|| {
							let v = value?;
							if let Err(e) = map.serialize_entry(field.as_str(), &v) {
								serde_error = Some(e);
							}
							Ok(())
						},
					)
					.map_err(|e| S::Error::custom(e.to_string()))?;
					if let Some(e) = serde_error {
						return Err(e);
					}
				}
				map.end()
			}
			Val::Func(_) => Err(S::Error::custom("tried to manifest function")),
		}
	}
}
