mod python;
mod toml;
mod xml;
mod yaml;

use jrsonnet_evaluator::{
	function::builtin,
	manifest::{escape_string_json, JsonFormat, YamlStreamFormat},
	IStr, ObjValue, Result, Val,
};
pub use python::{PythonFormat, PythonVarsFormat};
pub use toml::TomlFormat;
pub use xml::XmlJsonmlFormat;
pub use yaml::YamlFormat;

#[builtin]
pub fn builtin_escape_string_json(str_: IStr) -> Result<String> {
	Ok(escape_string_json(&str_))
}

#[builtin]
pub fn builtin_manifest_json_ex(
	value: Val,
	indent: String,
	newline: Option<IStr>,
	key_val_sep: Option<IStr>,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	let newline = newline.as_deref().unwrap_or("\n");
	let key_val_sep = key_val_sep.as_deref().unwrap_or(": ");
	value.manifest(JsonFormat::std_to_json(
		indent,
		newline,
		key_val_sep,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	))
}

#[builtin]
pub fn builtin_manifest_json(
	value: Val,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	builtin_manifest_json_ex(
		value,
		"    ".to_owned(),
		None,
		None,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}

#[builtin]
pub fn builtin_manifest_json_minified(
	value: Val,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	value.manifest(JsonFormat::minify(
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	))
}

#[builtin]
pub fn builtin_manifest_yaml_doc(
	value: Val,
	#[default(false)] indent_array_in_object: bool,
	#[default(true)] quote_keys: bool,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	value.manifest(YamlFormat::std_to_yaml(
		indent_array_in_object,
		quote_keys,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	))
}

#[builtin]
pub fn builtin_manifest_yaml_stream(
	value: Val,
	#[default(false)] indent_array_in_object: bool,
	#[default(true)] c_document_end: bool,
	#[default(true)] quote_keys: bool,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	value.manifest(YamlStreamFormat::std_yaml_stream(
		YamlFormat::std_to_yaml(
			indent_array_in_object,
			quote_keys,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		),
		c_document_end,
	))
}

#[builtin]
pub fn builtin_manifest_toml_ex(
	value: ObjValue,
	indent: String,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	Val::Obj(value).manifest(TomlFormat::std_to_toml(
		indent,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	))
}

#[builtin]
pub fn builtin_manifest_toml(
	value: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	builtin_manifest_toml_ex(
		value,
		"  ".to_owned(),
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}

#[builtin]
pub fn builtin_to_string(a: Val) -> Result<IStr> {
	a.to_string()
}

#[builtin]
pub fn builtin_manifest_python(
	v: Val,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	v.manifest(PythonFormat::std(
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	))
}
#[builtin]
pub fn builtin_manifest_python_vars(
	v: Val,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Result<String> {
	v.manifest(PythonVarsFormat::std(
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	))
}

#[builtin]
pub fn builtin_escape_string_xml(str_: String) -> String {
	xml::escape_string_xml(str_.as_str())
}

#[builtin]
pub fn builtin_manifest_xml_jsonml(value: Val) -> Result<String> {
	value.manifest(XmlJsonmlFormat::std_to_xml())
}
