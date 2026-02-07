use jrsonnet_evaluator::{function::builtin, IStr, Val};

#[builtin]
pub fn builtin_type(x: Val) -> IStr {
	x.value_type().name().into()
}

#[builtin]
pub fn builtin_is_string(v: Val) -> bool {
	matches!(v, Val::Str(_))
}
#[builtin]
pub fn builtin_is_number(v: Val) -> bool {
	matches!(v, Val::Num(_))
}
#[builtin]
pub fn builtin_is_boolean(v: Val) -> bool {
	matches!(v, Val::Bool(_))
}
#[builtin]
pub fn builtin_is_object(v: Val) -> bool {
	matches!(v, Val::Obj(_))
}
#[builtin]
pub fn builtin_is_array(v: Val) -> bool {
	matches!(v, Val::Arr(_))
}
#[builtin]
pub fn builtin_is_function(v: Val) -> bool {
	matches!(v, Val::Func(_))
}
#[builtin]
pub fn builtin_is_null(v: Val) -> bool {
	matches!(v, Val::Null)
}
