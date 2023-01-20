use base64::{engine::general_purpose::STANDARD, Engine};
use jrsonnet_evaluator::{
	error::{ErrorKind::RuntimeError, Result},
	function::builtin,
	typed::{Either, Either2},
	IBytes, IStr,
};

#[builtin]
pub fn builtin_encode_utf8(str: IStr) -> IBytes {
	str.cast_bytes()
}

#[builtin]
pub fn builtin_decode_utf8(arr: IBytes) -> Result<IStr> {
	Ok(arr
		.cast_str()
		.ok_or_else(|| RuntimeError("bad utf8".into()))?)
}

#[builtin]
pub fn builtin_base64(input: Either![IStr, IBytes]) -> String {
	use Either2::*;
	match input {
		A(l) => STANDARD.encode(l.as_bytes()),
		B(a) => STANDARD.encode(a.as_slice()),
	}
}

#[builtin]
pub fn builtin_base64_decode_bytes(str: IStr) -> Result<IBytes> {
	Ok(STANDARD
		.decode(str.as_bytes())
		.map_err(|e| RuntimeError(format!("invalid base64: {e}").into()))?
		.as_slice()
		.into())
}

#[builtin]
pub fn builtin_base64_decode(str: IStr) -> Result<String> {
	let bytes = STANDARD
		.decode(str.as_bytes())
		.map_err(|e| RuntimeError(format!("invalid base64: {e}").into()))?;
	Ok(String::from_utf8(bytes).map_err(|_| RuntimeError("bad utf8".into()))?)
}
