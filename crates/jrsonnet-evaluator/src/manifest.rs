use std::{borrow::Cow, cell::Cell, fmt::Write, ptr};

use crate::{bail, in_description_frame, Result, ResultExt, Val};

// Thread-local flag to control float formatting style in std.toString
// When true (default), uses Go's %.17g format (e.g., 0.59999999999999998)
// When false, uses Rust's shortest representation (e.g., 0.6)
thread_local! {
	static USE_GO_STYLE_FLOATS: Cell<bool> = const { Cell::new(true) };
}

/// Set whether to use Go-style float formatting in std.toString
/// - true (default): Use Go's %.17g format (matches go-jsonnet)
/// - false: Use Rust's Display (shortest representation, matches jrsonnet binary)
pub fn set_use_go_style_floats(use_go_style: bool) {
	USE_GO_STYLE_FLOATS.with(|s| s.set(use_go_style));
}

/// Check if Go-style float formatting is enabled
pub(crate) fn should_use_go_style_floats() -> bool {
	USE_GO_STYLE_FLOATS.with(Cell::get)
}

/// Format a float like Go's %.17g format
/// This matches go-jsonnet's unparseNumber function for non-integer values
pub(crate) fn format_float_go_g17(v: f64) -> String {
	// Go's %.17g format:
	// - Uses 17 significant digits maximum
	// - Chooses %e or %f based on exponent (uses %e if exp < -4 or exp >= precision)
	// - Trims trailing zeros and unnecessary decimal point
	// - Uses 2-digit exponent padding (e-05 not e-5)

	// Get the exponent to decide format
	let exp = if v == 0.0 {
		0
	} else {
		v.abs().log10().floor() as i32
	};

	if exp < -4 || exp >= 17 {
		// Use scientific notation like %e
		let formatted = format!("{:.16e}", v);
		// Parse and clean up: "3.1415926535897930e0" -> "3.141592653589793e0"
		if let Some((mantissa, exp_str)) = formatted.split_once('e') {
			let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
			let exp_val: i32 = exp_str.parse().unwrap_or(0);
			if exp_val == 0 {
				mantissa.to_string()
			} else {
				// Go uses 2-digit minimum exponent: e-05 not e-5
				format!("{}e{:+03}", mantissa, exp_val)
			}
		} else {
			formatted
		}
	} else {
		// Use decimal notation like %f but with 17 significant digits
		// Calculate digits after decimal point needed for 17 sig figs
		let digits_after_decimal = (16 - exp).max(0) as usize;
		let formatted = format!("{:.prec$}", v, prec = digits_after_decimal);
		// Trim trailing zeros but keep at least one digit after decimal if there was one
		let trimmed = formatted.trim_end_matches('0');
		let trimmed = trimmed.trim_end_matches('.');
		trimmed.to_string()
	}
}

pub trait ManifestFormat {
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()>;
	fn manifest(&self, val: Val) -> Result<String> {
		let mut out = String::new();
		self.manifest_buf(val, &mut out)?;
		Ok(out)
	}
	/// When outputing to file, is it safe to append a trailing newline (I.e newline won't change
	/// the meaning).
	///
	/// Default implementation returns `true`
	fn file_trailing_newline(&self) -> bool {
		true
	}
}
impl<T> ManifestFormat for Box<T>
where
	T: ManifestFormat + ?Sized,
{
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()> {
		let inner = &**self;
		inner.manifest_buf(val, buf)
	}
	fn file_trailing_newline(&self) -> bool {
		let inner = &**self;
		inner.file_trailing_newline()
	}
}
impl<T> ManifestFormat for &'_ T
where
	T: ManifestFormat + ?Sized,
{
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()> {
		let inner = &**self;
		inner.manifest_buf(val, buf)
	}
	fn file_trailing_newline(&self) -> bool {
		let inner = &**self;
		inner.file_trailing_newline()
	}
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum JsonFormatting {
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
	mtype: JsonFormatting,
	newline: &'s str,
	key_val_sep: &'s str,
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
	#[cfg(feature = "exp-bigint")]
	preserve_bigints: bool,
	debug_truncate_strings: Option<usize>,
}

impl<'s> JsonFormat<'s> {
	// Minifying format
	pub fn minify(#[cfg(feature = "exp-preserve-order")] preserve_order: bool) -> Self {
		Self {
			padding: Cow::Borrowed(""),
			mtype: JsonFormatting::Minify,
			newline: "\n",
			key_val_sep: ":",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
			#[cfg(feature = "exp-bigint")]
			preserve_bigints: false,
			debug_truncate_strings: None,
		}
	}
	/// Same format as std.toString, except does not keeps top-level string as-is
	/// To avoid confusion, the format is private in jrsonnet, use [`ToStringFormat`] instead
	const fn std_to_string_helper() -> Self {
		Self {
			padding: Cow::Borrowed(""),
			mtype: JsonFormatting::ToString,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: false,
			#[cfg(feature = "exp-bigint")]
			preserve_bigints: false,
			debug_truncate_strings: None,
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
			mtype: JsonFormatting::Std,
			newline,
			key_val_sep,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
			#[cfg(feature = "exp-bigint")]
			preserve_bigints: false,
			debug_truncate_strings: None,
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
			mtype: JsonFormatting::Manifest,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
			#[cfg(feature = "exp-bigint")]
			preserve_bigints: false,
			debug_truncate_strings: None,
		}
	}
	// Same format as CLI manifestification
	pub fn debug() -> Self {
		Self {
			padding: Cow::Borrowed("   "),
			mtype: JsonFormatting::Manifest,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: true,
			#[cfg(feature = "exp-bigint")]
			preserve_bigints: true,
			debug_truncate_strings: Some(256),
		}
	}
}
impl Default for JsonFormat<'static> {
	fn default() -> Self {
		Self {
			padding: Cow::Borrowed("    "),
			mtype: JsonFormatting::Manifest,
			newline: "\n",
			key_val_sep: ": ",
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: false,
			#[cfg(feature = "exp-bigint")]
			preserve_bigints: false,
			debug_truncate_strings: None,
		}
	}
}

pub fn manifest_json_ex(val: &Val, options: &JsonFormat<'_>) -> Result<String> {
	let mut out = String::new();
	manifest_json_ex_buf(val, &mut out, &mut String::new(), options)?;
	Ok(out)
}

#[allow(clippy::too_many_lines)]
fn manifest_json_ex_buf(
	val: &Val,
	buf: &mut String,
	cur_padding: &mut String,
	options: &JsonFormat<'_>,
) -> Result<()> {
	use JsonFormatting::*;

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
		Val::Str(s) => {
			let flat = s.clone().into_flat();
			if let Some(truncate) = options.debug_truncate_strings {
				if flat.len() > truncate {
					let (start, end) = flat.split_at(truncate / 2);
					let (_, end) = end.split_at(end.len() - truncate / 2);
					escape_string_json_buf(&format!("{start}..{end}"), buf);
				} else {
					escape_string_json_buf(&flat, buf);
				}
			} else {
				escape_string_json_buf(&flat, buf);
			}
		}
		Val::Num(n) => {
			let v = n.get();
			match mtype {
				// std.toString uses Go's unparseNumber: %.0f for integers, %.17g for floats
				// This is critical for config_hash compatibility (std.md5(std.toString(...)))
				// The go-style formatting can be disabled via set_use_go_style_floats(false)
				// to match upstream jrsonnet binary behavior
				ToString => {
					if v == v.floor() {
						write!(buf, "{:.0}", v).unwrap();
					} else if should_use_go_style_floats() {
						buf.push_str(&format_float_go_g17(v));
					} else {
						// Use Rust's shortest representation (matches jrsonnet binary)
						write!(buf, "{}", v).unwrap();
					}
				}
				// std.manifestJson uses strconv.FormatFloat('f', -1) - shortest decimal
				// Rust's default Display is similar (uses ryu algorithm)
				Manifest | Std | Minify => {
					write!(buf, "{}", v).unwrap();
				}
			}
		}
		#[cfg(feature = "exp-bigint")]
		Val::BigInt(n) => {
			if options.preserve_bigints {
				write!(buf, "{n}").unwrap();
			} else {
				write!(buf, "{:?}", n.to_string()).unwrap();
			}
		}
		Val::Arr(items) => {
			buf.push('[');

			let old_len = cur_padding.len();
			cur_padding.push_str(&options.padding);

			let mut had_items = false;
			for (i, item) in items.iter().enumerate() {
				had_items = true;
				let item = item.with_description(|| format!("elem <{i}> evaluation"))?;

				if i != 0 {
					buf.push(',');
				}
				match mtype {
					Manifest | Std => {
						buf.push_str(options.newline);
						buf.push_str(cur_padding);
					}
					ToString if i != 0 => buf.push(' '),
					Minify | ToString => {}
				};

				in_description_frame(
					|| format!("elem <{i}> manifestification"),
					|| manifest_json_ex_buf(&item, buf, cur_padding, options),
				)?;
			}

			cur_padding.truncate(old_len);

			match mtype {
				Manifest | ToString if !had_items => {
					// Empty array as "[ ]"
					buf.push(' ');
				}
				Manifest => {
					buf.push_str(options.newline);
					buf.push_str(cur_padding);
				}
				Std => {
					if !had_items {
						// Stdlib formats empty array as "[\n\n]"
						buf.push_str(options.newline);
					}
					buf.push_str(options.newline);
					buf.push_str(cur_padding);
				}
				Minify | ToString => {}
			}

			buf.push(']');
		}
		Val::Obj(obj) => {
			obj.run_assertions()?;
			buf.push('{');

			let old_len = cur_padding.len();
			cur_padding.push_str(&options.padding);

			let mut had_fields = false;
			let mut field_count = 0;
			for (key, value) in obj.iter(
				#[cfg(feature = "exp-preserve-order")]
				options.preserve_order,
			) {
				// Skip fields that evaluate to runtime errors (e.g., error statements in unused conditionals)
				// This matches Go Tanka's behavior where these fields are silently ignored during manifest
				// Note: This may hide legitimate configuration errors, but is needed for tk compatibility
				let value = match value.with_description(|| format!("field <{key}> evaluation")) {
					Ok(v) => v,
					Err(e) if matches!(e.error(), crate::error::ErrorKind::RuntimeError(_)) => {
						// Skip this field silently - tk doesn't manifest fields with runtime errors
						continue;
					}
					Err(e) => return Err(e),
				};

				had_fields = true;
				if field_count > 0 {
					buf.push(',');
				}
				field_count += 1;
				match mtype {
					Manifest | Std => {
						buf.push_str(options.newline);
						buf.push_str(cur_padding);
					}
					ToString if field_count > 1 => buf.push(' '),
					Minify | ToString => {}
				}

				escape_string_json_buf(&key, buf);
				buf.push_str(options.key_val_sep);
				in_description_frame(
					|| format!("field <{key}> manifestification"),
					|| manifest_json_ex_buf(&value, buf, cur_padding, options),
				)?;
			}

			cur_padding.truncate(old_len);

			match mtype {
				Manifest | ToString if !had_fields => {
					// Empty object as "{ }"
					buf.push(' ');
				}
				Manifest => {
					buf.push_str(options.newline);
					buf.push_str(cur_padding);
				}
				Std => {
					if !had_fields {
						// Stdlib formats empty object as "{\n\n}"
						buf.push_str(options.newline);
					}
					buf.push_str(options.newline);
					buf.push_str(cur_padding);
				}
				Minify | ToString => {}
			}

			buf.push('}');
		}
		Val::Func(_) => bail!("tried to manifest function"),
	};
	Ok(())
}

impl ManifestFormat for JsonFormat<'_> {
	fn manifest_buf(&self, val: Val, buf: &mut String) -> Result<()> {
		manifest_json_ex_buf(&val, buf, &mut String::new(), self)
	}
}

/// Same as [`JsonFormat`] with pre-set options, but top-level string is serialized as-is,
/// without quoting.
pub struct ToStringFormat;
impl ManifestFormat for ToStringFormat {
	fn manifest_buf(&self, val: Val, out: &mut String) -> Result<()> {
		const JSON_TO_STRING: JsonFormat = JsonFormat::std_to_string_helper();
		if let Some(str) = val.as_str() {
			out.push_str(&str);
			return Ok(());
		}
		#[cfg(feature = "exp-bigint")]
		if let Some(int) = val.as_bigint() {
			out.push_str(&int.to_str_radix(10));
			return Ok(());
		}
		JSON_TO_STRING.manifest_buf(val, out)
	}
	fn file_trailing_newline(&self) -> bool {
		false
	}
}
pub struct StringFormat;
impl ManifestFormat for StringFormat {
	fn manifest_buf(&self, val: Val, out: &mut String) -> Result<()> {
		let Val::Str(s) = val else {
			bail!(
				"output should be string for string manifest format, got {}",
				val.value_type()
			)
		};
		write!(out, "{s}").unwrap();
		Ok(())
	}
	fn file_trailing_newline(&self) -> bool {
		false
	}
}

pub struct YamlStreamFormat<I> {
	inner: I,
	c_document_end: bool,
	end_newline: bool,
	/// When true, empty arrays produce "\n" (jrsonnet behavior)
	/// When false, empty arrays produce "---\n\n" (go-jsonnet behavior)
	jrsonnet_empty: bool,
}
impl<I> YamlStreamFormat<I> {
	pub fn std_yaml_stream(inner: I, c_document_end: bool, jrsonnet_empty: bool) -> Self {
		Self {
			inner,
			c_document_end,
			// Stdlib format always inserts useless newline at the end
			end_newline: true,
			jrsonnet_empty,
		}
	}
	pub fn cli(inner: I) -> Self {
		Self {
			inner,
			c_document_end: true,
			end_newline: false,
			jrsonnet_empty: false,
		}
	}
}
impl<I: ManifestFormat> ManifestFormat for YamlStreamFormat<I> {
	fn manifest_buf(&self, val: Val, out: &mut String) -> Result<()> {
		let Val::Arr(arr) = val else {
			bail!(
				"output should be array for yaml stream format, got {}",
				val.value_type()
			)
		};
		if arr.is_empty() {
			if self.jrsonnet_empty {
				// jrsonnet binary outputs "\n" for empty arrays (just a newline)
				// or "...\n" when c_document_end is true
				// (no document marker for empty arrays)
			} else {
				// go-jsonnet outputs "---\n\n" for empty arrays (document marker + empty document)
				out.push_str("---\n\n");
			}
		} else {
			for (i, v) in arr.iter().enumerate() {
				let v = v.with_description(|| format!("elem <{i}> evaluation"))?;
				out.push_str("---\n");
				in_description_frame(
					|| format!("elem <{i}> manifestification"),
					|| self.inner.manifest_buf(v, out),
				)?;
				out.push('\n');
			}
		}
		if self.c_document_end {
			out.push_str("...");
		}
		// For jrsonnet empty mode: always add trailing newline
		// For go-jsonnet mode: only add trailing newline if c_document_end is true
		if self.jrsonnet_empty || self.c_document_end {
			if self.end_newline {
				out.push('\n');
			}
		}
		Ok(())
	}
}

pub fn escape_string_json(s: &str) -> String {
	let mut buf = String::new();
	escape_string_json_buf(s, &mut buf);
	buf
}

// Json string encoding was borrowed from https://github.com/serde-rs/json

const BB: u8 = b'b'; // \x08
const TT: u8 = b't'; // \x09
const NN: u8 = b'n'; // \x0A
const FF: u8 = b'f'; // \x0C
const RR: u8 = b'r'; // \x0D
const QU: u8 = b'"'; // \x22
const BS: u8 = b'\\'; // \x5C
const UU: u8 = b'u'; // \x00...\x1F except the ones above
const __: u8 = 0;

// Lookup table of escape sequences. A value of b'x' at index i means that byte
// i is escaped as "\x" in JSON. A value of 0 means that byte i is not escaped.
static ESCAPE: [u8; 256] = [
	//   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
	UU, UU, UU, UU, UU, UU, UU, UU, BB, TT, NN, UU, FF, RR, UU, UU, // 0
	UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, // 1
	__, __, QU, __, __, __, __, __, __, __, __, __, __, __, __, __, // 2
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 3
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 4
	__, __, __, __, __, __, __, __, __, __, __, __, BS, __, __, __, // 5
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 6
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 7
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 8
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 9
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // A
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // B
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // C
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // D
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // E
	__, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // F
];

pub fn escape_string_json_buf(value: &str, buf: &mut String) {
	// Safety: we only write correct utf-8 in this function
	let buf: &mut Vec<u8> = unsafe { &mut *ptr::from_mut(buf).cast::<Vec<u8>>() };
	let bytes = value.as_bytes();

	// Perfect for ascii strings, removes any reallocations
	buf.reserve(value.len() + 2);

	buf.push(b'"');

	let mut start = 0;

	for (i, &byte) in bytes.iter().enumerate() {
		let escape = ESCAPE[byte as usize];
		if escape == __ {
			continue;
		}

		if start < i {
			buf.extend_from_slice(&bytes[start..i]);
		}
		start = i + 1;

		match escape {
			self::BB | self::TT | self::NN | self::FF | self::RR | self::QU | self::BS => {
				buf.extend_from_slice(&[b'\\', escape]);
			}
			self::UU => {
				static HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";
				let bytes = &[
					b'\\',
					b'u',
					b'0',
					b'0',
					HEX_DIGITS[(byte >> 4) as usize],
					HEX_DIGITS[(byte & 0xF) as usize],
				];
				buf.extend_from_slice(bytes);
			}
			_ => unreachable!(),
		}
	}

	if start == bytes.len() {
		buf.push(b'"');
		return;
	}

	buf.extend_from_slice(&bytes[start..]);
	buf.push(b'"');
}
