use crate::{
	Context,
	error::{Error::*, LocError, Result},
	throw, LazyBinding, LazyVal, ObjMember, ObjValue, Val,
};
use jrsonnet_parser::Visibility;
use rustc_hash::FxHasher;
use serde_json::{Map, Number, Value};
use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
	hash::BuildHasherDefault,
	rc::Rc,
};

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
				Number::from_f64(*n).expect("to json number")
			}),
			Val::Arr(a) => {
				let mut out = Vec::with_capacity(a.len());
				for item in a.iter() {
					out.push((&item?).try_into()?);
				}
				Self::Array(out)
			}
			Val::Obj(o) => {
				let mut out = Map::new();
				for key in o.fields() {
					out.insert(
						(&key as &str).into(),
						(&o.get(key)?.expect("field exists")).try_into()?,
					);
				}
				Self::Object(out)
			}
			Val::Func(_) => throw!(RuntimeError("tried to manifest function".into())),
		})
	}
}

impl From<&Value> for Val {
	fn from(v: &Value) -> Self {
		match v {
			Value::Null => Self::Null,
			Value::Bool(v) => Self::Bool(*v),
			Value::Number(n) => Self::Num(n.as_f64().expect("as f64")),
			Value::String(s) => Self::Str((s as &str).into()),
			Value::Array(a) => {
				let mut out = Vec::with_capacity(a.len());
				for v in a {
					out.push(LazyVal::new_resolved(v.into()));
				}
				Self::Arr(out.into())
			}
			Value::Object(o) => {
				let mut entries = HashMap::with_capacity_and_hasher(
					o.len(),
					BuildHasherDefault::<FxHasher>::default(),
				);
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
				Self::Obj(ObjValue::new(Context::new(), None, Rc::new(entries), Rc::new(Vec::new())))
			}
		}
	}
}
