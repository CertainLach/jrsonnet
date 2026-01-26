use std::{borrow::Cow, fmt::Write};

use jrsonnet_evaluator::{
	bail, in_description_frame,
	manifest::{escape_string_json_buf, ManifestFormat},
	Result, ResultExt, Val,
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
	/// Should yaml string values always be quoted
	/// go-jsonnet always quotes string values in manifestYamlDoc
	/// ```yaml
	/// key: "value"
	/// # vs
	/// key: value
	/// ```
	quote_values: bool,
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
			quote_values: false,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
	pub fn std_to_yaml(
		indent_array_in_object: bool,
		quote_keys: bool,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Self {
		Self::std_to_yaml_with_settings(
			indent_array_in_object,
			quote_keys,
			true, // go-jsonnet always quotes string values
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		)
	}

	pub fn std_to_yaml_with_settings(
		indent_array_in_object: bool,
		quote_keys: bool,
		quote_values: bool,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Self {
		Self {
			padding: Cow::Borrowed("  "),
			arr_element_padding: Cow::Borrowed(if indent_array_in_object { "  " } else { "" }),
			quote_keys,
			quote_values,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		}
	}
}
impl ManifestFormat for YamlFormat<'_> {
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()> {
		manifest_yaml_ex_buf(&val, buf, &mut String::new(), self, false)
	}
}

/// Check if string looks like an ISO8601 timestamp that YAML parsers might interpret as a date.
/// Examples: 2025-07-03T15:30:00Z, 2025-07-03T15:30:00+00:00
fn looks_like_timestamp(string: &str) -> bool {
	// Quick length check: ISO8601 timestamps are at least 20 chars (2025-07-03T15:30:00Z)
	if string.len() < 20 {
		return false;
	}
	// Check for ISO8601 pattern: YYYY-MM-DDTHH:MM:SS followed by Z or timezone
	let bytes = string.as_bytes();
	// Check date part: YYYY-MM-DD
	if !(bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T') {
		return false;
	}
	// Check time part: HH:MM:SS
	if !(bytes[13] == b':' && bytes[16] == b':') {
		return false;
	}
	// Check all digits are in right places
	bytes[0..4].iter().all(u8::is_ascii_digit)
		&& bytes[5..7].iter().all(u8::is_ascii_digit)
		&& bytes[8..10].iter().all(u8::is_ascii_digit)
		&& bytes[11..13].iter().all(u8::is_ascii_digit)
		&& bytes[14..16].iter().all(u8::is_ascii_digit)
		&& bytes[17..19].iter().all(u8::is_ascii_digit)
		// Check timezone indicator (Z, +, or -)
		&& (bytes[19] == b'Z' || bytes[19] == b'+' || bytes[19] == b'-')
}

/// From <https://github.com/chyh1990/yaml-rust/blob/da52a68615f2ecdd6b7e4567019f280c433c1521/src/emitter.rs#L289>
/// With added date check and go-jsonnet compatibility
fn yaml_needs_quotes(string: &str) -> bool {
	fn need_quotes_spaces(string: &str) -> bool {
		string.starts_with(' ') || string.ends_with(' ')
	}

	string.is_empty()
		|| need_quotes_spaces(string)
		// Characters that are special at the start of a YAML value
		|| string.starts_with(['&', '*', '?', '|', '-', '!', '%', '@', '{', '[', '"', '\'', '<', '>'])
		// Colon anywhere creates key-value ambiguity - jrsonnet quotes any string with colon
		|| string.contains(':')
		// Comma anywhere - jrsonnet quotes any string with comma
		|| string.contains(',')
		// Flow indicators anywhere in string need quoting
		// This includes { } [ ] which could be interpreted as flow sequences/mappings
		|| string.contains('{')
		|| string.contains('}')
		|| string.contains('[')
		|| string.contains(']')
		// # starts a comment in YAML - jrsonnet quotes any string containing #
		// to avoid potential parsing issues even mid-string (e.g., URLs with anchors)
		|| string.contains('#')
		|| string.contains(|c| matches!(c, '`' | '\0'..='\x06' | '\t' | '\n' | '\r' | '\x0e'..='\x1a' | '\x1c'..='\x1f'))
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
		|| string.parse::<f64>().is_ok_and(|f| f.is_finite())
		// ISO8601 timestamps should be quoted to prevent YAML parsers from
		// interpreting them as dates (matches Go's yaml.v3 behavior)
		|| looks_like_timestamp(string)
		// Strings containing quotes need quoting
		|| string.contains('\'')
		|| string.contains('"')
}

/// Escape a string for YAML with intelligent quote selection
/// Always uses double quotes with JSON-style escaping to match both go-jsonnet and jrsonnet behavior.
/// Go's yaml.v3 uses single quotes when possible, but both go-jsonnet and jrsonnet use double quotes.
fn escape_string_yaml_buf(s: &str, buf: &mut String, _quote_values: bool) {
	// Always use double quotes with JSON-style escaping
	// This matches both go-jsonnet (quote_values=true) and jrsonnet (quote_values=false) behavior
	escape_string_json_buf(s, buf);
}

#[allow(dead_code)]
fn manifest_yaml_ex(val: &Val, options: &YamlFormat<'_>) -> Result<String> {
	let mut out = String::new();
	manifest_yaml_ex_buf(val, &mut out, &mut String::new(), options, false)?;
	Ok(out)
}

#[allow(clippy::too_many_lines)]
fn manifest_yaml_ex_buf(
	val: &Val,
	buf: &mut String,
	cur_padding: &mut String,
	options: &YamlFormat<'_>,
	// When true, this object is an array element and its fields should be
	// indented to align with the first field (after "- ")
	in_array_context: bool,
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
			} else if s.ends_with('\n') {
				// Block scalar with trailing newline - use | (clip: adds one trailing newline when parsed)
				// Go's manifestYamlDoc uses block scalar for strings ending with \n
				let content = s.strip_suffix('\n').unwrap();
				buf.push('|');
				for line in content.split('\n') {
					buf.push('\n');
					buf.push_str(cur_padding);
					buf.push_str(&options.padding);
					buf.push_str(line);
				}
			} else if !options.quote_values && s.contains('\n') {
				// Block scalar without trailing newline - use |- (strip: no trailing newline when parsed)
				// Only use block scalars when quote_values is false (CLI mode).
				// When quote_values is true (std.manifestYamlDoc), Go uses double-quoted for
				// strings that contain \n but don't end with \n.
				buf.push_str("|-");
				for line in s.split('\n') {
					buf.push('\n');
					buf.push_str(cur_padding);
					buf.push_str(&options.padding);
					buf.push_str(line);
				}
			} else if !options.quote_values && !yaml_needs_quotes(&s) {
				// Simple string without newlines - output unquoted if safe
				buf.push_str(&s);
			} else {
				// Use intelligent quote selection to match Go's yaml.v3 behavior
				escape_string_yaml_buf(&s, buf, options.quote_values);
			}
		}
		Val::Num(n) => {
			// Go-jsonnet uses strconv.FormatFloat(v, 'f', -1, 64) - shortest decimal representation
			// No scientific notation is used in manifestYamlDoc
			let v = n.get();
			if v.fract() == 0.0 {
				// Integer: write without decimal point
				write!(buf, "{}", v as i64).unwrap();
			} else {
				// Float: use shortest representation (Rust's default)
				write!(buf, "{v}").unwrap();
			}
		}
		#[cfg(feature = "exp-bigint")]
		Val::BigInt(n) => write!(buf, "{}", *n).unwrap(),
		Val::Arr(a) => {
			let mut had_items = false;
			for (i, item) in a.iter().enumerate() {
				had_items = true;
				let item = item.with_description(|| format!("elem <{i}> evaluation"))?;
				if i != 0 {
					buf.push('\n');
					buf.push_str(cur_padding);
				}
				buf.push('-');
				match &item {
					Val::Arr(a) if !a.is_empty() => {
						// Nested arrays need a newline and extra indentation
						buf.push('\n');
						buf.push_str(cur_padding);
						buf.push_str(&options.padding);
					}
					_ => buf.push(' '),
				}
				// For nested arrays, add padding to cur_padding
				let prev_len = cur_padding.len();
				if let Val::Arr(a) = &item {
					if !a.is_empty() {
						cur_padding.push_str(&options.padding);
					}
				}
				// Objects in arrays need special handling: their fields should
				// align with the first field (after "- "), but nested structures
				// should not inherit this offset
				let is_object_in_array = matches!(&item, Val::Obj(o) if !o.is_empty());
				in_description_frame(
					|| format!("elem <{i}> manifestification"),
					|| manifest_yaml_ex_buf(&item, buf, cur_padding, options, is_object_in_array),
				)?;
				cur_padding.truncate(prev_len);
			}
			if !had_items {
				buf.push_str("[]");
			}
		}
		Val::Obj(o) => {
			let mut had_fields = false;
			// Store the base padding BEFORE any in_array_context adjustment.
			let base_padding_len = cur_padding.len();

			// For key alignment: if this object is an array element, keys (except the first)
			// need 2 extra spaces to align with the first key (which appears after "- ").
			// This offset is ONLY for key alignment, NOT for nested content.
			let key_padding = if in_array_context {
				let mut kp = cur_padding.clone();
				kp.push_str("  ");
				kp
			} else {
				cur_padding.clone()
			};

			for (i, (key, value)) in o
				.iter(
					#[cfg(feature = "exp-preserve-order")]
					options.preserve_order,
				)
				.enumerate()
			{
				had_fields = true;
				let value = value.with_description(|| format!("field <{key}> evaluation"))?;
				if i != 0 {
					buf.push('\n');
					buf.push_str(&key_padding);
				}
				if !options.quote_keys && !yaml_needs_quotes(&key) {
					buf.push_str(&key);
				} else {
					escape_string_json_buf(&key, buf);
				}
				buf.push(':');

				// For nested content (arrays/objects as values), we need to account for
				// whether this object is an array element. If so, the first field starts
				// at cur_padding + 2 (after "- "), so nested content should be relative
				// to that position.
				//
				// When in_array_context, we add +2 to account for the "- " prefix, but we
				// DON'T add arr_element_padding for arrays - the +2 offset already provides
				// the correct indentation. For non-array context, we DO add arr_element_padding.
				let content_base = if in_array_context {
					let mut base = cur_padding.clone();
					base.push_str("  ");
					base
				} else {
					cur_padding.clone()
				};

				let prev_len = cur_padding.len();
				match &value {
					Val::Arr(a) if !a.is_empty() => {
						buf.push('\n');
						// For arrays in object fields, use content_base (which includes the
						// in_array_context alignment) plus arr_element_padding.
						buf.push_str(&content_base);
						buf.push_str(&options.arr_element_padding);
						// Set cur_padding for nested content
						cur_padding.clear();
						cur_padding.push_str(&content_base);
						cur_padding.push_str(&options.arr_element_padding);
					}
					Val::Obj(o) if !o.is_empty() => {
						buf.push('\n');
						buf.push_str(&content_base);
						buf.push_str(&options.padding);
						// Set cur_padding for nested content
						cur_padding.clear();
						cur_padding.push_str(&content_base);
						cur_padding.push_str(&options.padding);
					}
					_ => {
						buf.push(' ');
						// Set cur_padding for block scalar indentation in array context
						// This ensures block scalar content is indented relative to key position
						if in_array_context {
							cur_padding.clear();
							cur_padding.push_str(&content_base);
						}
					}
				}
				in_description_frame(
					|| format!("field <{key}> manifestification"),
					|| manifest_yaml_ex_buf(&value, buf, cur_padding, options, false),
				)?;
				cur_padding.truncate(prev_len);
			}
			// Restore cur_padding to original value
			cur_padding.truncate(base_padding_len);
			if !had_fields {
				buf.push_str("{}");
			}
		}
		Val::Func(_) => bail!("tried to manifest function"),
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use jrsonnet_evaluator::{
		val::{ArrValue, NumValue},
		ObjValueBuilder,
	};

	use super::*;

	/// Helper to create a Val::Num
	fn num(v: f64) -> Val {
		Val::Num(NumValue::new(v).expect("finite"))
	}

	/// Helper to create a Val::Str
	fn str_val(s: &str) -> Val {
		Val::Str(s.into())
	}

	/// Helper to create a Val::Arr
	fn arr(items: Vec<Val>) -> Val {
		Val::Arr(ArrValue::eager(items))
	}

	/// Helper to manifest a value to YAML string using std_to_yaml options
	/// Uses quote_keys: false to match typical usage
	fn manifest_yaml(val: &Val) -> String {
		let options = YamlFormat::std_to_yaml(
			false, // indent_array_in_object
			false, // quote_keys - false to not quote keys unnecessarily
			#[cfg(feature = "exp-preserve-order")]
			false,
		);
		let mut out = String::new();
		manifest_yaml_ex_buf(val, &mut out, &mut String::new(), &options, false).unwrap();
		out
	}

	/// Helper to manifest with indent_array_in_object: true
	fn manifest_yaml_indented(val: &Val) -> String {
		let options = YamlFormat::std_to_yaml(
			true,  // indent_array_in_object
			false, // quote_keys
			#[cfg(feature = "exp-preserve-order")]
			false,
		);
		let mut out = String::new();
		manifest_yaml_ex_buf(val, &mut out, &mut String::new(), &options, false).unwrap();
		out
	}

	// ==========================================================================
	// Tests for looks_like_timestamp function
	// ==========================================================================

	#[test]
	fn test_looks_like_timestamp_with_z_suffix() {
		assert!(looks_like_timestamp("2025-07-03T15:30:00Z"));
		assert!(looks_like_timestamp("2024-12-25T00:00:00Z"));
		assert!(looks_like_timestamp("1999-01-01T23:59:59Z"));
	}

	#[test]
	fn test_looks_like_timestamp_with_positive_offset() {
		assert!(looks_like_timestamp("2025-07-03T15:30:00+00:00"));
		assert!(looks_like_timestamp("2025-07-03T15:30:00+05:30"));
	}

	#[test]
	fn test_looks_like_timestamp_with_negative_offset() {
		assert!(looks_like_timestamp("2025-07-03T15:30:00-07:00"));
		assert!(looks_like_timestamp("2025-07-03T15:30:00-12:00"));
	}

	#[test]
	fn test_looks_like_timestamp_rejects_invalid() {
		// Too short
		assert!(!looks_like_timestamp("2025-07-03"));
		assert!(!looks_like_timestamp("2025-07-03T15:30"));
		// Wrong format
		assert!(!looks_like_timestamp("not a timestamp at all"));
		assert!(!looks_like_timestamp("2025/07/03T15:30:00Z"));
		// Missing T separator
		assert!(!looks_like_timestamp("2025-07-03 15:30:00Z"));
	}

	// ==========================================================================
	// Tests for yaml_needs_quotes function
	// ==========================================================================

	#[test]
	fn test_yaml_needs_quotes_for_timestamps() {
		// ISO8601 timestamps should be quoted to match Go's yaml.v3 behavior
		assert!(yaml_needs_quotes("2025-07-03T15:30:00Z"));
		assert!(yaml_needs_quotes("2025-07-03T15:30:00+00:00"));
		assert!(yaml_needs_quotes("2025-07-03T15:30:00-05:00"));
	}

	#[test]
	fn test_yaml_needs_quotes_for_booleans() {
		assert!(yaml_needs_quotes("true"));
		assert!(yaml_needs_quotes("false"));
		assert!(yaml_needs_quotes("True"));
		assert!(yaml_needs_quotes("False"));
		assert!(yaml_needs_quotes("TRUE"));
		assert!(yaml_needs_quotes("FALSE"));
		assert!(yaml_needs_quotes("yes"));
		assert!(yaml_needs_quotes("no"));
		assert!(yaml_needs_quotes("on"));
		assert!(yaml_needs_quotes("off"));
	}

	#[test]
	fn test_yaml_needs_quotes_for_null() {
		assert!(yaml_needs_quotes("null"));
		assert!(yaml_needs_quotes("Null"));
		assert!(yaml_needs_quotes("NULL"));
		assert!(yaml_needs_quotes("~"));
	}

	#[test]
	fn test_yaml_needs_quotes_for_numbers() {
		assert!(yaml_needs_quotes("123"));
		assert!(yaml_needs_quotes("3.14"));
		assert!(yaml_needs_quotes("-42"));
		assert!(yaml_needs_quotes("0x1F"));
	}

	#[test]
	fn test_yaml_needs_quotes_for_special_chars() {
		assert!(yaml_needs_quotes("key: value")); // colon followed by space
		assert!(yaml_needs_quotes("hello #world")); // hash anywhere requires quoting
		assert!(yaml_needs_quotes("hello#world")); // hash anywhere requires quoting (matches jrsonnet)
		assert!(yaml_needs_quotes("{json}")); // starts with brace
		assert!(yaml_needs_quotes("[array]")); // starts with bracket
										 // Braces/brackets anywhere require quoting to match go-jsonnet behavior
		assert!(yaml_needs_quotes("hello{world}"));
		assert!(yaml_needs_quotes("hello[world]"));
		// Quotes at start need quoting
		assert!(yaml_needs_quotes("\"quoted\"")); // starts with double quote
		assert!(yaml_needs_quotes("'quoted'")); // starts with single quote
										  // Single quotes anywhere need quoting in jrsonnet
		assert!(yaml_needs_quotes("hello'world")); // single quote in middle
											 // Double quotes anywhere need quoting in jrsonnet
		assert!(yaml_needs_quotes("hello\"world")); // double quote in middle
		assert!(yaml_needs_quotes("job=\"api-server\"")); // typical promql/yaml pattern
	}

	#[test]
	fn test_yaml_no_quotes_needed_for_simple_strings() {
		assert!(!yaml_needs_quotes("hello"));
		assert!(!yaml_needs_quotes("simple_key"));
		assert!(!yaml_needs_quotes("CamelCase"));
		assert!(!yaml_needs_quotes("with-dash"));
	}

	// ==========================================================================
	// Tests for number formatting
	// ==========================================================================

	#[test]
	fn test_number_large_integers() {
		// Go-jsonnet uses decimal format for all integers (no scientific notation)
		assert_eq!(manifest_yaml(&num(10_000_000.0)), "10000000");
		assert_eq!(manifest_yaml(&num(5_625_000.0)), "5625000");
		assert_eq!(manifest_yaml(&num(536_870_912.0)), "536870912");
		assert_eq!(manifest_yaml(&num(100_000_000_000.0)), "100000000000");
	}

	#[test]
	fn test_number_smaller_values() {
		// All integers use plain integer format (no scientific notation in go-jsonnet)
		assert_eq!(manifest_yaml(&num(50000.0)), "50000");
		assert_eq!(manifest_yaml(&num(999999.0)), "999999");
		assert_eq!(manifest_yaml(&num(100.0)), "100");
		assert_eq!(manifest_yaml(&num(0.0)), "0");
		assert_eq!(manifest_yaml(&num(1_000_000.0)), "1000000"); // exactly 1 million, decimal format
	}

	#[test]
	fn test_number_floats() {
		// Floats with fractional parts should not use scientific notation
		assert_eq!(manifest_yaml(&num(3.14)), "3.14");
		assert_eq!(manifest_yaml(&num(0.5)), "0.5");
	}

	#[test]
	fn test_number_negative() {
		assert_eq!(manifest_yaml(&num(-100.0)), "-100");
		assert_eq!(manifest_yaml(&num(-10_000_000.0)), "-10000000");
	}

	// ==========================================================================
	// Tests for timestamp quoting in YAML output
	// These tests verify Go yaml.v3 compatibility for ISO8601 timestamps
	// ==========================================================================

	#[test]
	fn test_timestamp_is_quoted_in_yaml() {
		// ISO8601 timestamps should be quoted in YAML output
		let result = manifest_yaml(&str_val("2025-07-03T15:30:00Z"));
		assert_eq!(result, "\"2025-07-03T15:30:00Z\"");
	}

	#[test]
	fn test_timestamp_with_offset_is_quoted() {
		let result = manifest_yaml(&str_val("2025-07-03T15:30:00+05:30"));
		assert_eq!(result, "\"2025-07-03T15:30:00+05:30\"");
	}

	// ==========================================================================
	// Tests for object-in-array field alignment
	// These tests verify Go yaml.v3 compatibility for object indentation
	// ==========================================================================

	#[test]
	fn test_object_in_array_field_alignment() {
		// When an object is an array element, subsequent fields should align
		// with the first field (after "- ")
		let mut builder = ObjValueBuilder::new();
		builder.field("prefix").value(str_val("ingester-data"));
		builder.field("pruneFrequency").value(str_val("8h"));
		let val = arr(vec![Val::Obj(builder.build())]);

		let result = manifest_yaml(&val);

		// The format should be:
		// - prefix: ingester-data
		//   pruneFrequency: 8h
		// Verify the alignment by checking indentation
		let lines: Vec<&str> = result.lines().collect();
		assert_eq!(lines.len(), 2);
		assert!(lines[0].starts_with("- ")); // First line starts with "- "
		assert!(lines[1].starts_with("  ")); // Second line has 2 spaces of indent
	}

	#[test]
	fn test_simple_array_of_strings() {
		let val = arr(vec![str_val("a"), str_val("b"), str_val("c")]);
		let result = manifest_yaml(&val);

		// go-jsonnet always quotes string values in manifestYamlDoc
		let lines: Vec<&str> = result.lines().collect();
		assert_eq!(lines.len(), 3);
		assert_eq!(lines[0], "- \"a\"");
		assert_eq!(lines[1], "- \"b\"");
		assert_eq!(lines[2], "- \"c\"");
	}

	#[test]
	fn test_simple_object() {
		let mut builder = ObjValueBuilder::new();
		builder.field("key1").value(str_val("value1"));
		builder.field("key2").value(str_val("value2"));
		let val = Val::Obj(builder.build());
		let result = manifest_yaml(&val);

		// go-jsonnet always quotes string values in manifestYamlDoc
		// Object fields are sorted alphabetically by default
		assert!(result.contains("key1: \"value1\""));
		assert!(result.contains("key2: \"value2\""));
	}

	#[test]
	fn test_nested_object() {
		let mut inner_builder = ObjValueBuilder::new();
		inner_builder.field("inner").value(str_val("value"));
		let mut outer_builder = ObjValueBuilder::new();
		outer_builder
			.field("outer")
			.value(Val::Obj(inner_builder.build()));
		let val = Val::Obj(outer_builder.build());
		let result = manifest_yaml(&val);

		// go-jsonnet always quotes string values in manifestYamlDoc
		let lines: Vec<&str> = result.lines().collect();
		assert_eq!(lines.len(), 2);
		assert_eq!(lines[0], "outer:");
		assert_eq!(lines[1], "  inner: \"value\"");
	}

	#[test]
	fn test_object_with_array_value() {
		let mut builder = ObjValueBuilder::new();
		builder
			.field("items")
			.value(arr(vec![str_val("a"), str_val("b")]));
		let val = Val::Obj(builder.build());
		let result = manifest_yaml(&val);

		// go-jsonnet always quotes string values in manifestYamlDoc
		let lines: Vec<&str> = result.lines().collect();
		assert_eq!(lines.len(), 3);
		assert_eq!(lines[0], "items:");
		assert_eq!(lines[1], "- \"a\"");
		assert_eq!(lines[2], "- \"b\"");
	}

	#[test]
	fn test_empty_array() {
		let val = arr(vec![]);
		let result = manifest_yaml(&val);
		assert_eq!(result, "[]");
	}

	#[test]
	fn test_empty_object() {
		let builder = ObjValueBuilder::new();
		let val = Val::Obj(builder.build());
		let result = manifest_yaml(&val);
		assert_eq!(result, "{}");
	}

	// ==========================================================================
	// Tests for multiline strings (block scalars)
	// ==========================================================================

	#[test]
	fn test_multiline_string_with_trailing_newline() {
		let val = str_val("line1\nline2\n");
		let result = manifest_yaml(&val);

		// Should use | indicator for strings ending with newline
		assert!(result.starts_with('|'));
		assert!(result.contains("line1"));
		assert!(result.contains("line2"));
	}

	#[test]
	fn test_multiline_string_without_trailing_newline() {
		let val = str_val("line1\nline2");
		let result = manifest_yaml(&val);

		// Go's manifestYamlDoc uses double-quoted strings with \n escapes for
		// strings that contain newlines but don't end with a newline.
		// Block scalar |- is only used when quote_values is false (CLI mode).
		assert_eq!(result, "\"line1\\nline2\"");
	}

	// ==========================================================================
	// Integration test: complex nested structure
	// ==========================================================================

	#[test]
	fn test_complex_nested_structure() {
		// Simulates a real-world config structure
		let mut item_builder = ObjValueBuilder::new();
		item_builder.field("enabled").value(Val::Bool(true));
		item_builder.field("id").value(str_val("item-1"));
		item_builder
			.field("timestamp")
			.value(str_val("2025-07-03T15:30:00Z"));

		let mut root_builder = ObjValueBuilder::new();
		root_builder.field("count").value(num(10_000_000.0));
		root_builder
			.field("items")
			.value(arr(vec![Val::Obj(item_builder.build())]));
		root_builder.field("name").value(str_val("test-config"));

		let val = Val::Obj(root_builder.build());
		let result = manifest_yaml(&val);

		// Check large number formatting (decimal format, no scientific notation)
		assert!(result.contains("count: 10000000"));

		// Check timestamp is quoted
		assert!(result.contains("\"2025-07-03T15:30:00Z\""));

		// Check object-in-array structure (first field after "- ")
		assert!(result.contains("- enabled: true"));
	}

	#[test]
	fn test_array_in_array() {
		// Nested array: [[a, b], [c, d]]
		let val = arr(vec![
			arr(vec![str_val("a"), str_val("b")]),
			arr(vec![str_val("c"), str_val("d")]),
		]);
		let result = manifest_yaml(&val);

		// Expected format (Go yaml.v3 style):
		// -
		//   - "a"
		//   - "b"
		// -
		//   - "c"
		//   - "d"
		let expected = r#"-
  - "a"
  - "b"
-
  - "c"
  - "d""#;
		assert_eq!(result, expected, "Nested array format mismatch:\n{result}");
	}

	#[test]
	fn test_object_in_array_with_array_value() {
		// Object in array where object has an array field
		// This is the "ranked_choice" pattern from the user's diff
		let mut builder = ObjValueBuilder::new();
		builder.field("ranked_choice").value(arr(vec![
			str_val("k8s_namespace_name"),
			str_val("service_namespace"),
		]));
		let val = arr(vec![Val::Obj(builder.build())]);
		let result = manifest_yaml(&val);

		// Expected format (Go yaml.v3 style):
		// - ranked_choice:
		//   - "k8s_namespace_name"
		//   - "service_namespace"
		let expected = r#"- ranked_choice:
  - "k8s_namespace_name"
  - "service_namespace""#;
		assert_eq!(
			result, expected,
			"Object-in-array with array value format mismatch:\n{result}"
		);
	}

	#[test]
	fn test_deeply_nested_object_array_with_array_value() {
		// Simulates the structure with indent_array_in_object: true
		// asserts:
		//   gated_rules:
		//     - ranked_choice:
		//       - k8s_namespace_name
		//       - service_namespace
		let mut ranked_choice_obj = ObjValueBuilder::new();
		ranked_choice_obj.field("ranked_choice").value(arr(vec![
			str_val("k8s_namespace_name"),
			str_val("service_namespace"),
		]));

		let mut gated_rules_obj = ObjValueBuilder::new();
		gated_rules_obj
			.field("gated_rules")
			.value(arr(vec![Val::Obj(ranked_choice_obj.build())]));

		let mut asserts_obj = ObjValueBuilder::new();
		asserts_obj
			.field("asserts")
			.value(Val::Obj(gated_rules_obj.build()));

		let val = Val::Obj(asserts_obj.build());
		let result = manifest_yaml_indented(&val);

		// With indent_array_in_object: true, arrays are indented under their parent key
		// Arrays inside objects get content_base (alignment after "- ") + arr_element_padding
		let expected = r#"asserts:
  gated_rules:
    - ranked_choice:
        - "k8s_namespace_name"
        - "service_namespace""#;
		assert_eq!(
			result, expected,
			"Deeply nested structure format mismatch:\n{result}"
		);
	}
}
