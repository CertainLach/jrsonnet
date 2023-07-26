use jrsonnet_evaluator::{
	function::builtin,
	val::{StrValue, Val},
	IStr, ObjValue, ObjValueBuilder,
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

#[builtin]
pub fn builtin_object_remove_key(
	obj: ObjValue,
	key: IStr,
	// Standard implementation uses std.objectFields without such argument, we can't
	// assume order preservation should always be enabled/disabled
	#[cfg(feature = "exp-preserve-order")] preserve_order: Option<bool>,
) -> ObjValue {
	#[cfg(feature = "exp-preserve-order")]
	let preserve_order = preserve_order.unwrap_or(false);
	let mut new_obj = ObjValueBuilder::with_capacity(obj.len() - 1);
	for (k, v) in obj.iter(
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	) {
		if k == key {
			continue;
		}
		new_obj.member(k).value_unchecked(v.unwrap())
	}

	new_obj.build()
}
