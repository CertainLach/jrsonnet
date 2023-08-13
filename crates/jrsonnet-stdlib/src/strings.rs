use jrsonnet_evaluator::{
	bail,
	error::{ErrorKind::*, Result},
	function::builtin,
	typed::{Either2, M1},
	val::{ArrValue, StrValue},
	Either, IStr, Val,
};

#[builtin]
pub const fn builtin_codepoint(str: char) -> u32 {
	str as u32
}

#[builtin]
pub fn builtin_substr(str: IStr, from: usize, len: usize) -> String {
	str.chars().skip(from).take(len).collect()
}

#[builtin]
pub fn builtin_char(n: u32) -> Result<char> {
	Ok(std::char::from_u32(n).ok_or_else(|| InvalidUnicodeCodepointGot(n))?)
}

#[builtin]
pub fn builtin_str_replace(str: String, from: IStr, to: IStr) -> String {
	str.replace(&from as &str, &to as &str)
}

#[builtin]
pub fn builtin_is_empty(str: String) -> bool {
	str.is_empty()
}

#[builtin]
pub fn builtin_equals_ignore_case(str1: String, str2: String) -> bool {
	str1.to_ascii_lowercase() == str2.to_ascii_lowercase()
}

#[builtin]
pub fn builtin_splitlimit(str: IStr, c: IStr, maxsplits: Either![usize, M1]) -> ArrValue {
	use Either2::*;
	match maxsplits {
		A(n) => str
			.splitn(n + 1, &c as &str)
			.map(|s| Val::Str(StrValue::Flat(s.into())))
			.collect(),
		B(_) => str
			.split(&c as &str)
			.map(|s| Val::Str(StrValue::Flat(s.into())))
			.collect(),
	}
}

#[builtin]
pub fn builtin_ascii_upper(str: IStr) -> String {
	str.to_ascii_uppercase()
}

#[builtin]
pub fn builtin_ascii_lower(str: IStr) -> String {
	str.to_ascii_lowercase()
}

#[builtin]
pub fn builtin_find_substr(pat: IStr, str: IStr) -> ArrValue {
	if pat.is_empty() || str.is_empty() || pat.len() > str.len() {
		return ArrValue::empty();
	}

	let str = str.as_str();
	let pat = pat.as_bytes();
	let strb = str.as_bytes();

	let max_pos = str.len() - pat.len();

	let mut out: Vec<Val> = Vec::new();
	for (ch_idx, (i, _)) in str
		.char_indices()
		.take_while(|(i, _)| i <= &max_pos)
		.enumerate()
	{
		if &strb[i..i + pat.len()] == pat {
			out.push(Val::Num(ch_idx as f64))
		}
	}
	out.into()
}

#[builtin]
pub fn builtin_parse_int(str: IStr) -> Result<f64> {
	if let Some(raw) = str.strip_prefix('-') {
		if raw.is_empty() {
			bail!("integer only consists of a minus")
		}

		parse_nat::<10>(raw).map(|value| -value)
	} else {
		if str.is_empty() {
			bail!("empty integer")
		}

		parse_nat::<10>(str.as_str())
	}
}

#[builtin]
pub fn builtin_parse_octal(str: IStr) -> Result<f64> {
	if str.is_empty() {
		bail!("empty octal integer");
	}

	parse_nat::<8>(str.as_str())
}

#[builtin]
pub fn builtin_parse_hex(str: IStr) -> Result<f64> {
	if str.is_empty() {
		bail!("empty hexadecimal integer");
	}

	parse_nat::<16>(str.as_str())
}

fn parse_nat<const BASE: u32>(raw: &str) -> Result<f64> {
	debug_assert!(
		1 <= BASE && BASE <= 16,
		"integer base should be between 1 and 16"
	);

	const ZERO_CODE: u32 = '0' as u32;
	const UPPER_A_CODE: u32 = 'A' as u32;
	const LOWER_A_CODE: u32 = 'a' as u32;

	#[inline]
	fn checked_sub_if(condition: bool, lhs: u32, rhs: u32) -> Option<u32> {
		if condition {
			lhs.checked_sub(rhs)
		} else {
			None
		}
	}

	let base = BASE as f64;

	raw.chars().try_fold(0f64, |aggregate, digit| {
		let digit = digit as u32;
		let digit = if let Some(digit) = checked_sub_if(BASE > 10, digit, LOWER_A_CODE) {
			digit + 10
		} else if let Some(digit) = checked_sub_if(BASE > 10, digit, UPPER_A_CODE) {
			digit + 10
		} else {
			digit.checked_sub(ZERO_CODE).unwrap_or(BASE)
		};

		if digit < BASE {
			Ok(base * aggregate + digit as f64)
		} else {
			bail!("{raw:?} is not a base {BASE} integer");
		}
	})
}

#[cfg(feature = "exp-bigint")]
#[builtin]
pub fn builtin_bigint(v: Either![f64, IStr]) -> Result<Val> {
	use jrsonnet_evaluator::runtime_error;
	use Either2::*;
	Ok(match v {
		A(a) => Val::BigInt(Box::new((a as i64).into())),
		B(b) => Val::BigInt(Box::new(
			b.as_str()
				.parse()
				.map_err(|e| runtime_error!("bad bigint: {e}"))?,
		)),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_nat_base_8() {
		assert_eq!(parse_nat::<8>("0").unwrap(), 0.);
		assert_eq!(parse_nat::<8>("5").unwrap(), 5.);
		assert_eq!(parse_nat::<8>("32").unwrap(), 0o32 as f64);
		assert_eq!(parse_nat::<8>("761").unwrap(), 0o761 as f64);
	}

	#[test]
	fn parse_nat_base_10() {
		assert_eq!(parse_nat::<10>("0").unwrap(), 0.);
		assert_eq!(parse_nat::<10>("3").unwrap(), 3.);
		assert_eq!(parse_nat::<10>("27").unwrap(), 27.);
		assert_eq!(parse_nat::<10>("123").unwrap(), 123.);
	}

	#[test]
	fn parse_nat_base_16() {
		assert_eq!(parse_nat::<16>("0").unwrap(), 0.);
		assert_eq!(parse_nat::<16>("A").unwrap(), 10.);
		assert_eq!(parse_nat::<16>("a9").unwrap(), 0xA9 as f64);
		assert_eq!(parse_nat::<16>("BbC").unwrap(), 0xBBC as f64);
	}
}
