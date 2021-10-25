use crate::error::Error::*;
use crate::error::Result;
use crate::{throw, Val};

#[derive(PartialEq, Clone, Copy)]
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

pub struct ManifestJsonOptions<'s> {
	pub padding: &'s str,
	pub mtype: ManifestType,
}

pub fn manifest_json_ex(val: &Val, options: &ManifestJsonOptions<'_>) -> Result<String> {
	let mut out = String::new();
	manifest_json_ex_buf(val, &mut out, &mut String::new(), options)?;
	Ok(out)
}
fn manifest_json_ex_buf(
	val: &Val,
	buf: &mut String,
	cur_padding: &mut String,
	options: &ManifestJsonOptions<'_>,
) -> Result<()> {
	use std::fmt::Write;
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
		Val::Num(n) => write!(buf, "{}", n).unwrap(),
		Val::Arr(items) => {
			buf.push('[');
			if !items.is_empty() {
				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push('\n');
				}

				let old_len = cur_padding.len();
				cur_padding.push_str(options.padding);
				for (i, item) in items.iter().enumerate() {
					if i != 0 {
						buf.push(',');
						if mtype == ManifestType::ToString {
							buf.push(' ');
						} else if mtype != ManifestType::Minify {
							buf.push('\n');
						}
					}
					buf.push_str(cur_padding);
					manifest_json_ex_buf(&item?, buf, cur_padding, options)?;
				}
				cur_padding.truncate(old_len);

				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push('\n');
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
			let fields = obj.fields();
			if !fields.is_empty() {
				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push('\n');
				}

				let old_len = cur_padding.len();
				cur_padding.push_str(options.padding);
				for (i, field) in fields.into_iter().enumerate() {
					if i != 0 {
						buf.push(',');
						if mtype == ManifestType::ToString {
							buf.push(' ');
						} else if mtype != ManifestType::Minify {
							buf.push('\n');
						}
					}
					buf.push_str(cur_padding);
					escape_string_json_buf(&field, buf);
					buf.push_str(": ");
					crate::push(
						None,
						|| format!("field <{}> manifestification", field.clone()),
						|| {
							let value = obj.get(field.clone())?.unwrap();
							manifest_json_ex_buf(&value, buf, cur_padding, options)
						},
					)?;
				}
				cur_padding.truncate(old_len);

				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push('\n');
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

pub fn escape_string_json(s: &str) -> String {
	let mut buf = String::new();
	escape_string_json_buf(s, &mut buf);
	buf
}

fn escape_string_json_buf(s: &str, buf: &mut String) {
	use std::fmt::Write;
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
				write!(buf, "\\u{:04x}", c as u32).unwrap()
			}
			c => buf.push(c),
		}
	}
	buf.push('"');
}

pub struct ManifestYamlOptions<'s> {
	/// Padding before fields, i.e
	/// a:
	///   b:
	/// ^^ this
	pub padding: &'s str,
	/// Padding before array elements in objects
	/// a:
	///   - 1
	/// ^^ this
	pub arr_element_padding: &'s str,
}

pub fn manifest_yaml_ex(val: &Val, options: &ManifestYamlOptions<'_>) -> Result<String> {
	let mut out = String::new();
	manifest_yaml_ex_buf(val, &mut out, &mut String::new(), options)?;
	Ok(out)
}
fn manifest_yaml_ex_buf(
	val: &Val,
	buf: &mut String,
	cur_padding: &mut String,
	options: &ManifestYamlOptions<'_>,
) -> Result<()> {
	use std::fmt::Write;
	match val {
		Val::Bool(v) => {
			if *v {
				buf.push_str("true")
			} else {
				buf.push_str("false")
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
					buf.push_str(options.padding);
					buf.push_str(line);
				}
			} else {
				escape_string_json_buf(s, buf)
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
					if let Val::Arr(a) = &item {
						if !a.is_empty() {
							buf.push('\n');
							buf.push_str(cur_padding);
							buf.push_str(options.padding);
						} else {
							buf.push(' ');
						}
					} else {
						buf.push(' ');
					}
					let extra_padding = if let Val::Arr(a) = &item {
						!a.is_empty()
					} else if let Val::Obj(a) = &item {
						!a.is_empty()
					} else {
						false
					};
					let prev_len = cur_padding.len();
					if extra_padding {
						cur_padding.push_str(options.padding);
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
				for (i, key) in o.fields().iter().enumerate() {
					if i != 0 {
						buf.push('\n');
						buf.push_str(cur_padding);
					}
					escape_string_json_buf(key, buf);
					buf.push(':');
					let item = o.get(key.clone())?.expect("field exists");
					if let Val::Arr(a) = &item {
						if !a.is_empty() {
							buf.push('\n');
							buf.push_str(cur_padding);
							buf.push_str(options.arr_element_padding);
						} else {
							buf.push(' ');
						}
					} else if let Val::Obj(o) = &item {
						if !o.is_empty() {
							buf.push('\n');
							buf.push_str(cur_padding);
							buf.push_str(options.padding);
						} else {
							buf.push(' ');
						}
					} else {
						buf.push(' ');
					}
					let prev_len = cur_padding.len();
					if let Val::Arr(a) = &item {
						if !a.is_empty() {
							cur_padding.push_str(options.arr_element_padding);
						}
					} else if let Val::Obj(a) = &item {
						if !a.is_empty() {
							cur_padding.push_str(options.padding);
						}
					};
					manifest_yaml_ex_buf(&item, buf, cur_padding, options)?;
					cur_padding.truncate(prev_len);
				}
			}
		}
		Val::Func(_) => throw!(RuntimeError("tried to manifest function".into())),
	}
	Ok(())
}
