use jrsonnet_evaluator::{
	error::{Error::RuntimeError, Result},
	function::builtin,
	typed::{Any, Typed},
	IStr, State, Val,
};
use serde::Deserialize;

#[builtin]
pub fn builtin_parse_json(st: State, s: IStr) -> Result<Any> {
	use serde_json::Value;
	let value: Value = serde_json::from_str(&s)
		.map_err(|e| RuntimeError(format!("failed to parse json: {}", e).into()))?;
	Ok(Any(Value::into_untyped(value, st)?))
}

#[builtin]
pub fn builtin_parse_yaml(st: State, s: IStr) -> Result<Any> {
	use serde_json::Value;
	use serde_yaml_with_quirks::DeserializingQuirks;
	let value = serde_yaml_with_quirks::Deserializer::from_str_with_quirks(
		&s,
		DeserializingQuirks { old_octals: true },
	);
	let mut out = vec![];
	for item in value {
		let value = Value::deserialize(item)
			.map_err(|e| RuntimeError(format!("failed to parse yaml: {}", e).into()))?;
		let val = Value::into_untyped(value, st.clone())?;
		out.push(val);
	}
	Ok(Any(if out.is_empty() {
		Val::Null
	} else if out.len() == 1 {
		out.into_iter().next().unwrap()
	} else {
		Val::Arr(out.into())
	}))
}
