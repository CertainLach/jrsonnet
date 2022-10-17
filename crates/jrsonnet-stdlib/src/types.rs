use jrsonnet_evaluator::{error::Result, function::builtin, typed::Any, IStr, Val};

#[builtin]
pub fn builtin_type(x: Any) -> Result<IStr> {
	Ok(x.0.value_type().name().into())
}

#[builtin]
pub fn builtin_is_string(x: Any) -> Result<bool> {
	Ok(matches!(x.0, Val::Str(_)))
}
#[builtin]
pub fn builtin_is_number(x: Any) -> Result<bool> {
	Ok(matches!(x.0, Val::Num(_)))
}
#[builtin]
pub fn builtin_is_boolean(x: Any) -> Result<bool> {
	Ok(matches!(x.0, Val::Bool(_)))
}
#[builtin]
pub fn builtin_is_object(x: Any) -> Result<bool> {
	Ok(matches!(x.0, Val::Obj(_)))
}
#[builtin]
pub fn builtin_is_array(x: Any) -> Result<bool> {
	Ok(matches!(x.0, Val::Arr(_)))
}
#[builtin]
pub fn builtin_is_function(x: Any) -> Result<bool> {
	Ok(matches!(x.0, Val::Func(_)))
}
