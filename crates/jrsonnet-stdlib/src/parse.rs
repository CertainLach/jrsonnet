use jrsonnet_evaluator::{
	error::{Error::RuntimeError, Result},
	function::builtin,
	typed::Any,
	IStr, Val,
};
use serde::Deserialize;

#[builtin]
pub fn builtin_parse_json(s: IStr) -> Result<Any> {
	let value: Val = serde_json::from_str(&s)
		.map_err(|e| RuntimeError(format!("failed to parse json: {}", e).into()))?;
	Ok(Any(value))
}

#[builtin]
pub fn builtin_parse_yaml(s: IStr) -> Result<Any> {
	use serde_yaml_with_quirks::DeserializingQuirks;
	let value = serde_yaml_with_quirks::Deserializer::from_str_with_quirks(
		&s,
		DeserializingQuirks { old_octals: true },
	);
	let mut out = vec![];
	for item in value {
		let val = Val::deserialize(item)
			.map_err(|e| RuntimeError(format!("failed to parse yaml: {}", e).into()))?;
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
