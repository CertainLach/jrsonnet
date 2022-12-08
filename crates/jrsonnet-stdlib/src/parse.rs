use jrsonnet_evaluator::{
	error::{ErrorKind::RuntimeError, Result},
	function::builtin,
	IStr, Val,
};
use serde::Deserialize;

#[builtin]
pub fn builtin_parse_json(str: IStr) -> Result<Val> {
	let value: Val = serde_json::from_str(&str)
		.map_err(|e| RuntimeError(format!("failed to parse json: {}", e).into()))?;
	Ok(value)
}

#[builtin]
pub fn builtin_parse_yaml(str: IStr) -> Result<Val> {
	use serde_yaml_with_quirks::DeserializingQuirks;
	let value = serde_yaml_with_quirks::Deserializer::from_str_with_quirks(
		&str,
		DeserializingQuirks { old_octals: true },
	);
	let mut out = vec![];
	for item in value {
		let val = Val::deserialize(item)
			.map_err(|e| RuntimeError(format!("failed to parse yaml: {}", e).into()))?;
		out.push(val);
	}
	Ok(if out.is_empty() {
		Val::Null
	} else if out.len() == 1 {
		out.into_iter().next().unwrap()
	} else {
		Val::Arr(out.into())
	})
}
