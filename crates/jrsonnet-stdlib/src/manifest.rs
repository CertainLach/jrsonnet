use jrsonnet_evaluator::{
	error::Result,
	function::builtin,
	stdlib::manifest::{
		escape_string_json, manifest_json_ex, manifest_yaml_ex, ManifestJsonOptions, ManifestType,
		ManifestYamlOptions,
	},
	typed::Any,
	IStr, State,
};

#[builtin]
pub fn builtin_escape_string_json(str_: IStr) -> Result<String> {
	Ok(escape_string_json(&str_))
}

#[builtin]
pub fn builtin_manifest_json_ex(
	s: State,
	value: Any,
	indent: IStr,
	newline: Option<IStr>,
	key_val_sep: Option<IStr>,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	let newline = newline.as_deref().unwrap_or("\n");
	let key_val_sep = key_val_sep.as_deref().unwrap_or(": ");
	manifest_json_ex(
		s,
		&value.0,
		&ManifestJsonOptions {
			padding: &indent,
			mtype: ManifestType::Std,
			newline,
			key_val_sep,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: preserve_order.unwrap_or(false),
		},
	)
}

#[builtin]
pub fn builtin_manifest_yaml_doc(
	s: State,
	value: Any,
	indent_array_in_object: Option<bool>,
	quote_keys: Option<bool>,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	manifest_yaml_ex(
		s,
		&value.0,
		&ManifestYamlOptions {
			padding: "  ",
			arr_element_padding: if indent_array_in_object.unwrap_or(false) {
				"  "
			} else {
				""
			},
			quote_keys: quote_keys.unwrap_or(true),
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: preserve_order.unwrap_or(false),
		},
	)
}
