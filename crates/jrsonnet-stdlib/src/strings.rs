use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result},
	function::builtin,
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
