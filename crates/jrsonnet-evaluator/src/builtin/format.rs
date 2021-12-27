//! faster std.format impl
#![allow(clippy::too_many_arguments)]

use crate::{error::Error::*, throw, LocError, ObjValue, Result, Val};
use gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_types::ValType;
use std::convert::TryFrom;
use thiserror::Error;

#[derive(Debug, Clone, Error, Trace)]
pub enum FormatError {
	#[error("truncated format code")]
	TruncatedFormatCode,
	#[error("unrecognized conversion type: {0}")]
	UnrecognizedConversionType(char),

	#[error("not enough values")]
	NotEnoughValues,

	#[error("cannot use * width with object")]
	CannotUseStarWidthWithObject,
	#[error("mapping keys required")]
	MappingKeysRequired,
	#[error("no such format field: {0}")]
	NoSuchFormatField(IStr),
}

impl From<FormatError> for LocError {
	fn from(e: FormatError) -> Self {
		Self::new(Format(e))
	}
}

use FormatError::*;

type ParseResult<'t, T> = std::result::Result<(T, &'t str), FormatError>;

pub fn try_parse_mapping_key(str: &str) -> ParseResult<&str> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}
	let bytes = str.as_bytes();
	if bytes[0] == b'(' {
		let mut i = 1;
		while i < bytes.len() {
			if bytes[i] == b')' {
				return Ok((&str[1..i as usize], &str[i as usize + 1..]));
			}
			i += 1;
		}
		Err(TruncatedFormatCode)
	} else {
		Ok(("", str))
	}
}

#[cfg(test)]
pub mod tests_key {
	use super::*;

	#[test]
	fn parse_key() {
		assert_eq!(
			try_parse_mapping_key("(hello ) world").unwrap(),
			("hello ", " world")
		);
		assert_eq!(try_parse_mapping_key("() world").unwrap(), ("", " world"));
		assert_eq!(try_parse_mapping_key(" world").unwrap(), ("", " world"));
		assert_eq!(
			try_parse_mapping_key(" () world").unwrap(),
			("", " () world")
		);
	}

	#[test]
	#[should_panic]
	fn parse_key_missing_start() {
		try_parse_mapping_key("").unwrap();
	}

	#[test]
	#[should_panic]
	fn parse_key_missing_end() {
		try_parse_mapping_key("(   ").unwrap();
	}
}

#[derive(Default, Debug)]
pub struct CFlags {
	pub alt: bool,
	pub zero: bool,
	pub left: bool,
	pub blank: bool,
	pub sign: bool,
}

pub fn try_parse_cflags(str: &str) -> ParseResult<CFlags> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}
	let bytes = str.as_bytes();
	let mut i = 0;
	let mut out = CFlags::default();
	loop {
		if bytes.len() == i {
			return Err(TruncatedFormatCode);
		}
		match bytes[i] {
			b'#' => out.alt = true,
			b'0' => out.zero = true,
			b'-' => out.left = true,
			b' ' => out.blank = true,
			b'+' => out.sign = true,
			_ => break,
		}
		i += 1;
	}
	Ok((out, &str[i..]))
}

#[derive(Debug, PartialEq)]
pub enum Width {
	Star,
	Fixed(usize),
}
pub fn try_parse_field_width(str: &str) -> ParseResult<Width> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}
	let bytes = str.as_bytes();
	if bytes[0] == b'*' {
		return Ok((Width::Star, &str[1..]));
	}
	let mut out: usize = 0;
	let mut digits = 0;
	while let Some(digit) = (bytes[digits] as char).to_digit(10) {
		out *= 10;
		out += digit as usize;
		digits += 1;
		if digits == bytes.len() {
			return Err(TruncatedFormatCode);
		}
	}
	Ok((Width::Fixed(out), &str[digits..]))
}

pub fn try_parse_precision(str: &str) -> ParseResult<Option<Width>> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}
	let bytes = str.as_bytes();
	if bytes[0] == b'.' {
		try_parse_field_width(&str[1..]).map(|(r, s)| (Some(r), s))
	} else {
		Ok((None, str))
	}
}

// Only skips
pub fn try_parse_length_modifier(str: &str) -> ParseResult<()> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}
	let bytes = str.as_bytes();
	let mut idx = 0;
	while bytes[idx] == b'h' || bytes[idx] == b'l' || bytes[idx] == b'L' {
		idx += 1;
		if bytes.len() == idx {
			return Err(TruncatedFormatCode);
		}
	}
	Ok(((), &str[idx..]))
}

#[derive(Debug, PartialEq)]
pub enum ConvTypeV {
	Decimal,
	Octal,
	Hexadecimal,
	Scientific,
	Float,
	Shorter,
	Char,
	String,
	Percent,
}
pub struct ConvType {
	v: ConvTypeV,
	caps: bool,
}

pub fn parse_conversion_type(str: &str) -> ParseResult<ConvType> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}

	let code = str.as_bytes()[0];
	let v: (ConvTypeV, bool) = match code {
		b'd' | b'i' | b'u' => (ConvTypeV::Decimal, false),
		b'o' => (ConvTypeV::Octal, false),
		b'x' => (ConvTypeV::Hexadecimal, false),
		b'X' => (ConvTypeV::Hexadecimal, true),
		b'e' => (ConvTypeV::Scientific, false),
		b'E' => (ConvTypeV::Scientific, true),
		b'f' => (ConvTypeV::Float, false),
		b'F' => (ConvTypeV::Float, true),
		b'g' => (ConvTypeV::Shorter, false),
		b'G' => (ConvTypeV::Shorter, true),
		b'c' => (ConvTypeV::Char, false),
		b's' => (ConvTypeV::String, false),
		b'%' => (ConvTypeV::Percent, false),
		c => return Err(UnrecognizedConversionType(c as char)),
	};

	Ok((ConvType { v: v.0, caps: v.1 }, &str[1..]))
}

#[derive(Debug)]
pub struct Code<'s> {
	mkey: &'s str,
	cflags: CFlags,
	width: Width,
	precision: Option<Width>,
	convtype: ConvTypeV,
	caps: bool,
}
pub fn parse_code(str: &str) -> ParseResult<Code> {
	if str.is_empty() {
		return Err(TruncatedFormatCode);
	}
	let (mkey, str) = try_parse_mapping_key(str)?;
	let (cflags, str) = try_parse_cflags(str)?;
	let (width, str) = try_parse_field_width(str)?;
	let (precision, str) = try_parse_precision(str)?;
	let (_, str) = try_parse_length_modifier(str)?;
	let (convtype, str) = parse_conversion_type(str)?;

	Ok((
		Code {
			mkey,
			cflags,
			width,
			precision,
			convtype: convtype.v,
			caps: convtype.caps,
		},
		str,
	))
}

#[derive(Debug)]
pub enum Element<'s> {
	String(&'s str),
	Code(Code<'s>),
}
pub fn parse_codes(mut str: &str) -> Result<Vec<Element>> {
	let mut bytes = str.as_bytes();
	let mut out = vec![];
	let mut offset = 0;

	loop {
		while offset != bytes.len() && bytes[offset] != b'%' {
			offset += 1;
		}
		if offset != 0 {
			out.push(Element::String(&str[0..offset]));
		}
		if offset == bytes.len() {
			return Ok(out);
		}
		str = &str[offset + 1..];
		let (code, nstr) = parse_code(str)?;
		str = nstr;
		bytes = str.as_bytes();
		offset = 0;

		out.push(Element::Code(code))
	}
}

const NUMBERS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

#[inline]
pub fn render_integer(
	out: &mut String,
	iv: i64,
	padding: usize,
	precision: usize,
	blank: bool,
	sign: bool,
	radix: i64,
	prefix: &str,
	caps: bool,
) {
	// Digit char indexes in reverse order, i.e
	// for radix = 16 and n = 12f: [15, 2, 1]
	let digits = if iv == 0 {
		vec![0u8]
	} else {
		let mut v = iv.abs();
		let mut nums = Vec::with_capacity(1);
		while v > 0 {
			nums.push((v % radix) as u8);
			v /= radix;
		}
		nums
	};
	let neg = iv < 0;
	let zp = padding.saturating_sub(if neg || blank || sign { 1 } else { 0 });
	let zp2 = zp
		.max(precision)
		.saturating_sub(prefix.len() + digits.len());

	if neg {
		out.push('-')
	} else if sign {
		out.push('+');
	} else if blank {
		out.push(' ');
	}

	out.reserve(zp2);
	for _ in 0..zp2 {
		out.push('0');
	}
	out.push_str(prefix);

	for digit in digits.into_iter().rev() {
		let ch = NUMBERS[digit as usize] as char;
		out.push(if caps { ch.to_ascii_uppercase() } else { ch });
	}
}

pub fn render_decimal(
	out: &mut String,
	iv: i64,
	padding: usize,
	precision: usize,
	blank: bool,
	sign: bool,
) {
	render_integer(out, iv, padding, precision, blank, sign, 10, "", false)
}
pub fn render_octal(
	out: &mut String,
	iv: i64,
	padding: usize,
	precision: usize,
	alt: bool,
	blank: bool,
	sign: bool,
) {
	render_integer(
		out,
		iv,
		padding,
		precision,
		blank,
		sign,
		8,
		if alt && iv != 0 { "0" } else { "" },
		false,
	)
}
pub fn render_hexadecimal(
	out: &mut String,
	iv: i64,
	padding: usize,
	precision: usize,
	alt: bool,
	blank: bool,
	sign: bool,
	caps: bool,
) {
	render_integer(
		out,
		iv,
		padding,
		precision,
		blank,
		sign,
		16,
		match (alt, caps) {
			(true, true) => "0X",
			(true, false) => "0x",
			(false, _) => "",
		},
		caps,
	)
}

pub fn render_float(
	out: &mut String,
	n: f64,
	mut padding: usize,
	precision: usize,
	blank: bool,
	sign: bool,
	ensure_pt: bool,
	trailing: bool,
) {
	let dot_size = if precision == 0 && !ensure_pt { 0 } else { 1 };
	padding = padding.saturating_sub(dot_size + precision);
	render_decimal(out, n.floor() as i64, padding, 0, blank, sign);
	if precision == 0 {
		if ensure_pt {
			out.push('.');
		}
		return;
	}
	let frac = n
		.fract()
		.mul_add(10.0_f64.powf(precision as f64), 0.5)
		.floor();
	if trailing || frac > 0.0 {
		out.push('.');
		let mut frac_str = String::new();
		render_decimal(&mut frac_str, frac as i64, precision, 0, false, false);
		let mut trim = frac_str.len();
		if !trailing {
			for b in frac_str.as_bytes().iter().rev() {
				if *b == b'0' {
					trim -= 1;
				}
			}
		}
		out.push_str(&frac_str[..trim]);
	} else if ensure_pt {
		out.push('.');
	}
}

pub fn render_float_sci(
	out: &mut String,
	n: f64,
	mut padding: usize,
	precision: usize,
	blank: bool,
	sign: bool,
	ensure_pt: bool,
	trailing: bool,
	caps: bool,
) {
	let exponent = n.log10().floor();
	let mantissa = if exponent as i16 == -324 {
		n * 10.0 / 10.0_f64.powf(exponent + 1.0)
	} else {
		n / 10.0_f64.powf(exponent)
	};
	let mut exponent_str = String::new();
	render_decimal(&mut exponent_str, exponent as i64, 3, 0, false, true);

	// +1 for e
	padding = padding.saturating_sub(exponent_str.len() + 1);

	render_float(
		out, mantissa, padding, precision, blank, sign, ensure_pt, trailing,
	);
	out.push(if caps { 'E' } else { 'e' });
	out.push_str(&exponent_str);
}

pub fn format_code(
	out: &mut String,
	value: &Val,
	code: &Code,
	width: usize,
	precision: Option<usize>,
) -> Result<()> {
	let clfags = &code.cflags;
	let (fpprec, iprec) = match precision {
		Some(v) => (v, v),
		None => (6, 0),
	};
	let padding = if clfags.zero && !clfags.left {
		width
	} else {
		0
	};

	// TODO: If left padded, can optimize by writing directly to out
	let mut tmp_out = String::new();

	match code.convtype {
		ConvTypeV::String => tmp_out.push_str(&value.clone().to_string()?),
		ConvTypeV::Decimal => {
			let value = f64::try_from(value.clone())?;
			render_decimal(
				&mut tmp_out,
				value as i64,
				padding,
				iprec,
				clfags.blank,
				clfags.sign,
			);
		}
		ConvTypeV::Octal => {
			let value = f64::try_from(value.clone())?;
			render_octal(
				&mut tmp_out,
				value as i64,
				padding,
				iprec,
				clfags.alt,
				clfags.blank,
				clfags.sign,
			);
		}
		ConvTypeV::Hexadecimal => {
			let value = f64::try_from(value.clone())?;
			render_hexadecimal(
				&mut tmp_out,
				value as i64,
				padding,
				iprec,
				clfags.alt,
				clfags.blank,
				clfags.sign,
				code.caps,
			);
		}
		ConvTypeV::Scientific => {
			let value = f64::try_from(value.clone())?;
			render_float_sci(
				&mut tmp_out,
				value,
				padding,
				fpprec,
				clfags.blank,
				clfags.sign,
				clfags.alt,
				true,
				code.caps,
			);
		}
		ConvTypeV::Float => {
			let value = f64::try_from(value.clone())?;
			render_float(
				&mut tmp_out,
				value,
				padding,
				fpprec,
				clfags.blank,
				clfags.sign,
				clfags.alt,
				true,
			);
		}
		ConvTypeV::Shorter => {
			let value = f64::try_from(value.clone())?;
			let exponent = value.log10().floor();
			if exponent < -4.0 || exponent >= fpprec as f64 {
				render_float_sci(
					&mut tmp_out,
					value,
					padding,
					fpprec - 1,
					clfags.blank,
					clfags.sign,
					clfags.alt,
					clfags.alt,
					code.caps,
				);
			} else {
				let digits_before_pt = 1.max(exponent as usize + 1);
				render_float(
					&mut tmp_out,
					value,
					padding,
					fpprec - digits_before_pt,
					clfags.blank,
					clfags.sign,
					clfags.alt,
					clfags.alt,
				);
			}
		}
		ConvTypeV::Char => match value.clone() {
			Val::Num(n) => tmp_out
				.push(std::char::from_u32(n as u32).ok_or(InvalidUnicodeCodepointGot(n as u32))?),
			Val::Str(s) => {
				if s.chars().count() != 1 {
					throw!(RuntimeError(
						format!("%c expected 1 char string, got {}", s.chars().count()).into(),
					));
				}
				tmp_out.push_str(&s);
			}
			_ => {
				throw!(TypeMismatch(
					"%c requires number/string",
					vec![ValType::Num, ValType::Str],
					value.value_type(),
				));
			}
		},
		ConvTypeV::Percent => tmp_out.push('%'),
	};

	let padding = width.saturating_sub(tmp_out.len());

	if !clfags.left {
		for _ in 0..padding {
			out.push(' ');
		}
	}
	out.push_str(&tmp_out);
	if clfags.left {
		for _ in 0..padding {
			out.push(' ');
		}
	}

	Ok(())
}

pub fn format_arr(str: &str, mut values: &[Val]) -> Result<String> {
	let codes = parse_codes(str)?;
	let mut out = String::new();

	for code in codes {
		match code {
			Element::String(s) => {
				out.push_str(s);
			}
			Element::Code(c) => {
				let width = match c.width {
					Width::Star => {
						if values.is_empty() {
							throw!(NotEnoughValues);
						}
						let value = &values[0];
						values = &values[1..];
						usize::try_from(value.clone())?
					}
					Width::Fixed(n) => n,
				};
				let precision = match c.precision {
					Some(Width::Star) => {
						if values.is_empty() {
							throw!(NotEnoughValues);
						}
						let value = &values[0];
						values = &values[1..];
						Some(usize::try_from(value.clone())?)
					}
					Some(Width::Fixed(n)) => Some(n),
					None => None,
				};

				// %% should not consume a value
				let value = if c.convtype == ConvTypeV::Percent {
					&Val::Null
				} else {
					if values.is_empty() {
						throw!(NotEnoughValues);
					}
					let value = &values[0];
					values = &values[1..];
					value
				};

				format_code(&mut out, value, &c, width, precision)?;
			}
		}
	}

	Ok(out)
}

pub fn format_obj(str: &str, values: &ObjValue) -> Result<String> {
	let codes = parse_codes(str)?;
	let mut out = String::new();

	for code in codes {
		match code {
			Element::String(s) => {
				out.push_str(s);
			}
			Element::Code(c) => {
				// TODO: Operate on ref
				let f: IStr = c.mkey.into();
				let width = match c.width {
					Width::Star => {
						throw!(CannotUseStarWidthWithObject);
					}
					Width::Fixed(n) => n,
				};
				let precision = match c.precision {
					Some(Width::Star) => {
						throw!(CannotUseStarWidthWithObject);
					}
					Some(Width::Fixed(n)) => Some(n),
					None => None,
				};

				let value = if c.convtype == ConvTypeV::Percent {
					Val::Null
				} else {
					if f.is_empty() {
						throw!(MappingKeysRequired);
					}
					if let Some(v) = values.get(f.clone())? {
						v
					} else {
						throw!(NoSuchFormatField(f));
					}
				};

				format_code(&mut out, &value, &c, width, precision)?;
			}
		}
	}

	Ok(out)
}

#[cfg(test)]
pub mod test_format {
	use super::*;

	#[test]
	fn parse() {
		assert_eq!(
			parse_codes(
				"How much error budget is left looking at our %.3f%% availability gurantees?"
			)
			.unwrap()
			.len(),
			4
		);
	}

	#[test]
	fn octals() {
		assert_eq!(format_arr("%#o", &[Val::Num(8.0)]).unwrap(), "010");
		assert_eq!(format_arr("%#4o", &[Val::Num(8.0)]).unwrap(), " 010");
		assert_eq!(format_arr("%4o", &[Val::Num(8.0)]).unwrap(), "  10");
		assert_eq!(format_arr("%04o", &[Val::Num(8.0)]).unwrap(), "0010");
		assert_eq!(format_arr("%+4o", &[Val::Num(8.0)]).unwrap(), " +10");
		assert_eq!(format_arr("%+04o", &[Val::Num(8.0)]).unwrap(), "+010");
		assert_eq!(format_arr("%-4o", &[Val::Num(8.0)]).unwrap(), "10  ");
		assert_eq!(format_arr("%+-4o", &[Val::Num(8.0)]).unwrap(), "+10 ");
		assert_eq!(format_arr("%+-04o", &[Val::Num(8.0)]).unwrap(), "+10 ");
	}

	#[test]
	fn percent_doesnt_consumes_values() {
		assert_eq!(
			format_arr(
				"How much error budget is left looking at our %.3f%% availability gurantees?",
				&[Val::Num(4.0)]
			)
			.unwrap(),
			"How much error budget is left looking at our 4.000% availability gurantees?"
		);
	}
}
