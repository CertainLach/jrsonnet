use jrsonnet_evaluator::{error::Result, function::builtin, IStr};

#[builtin]
pub fn builtin_md5(str: IStr) -> Result<String> {
	Ok(format!("{:x}", md5::compute(str.as_bytes())))
}

#[cfg(feature = "exp-more-hashes")]
#[builtin]
pub fn builtin_sha256(str: IStr) -> Result<String> {
	use sha2::digest::Digest;
	Ok(format!("{:?}", sha2::Sha256::digest(str.as_bytes())))
}
