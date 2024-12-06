use jrsonnet_evaluator::{function::builtin, runtime_error, IStr, Result, Val};

#[builtin]
pub fn builtin_parse_json(str: IStr) -> Result<Val> {
	let value: Val =
		serde_json::from_str(&str).map_err(|e| runtime_error!("failed to parse json: {e}"))?;
	Ok(value)
}

#[builtin]
#[cfg(feature = "yaml")]
pub fn builtin_parse_yaml(str: IStr) -> Result<Val> {
	use serde::Deserialize;
	use serde_yaml_with_quirks::DeserializingQuirks;

	let value = serde_yaml_with_quirks::Deserializer::from_str_with_quirks(
		&str,
		DeserializingQuirks { old_octals: true },
	);
	let mut out = vec![];
	for item in value {
		let val =
			Val::deserialize(item).map_err(|e| runtime_error!("failed to parse yaml: {e}"))?;
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
