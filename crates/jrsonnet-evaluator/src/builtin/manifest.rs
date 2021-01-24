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
		Val::Str(s) => buf.push_str(&escape_string_json(s)),
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
			} else if mtype == ManifestType::ToString {
				buf.push(' ');
			}
			buf.push(']');
		}
		Val::Obj(obj) => {
			buf.push('{');
			let fields = obj.visible_fields();
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
					buf.push_str(&escape_string_json(&field));
					buf.push_str(": ");
					manifest_json_ex_buf(&obj.get(field)?.unwrap(), buf, cur_padding, options)?;
				}
				cur_padding.truncate(old_len);

				if mtype != ManifestType::ToString && mtype != ManifestType::Minify {
					buf.push('\n');
					buf.push_str(cur_padding);
				}
			} else if mtype == ManifestType::Std {
				buf.push_str("\n\n");
				buf.push_str(cur_padding);
			} else if mtype == ManifestType::ToString {
				buf.push(' ');
			}
			buf.push('}');
		}
		Val::Func(_) => throw!(RuntimeError("tried to manifest function".into())),
	};
	Ok(())
}
pub fn escape_string_json(s: &str) -> String {
	use std::fmt::Write;
	let mut out = String::new();
	out.push('"');
	for c in s.chars() {
		match c {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\u{0008}' => out.push_str("\\b"),
			'\u{000c}' => out.push_str("\\f"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c if c < 32 as char || (c >= 127 as char && c <= 159 as char) => {
				write!(out, "\\u{:04x}", c as u32).unwrap()
			}
			c => out.push(c),
		}
	}
	out.push('"');
	out
}

#[test]
fn json_test() {
	assert_eq!(escape_string_json("\u{001f}"), "\"\\u001f\"")
}
