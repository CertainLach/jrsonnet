use std::{borrow::Cow, fmt::Write};

use jrsonnet_evaluator::{
	bail,
	manifest::{escape_string_json_buf, ManifestFormat},
	Result, Val,
};

pub struct YamlFormat<'s> {
	/// Padding before fields, i.e
	/// ```yaml
	/// a:
	///   b:
	/// ## <- this
	/// ```
	padding: Cow<'s, str>,
	/// Padding before array elements in objects
	/// ```yaml
	/// a:
	///   - 1
	/// ## <- this
	/// ```
	arr_element_padding: Cow<'s, str>,
	/// Should yaml keys appear unescaped, when possible
	/// ```yaml
	/// "safe_key": 1
	/// # vs
	/// safe_key: 1
	/// ```
	quote_keys: bool,
	/// If true - then order of fields is preserved as written,
	/// instead of sorting alphabetically
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
}
impl YamlFormat<'_> {
	pub fn cli(
		padding: usize,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Self {
		let padding = " ".repeat(padding);
		Self {
			padding: Cow::Owned(padding.clone()),
			arr_element_padding: Cow::Owned(padding),
			quote_keys: false,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
	pub fn std_to_yaml(
		indent_array_in_object: bool,
		quote_keys: bool,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Self {
		Self {
			padding: Cow::Borrowed("  "),
			arr_element_padding: Cow::Borrowed(if indent_array_in_object { "  " } else { "" }),
			quote_keys,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
}
impl ManifestFormat for YamlFormat<'_> {
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()> {
		manifest_yaml_ex_buf(&val, buf, &mut String::new(), self)
	}
}

/// From <https://github.com/chyh1990/yaml-rust/blob/da52a68615f2ecdd6b7e4567019f280c433c1521/src/emitter.rs#L289>
/// With added date check
fn yaml_needs_quotes(string: &str) -> bool {
	fn need_quotes_spaces(string: &str) -> bool {
		string.starts_with(' ') || string.ends_with(' ')
	}

	string.is_empty()
		|| need_quotes_spaces(string)
		|| string.starts_with(|c| matches!(c, '&' | '*' | '?' | '|' | '-' | '<' | '>' | '=' | '!' | '%' | '@'))
		|| string.contains(|c| matches!(c, ':' | '{' | '}' | '[' | ']' | ',' | '#' | '`' | '\"' | '\'' | '\\' | '\0'..='\x06' | '\t' | '\n' | '\r' | '\x0e'..='\x1a' | '\x1c'..='\x1f'))
		|| [
			// http://yaml.org/type/bool.html
			"yes", "Yes", "YES", "no", "No", "NO", "True", "TRUE", "true", "False", "FALSE", "false",
			"on", "On", "ON", "off", "Off", "OFF", // http://yaml.org/type/null.html
			"null", "Null", "NULL", "~",
			// > Quoted in std.jsonnet, however, in serde_yaml they were quoted:
			// > Note: 'y', 'Y', 'n', 'N', is not quoted deliberately, as in libyaml. PyYAML also parse
			// > them as string, not booleans, although it is violating the YAML 1.1 specification.
			// > See https://github.com/dtolnay/serde-yaml/pull/83#discussion_r152628088.
			"y", "Y", "n", "N",
			"-.inf", "+.inf", ".inf",
			"-", "---", ""
		].contains(&string)
		|| (string.chars().all(|c| matches!(c, '0'..='9' | '-'))
			&& string.chars().filter(|c| *c == '-').count() == 2)
		|| string.starts_with('.')
		|| string.starts_with("0x")
		|| string.parse::<i64>().is_ok()
		|| string.parse::<f64>().is_ok()
}

#[allow(dead_code)]
fn manifest_yaml_ex(val: &Val, options: &YamlFormat<'_>) -> Result<String> {
	let mut out = String::new();
	manifest_yaml_ex_buf(val, &mut out, &mut String::new(), options)?;
	Ok(out)
}

#[allow(clippy::too_many_lines)]
fn manifest_yaml_ex_buf(
	val: &Val,
	buf: &mut String,
	cur_padding: &mut String,
	options: &YamlFormat<'_>,
) -> Result<()> {
	match val {
		Val::Bool(v) => {
			if *v {
				buf.push_str("true");
			} else {
				buf.push_str("false");
			}
		}
		Val::Null => buf.push_str("null"),
		Val::Str(s) => {
			let s = s.clone().into_flat();
			if s.is_empty() {
				buf.push_str("\"\"");
			} else if let Some(s) = s.strip_suffix('\n') {
				buf.push('|');
				for line in s.split('\n') {
					buf.push('\n');
					buf.push_str(cur_padding);
					buf.push_str(&options.padding);
					buf.push_str(line);
				}
			} else if s.contains('\n') {
				buf.push_str("|-");
				for line in s.split('\n') {
					buf.push('\n');
					buf.push_str(cur_padding);
					buf.push_str(&options.padding);
					buf.push_str(line);
				}
			} else if !options.quote_keys && !yaml_needs_quotes(&s) {
				buf.push_str(&s);
			} else {
				escape_string_json_buf(&s, buf);
			}
		}
		Val::Num(n) => write!(buf, "{}", *n).unwrap(),
		#[cfg(feature = "exp-bigint")]
		Val::BigInt(n) => write!(buf, "{}", *n).unwrap(),
		Val::Arr(a) => {
			if a.is_empty() {
				buf.push_str("[]");
			} else {
				for (i, item) in a.iter().enumerate() {
					if i != 0 {
						buf.push('\n');
						buf.push_str(cur_padding);
					}
					let item = item?;
					buf.push('-');
					match &item {
						Val::Arr(a) if !a.is_empty() => {
							buf.push('\n');
							buf.push_str(cur_padding);
							buf.push_str(&options.padding);
						}
						_ => buf.push(' '),
					}
					let extra_padding = match &item {
						Val::Arr(a) => !a.is_empty(),
						Val::Obj(o) => !o.is_empty(),
						_ => false,
					};
					let prev_len = cur_padding.len();
					if extra_padding {
						cur_padding.push_str(&options.padding);
					}
					manifest_yaml_ex_buf(&item, buf, cur_padding, options)?;
					cur_padding.truncate(prev_len);
				}
			}
		}
		Val::Obj(o) => {
			if o.is_empty() {
				buf.push_str("{}");
			} else {
				for (i, key) in o
					.fields(
						#[cfg(feature = "exp-preserve-order")]
						options.preserve_order,
					)
					.iter()
					.enumerate()
				{
					if i != 0 {
						buf.push('\n');
						buf.push_str(cur_padding);
					}
					if !options.quote_keys && !yaml_needs_quotes(key) {
						buf.push_str(key);
					} else {
						escape_string_json_buf(key, buf);
					}
					buf.push(':');
					let prev_len = cur_padding.len();
					let item = o.get(key.clone())?.expect("field exists");
					match &item {
						Val::Arr(a) if !a.is_empty() => {
							buf.push('\n');
							buf.push_str(cur_padding);
							buf.push_str(&options.arr_element_padding);
							cur_padding.push_str(&options.arr_element_padding);
						}
						Val::Obj(o) if !o.is_empty() => {
							buf.push('\n');
							buf.push_str(cur_padding);
							buf.push_str(&options.padding);
							cur_padding.push_str(&options.padding);
						}
						_ => buf.push(' '),
					}
					manifest_yaml_ex_buf(&item, buf, cur_padding, options)?;
					cur_padding.truncate(prev_len);
				}
			}
		}
		Val::Func(_) => bail!("tried to manifest function"),
	}
	Ok(())
}
