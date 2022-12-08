use jrsonnet_evaluator::{
	function::builtin,
	val::{StrValue, Val},
	IStr, ObjValue,
};

#[builtin]
pub fn builtin_object_fields_ex(
	obj: ObjValue,
	hidden: bool,
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> Vec<Val> {
	#[cfg(feature = "exp-preserve-order")]
	let preserve_order = preserve_order.unwrap_or(false);
	let out = obj.fields_ex(
		hidden,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	);
	out.into_iter()
		.map(StrValue::Flat)
		.map(Val::Str)
		.collect::<Vec<_>>()
}

#[builtin]
pub fn builtin_object_has_ex(obj: ObjValue, fname: IStr, hidden: bool) -> bool {
	obj.has_field_ex(fname, hidden)
}
