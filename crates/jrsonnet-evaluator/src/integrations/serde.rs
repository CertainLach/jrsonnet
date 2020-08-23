use crate::{
	error::{Error::*, LocError, Result},
	throw, LazyBinding, LazyVal, ObjMember, ObjValue, Val,
};
use jrsonnet_parser::Visibility;
use serde_json::{Map, Number, Value};
use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
	rc::Rc,
};

impl TryFrom<&Val> for Value {
	type Error = LocError;
	fn try_from(v: &Val) -> Result<Self> {
		Ok(match v {
			Val::Bool(b) => Value::Bool(*b),
			Val::Null => Value::Null,
			Val::Str(s) => Value::String((&s as &str).into()),
			Val::Num(n) => Value::Number(if n.fract() <= f64::EPSILON {
				(*n as i64).into()
			} else {
				Number::from_f64(*n).expect("to json number")
			}),
			Val::Lazy(v) => (&v.evaluate()?).try_into()?,
			Val::Arr(a) => {
				let mut out = Vec::with_capacity(a.len());
				for item in a.iter() {
					out.push(item.try_into()?);
				}
				Value::Array(out)
			}
			Val::Obj(o) => {
				let mut out = Map::new();
				for key in o.visible_fields() {
					out.insert(
						(&key as &str).into(),
						(&o.get(key)?.expect("field exists")).try_into()?,
					);
				}
				Value::Object(out)
			}
			Val::Func(_) | Val::Intristic(_, _) | Val::NativeExt(_, _) => {
				throw!(RuntimeError("tried to manifest function".into()))
			}
		})
	}
}

impl From<&Value> for Val {
	fn from(v: &Value) -> Self {
		match v {
			Value::Null => Val::Null,
			Value::Bool(v) => Val::Bool(*v),
			Value::Number(n) => Val::Num(n.as_f64().expect("as f64")),
			Value::String(s) => Val::Str((s as &str).into()),
			Value::Array(a) => {
				let mut out = Vec::with_capacity(a.len());
				for v in a {
					out.push(v.into());
				}
				Val::Arr(Rc::new(out))
			}
			Value::Object(o) => {
				let mut entries = HashMap::with_capacity(o.len());
				for (k, v) in o {
					entries.insert(
						(k as &str).into(),
						ObjMember {
							add: false,
							visibility: Visibility::Normal,
							invoke: LazyBinding::Bound(LazyVal::new_resolved(v.into())),
							location: None,
						},
					);
				}
				Val::Obj(ObjValue::new(None, Rc::new(entries)))
			}
		}
	}
}
