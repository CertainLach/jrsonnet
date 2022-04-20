use crate::{
	error::{Error::*, LocError, Result},
	throw, ObjValueBuilder, Val,
};
use serde_json::{Map, Number, Value};
use std::convert::{TryFrom, TryInto};

impl TryFrom<&Val> for Value {
	type Error = LocError;
	fn try_from(v: &Val) -> Result<Self> {
		Ok(match v {
			Val::Bool(b) => Self::Bool(*b),
			Val::Null => Self::Null,
			Val::Str(s) => Self::String((s as &str).into()),
			Val::Num(n) => Self::Number(if n.fract() <= f64::EPSILON {
				(*n as i64).into()
			} else {
				Number::from_f64(*n).expect("jsonnet numbers can't be infinite or NaN")
			}),
			Val::Arr(a) => {
				let mut out = Vec::with_capacity(a.len());
				for item in a.iter() {
					out.push(item?.try_into()?);
				}
				Self::Array(out)
			}
			Val::Obj(o) => {
				let mut out = Map::new();
				for key in o.fields() {
					out.insert(
						(&key as &str).into(),
						o.get(key)?
							.expect("key is present in fields, so value should exist")
							.try_into()?,
					);
				}
				Self::Object(out)
			}
			Val::Func(_) => throw!(RuntimeError("tried to manifest function".into())),
		})
	}
}
impl TryFrom<Val> for Value {
	type Error = LocError;

	fn try_from(value: Val) -> Result<Self, Self::Error> {
		<Self as TryFrom<&Val>>::try_from(&value)
	}
}

impl TryFrom<&Value> for Val {
	type Error = LocError;
	fn try_from(v: &Value) -> Result<Self> {
		Ok(match v {
			Value::Null => Self::Null,
			Value::Bool(v) => Self::Bool(*v),
			Value::Number(n) => Self::Num(n.as_f64().ok_or_else(|| {
				RuntimeError(format!("json number can't be represented as jsonnet: {}", n).into())
			})?),
			Value::String(s) => Self::Str((s as &str).into()),
			Value::Array(a) => {
				let mut out: Vec<Self> = Vec::with_capacity(a.len());
				for v in a {
					out.push(v.try_into()?);
				}
				Self::Arr(out.into())
			}
			Value::Object(o) => {
				let mut builder = ObjValueBuilder::with_capacity(o.len());
				for (k, v) in o {
					builder.member((k as &str).into()).value(v.try_into()?)?;
				}
				Self::Obj(builder.build())
			}
		})
	}
}
impl TryFrom<Value> for Val {
	type Error = LocError;

	fn try_from(value: Value) -> Result<Self, Self::Error> {
		<Self as TryFrom<&Value>>::try_from(&value)
	}
}
