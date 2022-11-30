use jrsonnet_evaluator::{error::Result, function::builtin, typed::Any, IStr, Val};

#[builtin]
pub fn builtin_type(v: Any) -> Result<IStr> {
	Ok(v.0.value_type().name().into())
}

#[builtin]
pub fn builtin_is_string(v: Any) -> Result<bool> {
	Ok(matches!(v.0, Val::Str(_)))
}
#[builtin]
pub fn builtin_is_number(v: Any) -> Result<bool> {
	Ok(matches!(v.0, Val::Num(_)))
}
#[builtin]
pub fn builtin_is_boolean(v: Any) -> Result<bool> {
	Ok(matches!(v.0, Val::Bool(_)))
}
#[builtin]
pub fn builtin_is_object(v: Any) -> Result<bool> {
	Ok(matches!(v.0, Val::Obj(_)))
}
#[builtin]
pub fn builtin_is_array(v: Any) -> Result<bool> {
	Ok(matches!(v.0, Val::Arr(_)))
}
#[builtin]
pub fn builtin_is_function(v: Any) -> Result<bool> {
	Ok(matches!(v.0, Val::Func(_)))
}
