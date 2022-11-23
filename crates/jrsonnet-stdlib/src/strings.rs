use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	function::builtin,
	throw,
	typed::{Either2, VecVal, M1},
	val::ArrValue,
	Either, IStr, Val,
};
use jrsonnet_gcmodule::Cc;

#[builtin]
pub const fn builtin_codepoint(str: char) -> Result<u32> {
	Ok(str as u32)
}

#[builtin]
pub fn builtin_substr(str: IStr, from: usize, len: usize) -> Result<String> {
	Ok(str.chars().skip(from).take(len).collect())
}

#[builtin]
pub fn builtin_char(n: u32) -> Result<char> {
	Ok(std::char::from_u32(n).ok_or_else(|| InvalidUnicodeCodepointGot(n))?)
}

#[builtin]
pub fn builtin_str_replace(str: String, from: IStr, to: IStr) -> Result<String> {
	Ok(str.replace(&from as &str, &to as &str))
}

#[builtin]
pub fn builtin_splitlimit(str: IStr, c: IStr, maxsplits: Either![usize, M1]) -> Result<VecVal> {
	use Either2::*;
	Ok(VecVal(Cc::new(match maxsplits {
		A(n) => str
			.splitn(n + 1, &c as &str)
			.map(|s| Val::Str(s.into()))
			.collect(),
		B(_) => str.split(&c as &str).map(|s| Val::Str(s.into())).collect(),
	})))
}

#[builtin]
pub fn builtin_ascii_upper(str: IStr) -> Result<String> {
	Ok(str.to_ascii_uppercase())
}

#[builtin]
pub fn builtin_ascii_lower(str: IStr) -> Result<String> {
	Ok(str.to_ascii_lowercase())
}

#[builtin]
pub fn builtin_find_substr(pat: IStr, str: IStr) -> Result<ArrValue> {
	if pat.is_empty() || str.is_empty() || pat.len() > str.len() {
		return Ok(ArrValue::empty());
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
	Ok(out.into())
}

#[builtin]
pub fn builtin_parse_int(raw: IStr) -> Result<f64> {
	if let Some(raw) = raw.strip_prefix('-') {
		if raw.is_empty() {
			throw!("integer only consists of a minus")
		}

		parse_nat::<10>(raw).map(|value| -value)
	} else {
		if raw.is_empty() {
			throw!("empty integer")
		}

		parse_nat::<10>(raw.as_str())
	}
}

#[builtin]
pub fn builtin_parse_octal(raw: IStr) -> Result<f64> {
	if raw.is_empty() {
		throw!("empty octal integer");
	}

	parse_nat::<8>(raw.as_str())
}

#[builtin]
pub fn builtin_parse_hex(raw: IStr) -> Result<f64> {
	if raw.is_empty() {
		throw!("empty hexadecimal integer");
	}

	parse_nat::<16>(raw.as_str())
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
			throw!("{raw:?} is not a base {BASE} integer",);
		}
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
