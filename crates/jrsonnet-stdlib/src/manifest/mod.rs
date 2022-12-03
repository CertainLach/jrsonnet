mod toml;
mod yaml;

use jrsonnet_evaluator::{
	error::Result,
	function::builtin,
	manifest::{escape_string_json, JsonFormat},
	typed::Any,
	IStr, ObjValue, Val,
};
pub use toml::TomlFormat;
pub use yaml::YamlFormat;

#[builtin]
pub fn builtin_escape_string_json(str_: IStr) -> Result<String> {
	Ok(escape_string_json(&str_))
}

#[builtin]
pub fn builtin_manifest_json_ex(
	value: Any,
	indent: IStr,
	newline: Option<IStr>,
	key_val_sep: Option<IStr>,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	let newline = newline.as_deref().unwrap_or("\n");
	let key_val_sep = key_val_sep.as_deref().unwrap_or(": ");
	value.0.manifest(JsonFormat::std_to_json(
		indent.to_string(),
		newline,
		key_val_sep,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order.unwrap_or(false),
	))
}

#[builtin]
pub fn builtin_manifest_yaml_doc(
	value: Any,
	indent_array_in_object: Option<bool>,
	quote_keys: Option<bool>,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	value.0.manifest(YamlFormat::std_to_yaml(
		indent_array_in_object.unwrap_or(false),
		quote_keys.unwrap_or(true),
		#[cfg(feature = "exp-preserve-order")]
		preserve_order.unwrap_or(false),
	))
}

#[builtin]
pub fn builtin_manifest_toml_ex(
	value: ObjValue,
	indent: IStr,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<String> {
	Val::Obj(value).manifest(TomlFormat::std_to_toml(
		indent.to_string(),
		#[cfg(feature = "exp-preserve-order")]
		preserve_order.unwrap_or(false),
	))
}
