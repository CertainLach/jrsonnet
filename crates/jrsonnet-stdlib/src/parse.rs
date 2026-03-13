use jrsonnet_evaluator::{function::builtin, runtime_error, IStr, Result, Val};

#[builtin]
pub fn builtin_parse_json(str: IStr) -> Result<Val> {
	let value: Val =
		serde_json::from_str(&str).map_err(|e| runtime_error!("failed to parse json: {e}"))?;
	Ok(value)
}

#[builtin]
pub fn builtin_parse_yaml(str: IStr) -> Result<Val> {
	let out = serde_saphyr::from_multiple_with_options::<Val>(
		&str,
		serde_saphyr::Options {
			// Golang/C++ compat
			legacy_octal_numbers: true,
			// Disable budget limits - we trust the YAML input
			budget: None,
			..Default::default()
		},
	)
	.map_err(|e| runtime_error!("failed to parse yaml: {e}"))?;
	Ok(if out.is_empty() {
		Val::Null
	} else if out.len() == 1 {
		out.into_iter().next().unwrap()
	} else {
		Val::Arr(out.into())
	})
}
