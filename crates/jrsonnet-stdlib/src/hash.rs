use jrsonnet_evaluator::{error::Result, function::builtin, IStr};

#[builtin]
pub fn builtin_md5(str: IStr) -> Result<String> {
	Ok(format!("{:x}", md5::compute(&str.as_bytes())))
}
