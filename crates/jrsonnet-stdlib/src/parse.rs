use jrsonnet_evaluator::{function::builtin, runtime_error, IStr, Result, Val};

#[builtin]
pub fn builtin_parse_json(str: IStr) -> Result<Val> {
	let value: Val =
		serde_json::from_str(&str).map_err(|e| runtime_error!("failed to parse json: {e}"))?;
	Ok(value)
}

#[builtin]
pub fn builtin_parse_yaml(str: IStr) -> Result<Val> {
	// Use serde-saphyr which properly handles YAML 1.1 features including:
	// - Multiple merge keys (<<) in the same mapping
	// - Octal numbers (0755 -> 493)
	// - Anchor/alias expansion
	let options = serde_saphyr::Options {
		legacy_octal_numbers: true,
		budget: None, // Disable budget limits - we trust the YAML input
		..Default::default()
	};
	let values: Vec<Val> = serde_saphyr::from_multiple_with_options(&str, options)
		.map_err(|e| runtime_error!("failed to parse yaml: {e}"))?;

	Ok(if values.is_empty() {
		Val::Null
	} else if values.len() == 1 {
		values.into_iter().next().unwrap()
	} else {
		Val::Arr(values.into())
	})
}
