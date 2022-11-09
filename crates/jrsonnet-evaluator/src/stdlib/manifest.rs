use std::{borrow::Cow, fmt::Write};

use crate::{
	error::{Error::*, Result},
	throw, ManifestFormat, State, Val,
};

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ManifestType {
	// Applied in manifestification
	Manifest,
	/// Used for std.manifestJson
	/// Empty array/objects extends to "[\n\n]" instead of "[ ]" as in manifest
	Std,
	/// No line breaks, used in `obj+''`
	ToString,
	/// Minified json
	Minify,
}

pub struct JsonFormat<'s> {
	padding: Cow<'s, str>,
	mtype: ManifestType,
	newline: &'s str,
	key_val_sep: &'s str,
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
}

impl<'s> JsonFormat<'s> {
	// Minifying format
	pub fn minify(#[cfg(feature = "exp-preserve-order")] preserve_order: bool) -> Self {
		Self {
			padding: Cow::Borrowed(""),
			mtype: ManifestType::Minify,
			newline: "\n",
			key_val_sep: ":",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
	// Same format as std.toString
	pub fn std_to_string() -> Self {
		Self {
			padding: Cow::Borrowed(""),
			mtype: ManifestType::ToString,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: false,
		}
	}
	pub fn std_to_json(
		padding: String,
		newline: &'s str,
		key_val_sep: &'s str,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Self {
		Self {
			padding: Cow::Owned(padding),
			mtype: ManifestType::Std,
			newline,
			key_val_sep,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
	// Same format as CLI manifestification
	pub fn cli(
		padding: usize,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Self {
		if padding == 0 {
			return Self::minify(
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			);
		}
		Self {
			padding: Cow::Owned(" ".repeat(padding)),
			mtype: ManifestType::Manifest,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
}
impl Default for JsonFormat<'static> {
	fn default() -> Self {
		Self {
			padding: Cow::Borrowed("    "),
			mtype: ManifestType::Manifest,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: false,
		}
	}
}

pub fn manifest_json_ex(val: &Val, options: &JsonFormat<'_>) -> Result<String> {
	let mut out = String::new();
	manifest_json_ex_buf(val, &mut out, &mut String::new(), options)?;
	Ok(out)
}
fn manifest_json_ex_buf(
	val: &Val,
	buf: &mut String,
	cur_padding: &mut String,
	options: &JsonFormat<'_>,
) -> Result<()> {
	let mtype = options.mtype;
	match val {
		Val::Bool(v) => {
			if *v {
				buf.push_str("true");
			} else {
				buf.push_str("false");
			}
		}
		Val::Null => buf.push_str("null"),
		Val::Str(s) => escape_string_json_buf(s, buf),
		Val::Num(n) => write!(buf, "{n}").unwrap(),
		Val::Arr(items) => {
			buf.push('[');
			if !items.is_empty() {
				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push_str(options.newline);
				}

				let old_len = cur_padding.len();
				cur_padding.push_str(&options.padding);
				for (i, item) in items.iter().enumerate() {
					if i != 0 {
						buf.push(',');
						if mtype == ManifestType::ToString {
							buf.push(' ');
						} else if mtype != ManifestType::Minify {
							buf.push_str(options.newline);
						}
					}
					buf.push_str(cur_padding);
					manifest_json_ex_buf(&item?, buf, cur_padding, options)?;
				}
				cur_padding.truncate(old_len);

				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push_str(options.newline);
					buf.push_str(cur_padding);
				}
			} else if mtype == ManifestType::Std {
				buf.push_str("\n\n");
				buf.push_str(cur_padding);
			} else if mtype == ManifestType::ToString || mtype == ManifestType::Manifest {
				buf.push(' ');
			}
			buf.push(']');
		}
		Val::Obj(obj) => {
			obj.run_assertions()?;
			buf.push('{');
			let fields = obj.fields(
				#[cfg(feature = "exp-preserve-order")]
				options.preserve_order,
			);
			if !fields.is_empty() {
				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push_str(options.newline);
				}

				let old_len = cur_padding.len();
				cur_padding.push_str(&options.padding);
				for (i, field) in fields.into_iter().enumerate() {
					if i != 0 {
						buf.push(',');
						if mtype == ManifestType::ToString {
							buf.push(' ');
						} else if mtype != ManifestType::Minify {
							buf.push_str(options.newline);
						}
					}
					buf.push_str(cur_padding);
					escape_string_json_buf(&field, buf);
					buf.push_str(options.key_val_sep);
					State::push_description(
						|| format!("field <{}> manifestification", field.clone()),
						|| {
							let value = obj.get(field.clone())?.unwrap();
							manifest_json_ex_buf(&value, buf, cur_padding, options)?;
							Ok(())
						},
					)?;
				}
				cur_padding.truncate(old_len);

				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push_str(options.newline);
					buf.push_str(cur_padding);
				}
			} else if mtype == ManifestType::Std {
				buf.push_str("\n\n");
				buf.push_str(cur_padding);
			} else if mtype == ManifestType::ToString || mtype == ManifestType::Manifest {
				buf.push(' ');
			}
			buf.push('}');
		}
		Val::Func(_) => throw!(RuntimeError("tried to manifest function".into())),
	};
	Ok(())
}

impl ManifestFormat for JsonFormat<'_> {
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()> {
		manifest_json_ex_buf(&val, buf, &mut String::new(), &self)
	}
}

pub struct ToStringFormat;
impl ManifestFormat for ToStringFormat {
	fn manifest_buf(&self, val: Val, out: &mut String) -> Result<()> {
		JsonFormat::std_to_string().manifest_buf(val, out)
	}
}
pub struct StringFormat;
impl ManifestFormat for StringFormat {
	fn manifest_buf(&self, val: Val, out: &mut String) -> Result<()> {
		let Val::Str(s) = val else {
			throw!("output should be string for string manifest format, got {}", val.value_type())
		};
		out.write_str(&s).unwrap();
		Ok(())
	}
}

pub struct YamlStreamFormat<I>(pub I);
impl<I: ManifestFormat> ManifestFormat for YamlStreamFormat<I> {
	fn manifest_buf(&self, val: Val, out: &mut String) -> Result<()> {
		let Val::Arr(arr) = val else {
			throw!("output should be array for yaml stream format, got {}", val.value_type())
		};
		if !arr.is_empty() {
			for v in arr.iter() {
				let v = v?;
				out.push_str("---\n");
				self.0.manifest_buf(v, out)?;
				out.push('\n');
			}
			out.push_str("...");
		}
		Ok(())
	}
}

pub fn escape_string_json(s: &str) -> String {
	let mut buf = String::new();
	escape_string_json_buf(s, &mut buf);
	buf
}

fn escape_string_json_buf(s: &str, buf: &mut String) {
	buf.push('"');
	for c in s.chars() {
		match c {
			'"' => buf.push_str("\\\""),
			'\\' => buf.push_str("\\\\"),
			'\u{0008}' => buf.push_str("\\b"),
			'\u{000c}' => buf.push_str("\\f"),
			'\n' => buf.push_str("\\n"),
			'\r' => buf.push_str("\\r"),
			'\t' => buf.push_str("\\t"),
			c if c < 32 as char || (c >= 127 as char && c <= 159 as char) => {
				write!(buf, "\\u{:04x}", c as u32).unwrap();
			}
			c => buf.push(c),
		}
	}
	buf.push('"');
}

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
			// Note: 'y', 'Y', 'n', 'N', is not quoted deliberately, as in libyaml. PyYAML also parse
			// them as string, not booleans, although it is violating the YAML 1.1 specification.
			// See https://github.com/dtolnay/serde-yaml/pull/83#discussion_r152628088.
			"yes", "Yes", "YES", "no", "No", "NO", "True", "TRUE", "true", "False", "FALSE", "false",
			"on", "On", "ON", "off", "Off", "OFF", // http://yaml.org/type/null.html
			"null", "Null", "NULL", "~",
		].contains(&string)
		|| (string.chars().all(|c| matches!(c, '0'..='9' | '-'))
			&& string.chars().filter(|c| *c == '-').count() == 2)
		|| string.starts_with('.')
		|| string.starts_with("0x")
		|| string.parse::<i64>().is_ok()
		|| string.parse::<f64>().is_ok()
}

pub fn manifest_yaml_ex(val: &Val, options: &YamlFormat<'_>) -> Result<String> {
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
			} else if !options.quote_keys && !yaml_needs_quotes(s) {
				buf.push_str(s);
			} else {
				escape_string_json_buf(s, buf);
			}
		}
		Val::Num(n) => write!(buf, "{}", *n).unwrap(),
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
		Val::Func(_) => throw!("tried to manifest function"),
	}
	Ok(())
}
