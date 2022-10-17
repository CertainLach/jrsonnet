use jrsonnet_evaluator::{
	error::{Error::RuntimeError, Result},
	function::builtin,
	typed::{Either, Either2},
	IBytes, IStr,
};

#[builtin]
pub fn builtin_encode_utf8(str: IStr) -> Result<IBytes> {
	Ok(str.cast_bytes())
}

#[builtin]
pub fn builtin_decode_utf8(arr: IBytes) -> Result<IStr> {
	Ok(arr
		.cast_str()
		.ok_or_else(|| RuntimeError("bad utf8".into()))?)
}

#[builtin]
pub fn builtin_base64(input: Either![IBytes, IStr]) -> Result<String> {
	use Either2::*;
	Ok(match input {
		A(a) => base64::encode(a.as_slice()),
		B(l) => base64::encode(l.bytes().collect::<Vec<_>>()),
	})
}

#[builtin]
pub fn builtin_base64_decode_bytes(input: IStr) -> Result<IBytes> {
	Ok(base64::decode(input.as_bytes())
		.map_err(|_| RuntimeError("bad base64".into()))?
		.as_slice()
		.into())
}

#[builtin]
pub fn builtin_base64_decode(input: IStr) -> Result<String> {
	let bytes = base64::decode(input.as_bytes()).map_err(|_| RuntimeError("bad base64".into()))?;
	Ok(String::from_utf8(bytes).map_err(|_| RuntimeError("bad utf8".into()))?)
}
