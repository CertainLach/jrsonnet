use jrsonnet_evaluator::{function::builtin, IStr};

#[builtin]
pub fn builtin_md5(s: IStr) -> String {
	format!("{:x}", md5::compute(s.as_bytes()))
}

#[builtin]
pub fn builtin_sha256(str: IStr) -> String {
	use sha2::digest::Digest;
	format!("{:x}", sha2::Sha256::digest(str.as_bytes()))
}

#[builtin]
pub fn builtin_sha512(str: IStr) -> String {
	use sha2::digest::Digest;
	format!("{:x}", sha2::Sha512::digest(str.as_bytes()))
}

#[builtin]
pub fn builtin_sha1(str: IStr) -> String {
	use sha1::digest::Digest;
	format!("{:x}", sha1::Sha1::digest(str.as_bytes()))
}

#[builtin]
pub fn builtin_sha3(str: IStr) -> String {
	use sha3::digest::Digest;
	format!("{:x}", sha3::Sha3_512::digest(str.as_bytes()))
}
