use jrsonnet_types::ComplexValType;
use serde_json::{Map, Number, Value};

use crate::{
	error::{Error::*, Result},
	throw,
	typed::Typed,
	ObjValueBuilder, State, Val,
};

impl Typed for Value {
	const TYPE: &'static ComplexValType = &ComplexValType::Any;

	fn into_untyped(value: Self, s: State) -> Result<Val> {
		Ok(match value {
			Self::Null => Val::Null,
			Self::Bool(v) => Val::Bool(v),
			Self::Number(n) => Val::Num(n.as_f64().ok_or_else(|| {
				RuntimeError(format!("json number can't be represented as jsonnet: {}", n).into())
			})?),
			Self::String(s) => Val::Str((&s as &str).into()),
			Self::Array(a) => {
				let mut out: Vec<Val> = Vec::with_capacity(a.len());
				for v in a {
					out.push(Self::into_untyped(v, s.clone())?);
				}
				Val::Arr(out.into())
			}
			Self::Object(o) => {
				let mut builder = ObjValueBuilder::with_capacity(o.len());
				for (k, v) in o {
					builder
						.member((&k as &str).into())
						.value(s.clone(), Self::into_untyped(v, s.clone())?)?;
				}
				Val::Obj(builder.build())
			}
		})
	}

	fn from_untyped(value: Val, s: State) -> Result<Self> {
		Ok(match value {
			Val::Bool(b) => Self::Bool(b),
			Val::Null => Self::Null,
			Val::Str(s) => Self::String((&s as &str).into()),
			Val::Num(n) => Self::Number(if n.fract() <= f64::EPSILON {
				(n as i64).into()
			} else {
				Number::from_f64(n).expect("jsonnet numbers can't be infinite or NaN")
			}),
			Val::Arr(a) => {
				let mut out = Vec::with_capacity(a.len());
				for item in a.iter(s.clone()) {
					out.push(Self::from_untyped(item?, s.clone())?);
				}
				Self::Array(out)
			}
			Val::Obj(o) => {
				let mut out = Map::new();
				for key in o.fields(
					#[cfg(feature = "exp-preserve-order")]
					cfg!(feature = "exp-serde-preserve-order"),
				) {
					out.insert(
						(&key as &str).into(),
						Self::from_untyped(
							o.get(s.clone(), key)?
								.expect("key is present in fields, so value should exist"),
							s.clone(),
						)?,
					);
				}
				Self::Object(out)
			}
			Val::Func(_) => throw!(RuntimeError("tried to manifest function".into())),
		})
	}
}
