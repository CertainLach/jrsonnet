use jrsonnet_evaluator::{
	error::Result,
	function::builtin,
	typed::VecVal,
	val::{StrValue, Val},
	IStr, ObjValue,
};
use jrsonnet_gcmodule::Cc;

#[builtin]
pub fn builtin_object_fields_ex(
	obj: ObjValue,
	inc_hidden: bool,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Result<VecVal> {
	#[cfg(feature = "exp-preserve-order")]
	let preserve_order = preserve_order.unwrap_or(false);
	let out = obj.fields_ex(
		inc_hidden,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	);
	Ok(VecVal(Cc::new(
		out.into_iter()
			.map(StrValue::Flat)
			.map(Val::Str)
			.collect::<Vec<_>>(),
	)))
}

#[builtin]
pub fn builtin_object_has_ex(obj: ObjValue, f: IStr, inc_hidden: bool) -> Result<bool> {
	Ok(obj.has_field_ex(f, inc_hidden))
}
