use jrsonnet_evaluator::{
	function::builtin,
	val::{ArrValue, Val},
	IStr, ObjValue, ObjValueBuilder,
};

#[builtin]
pub fn builtin_object_fields_ex(
	obj: ObjValue,
	hidden: bool,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Vec<Val> {
	let out = obj.fields_ex(
		hidden,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	);
	out.into_iter().map(Val::string).collect::<Vec<_>>()
}

#[builtin]
pub fn builtin_object_fields(
	o: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Vec<Val> {
	builtin_object_fields_ex(
		o,
		false,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}

#[builtin]
pub fn builtin_object_fields_all(
	o: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> Vec<Val> {
	builtin_object_fields_ex(
		o,
		true,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}

pub fn builtin_object_values_ex(
	o: ObjValue,
	include_hidden: bool,

	#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
) -> ArrValue {
	o.values_ex(
		include_hidden,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}
#[builtin]
pub fn builtin_object_values(
	o: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> ArrValue {
	builtin_object_values_ex(
		o,
		false,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}
#[builtin]
pub fn builtin_object_values_all(
	o: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> ArrValue {
	builtin_object_values_ex(
		o,
		true,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}

pub fn builtin_object_keys_values_ex(
	o: ObjValue,
	include_hidden: bool,

	#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
) -> ArrValue {
	o.key_values_ex(
		include_hidden,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}
#[builtin]
pub fn builtin_object_keys_values(
	o: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> ArrValue {
	builtin_object_keys_values_ex(
		o,
		false,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}
#[builtin]
pub fn builtin_object_keys_values_all(
	o: ObjValue,

	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> ArrValue {
	builtin_object_keys_values_ex(
		o,
		true,
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	)
}

#[builtin]
pub fn builtin_object_has_ex(obj: ObjValue, fname: IStr, hidden: bool) -> bool {
	obj.has_field_ex(fname, hidden)
}

#[builtin]
pub fn builtin_object_has(o: ObjValue, f: IStr) -> bool {
	o.has_field(f)
}

#[builtin]
pub fn builtin_object_has_all(o: ObjValue, f: IStr) -> bool {
	o.has_field_include_hidden(f)
}

#[builtin]
pub fn builtin_object_remove_key(
	obj: ObjValue,
	key: IStr,

	// Standard implementation uses std.objectFields without such argument, we can't
	// assume order preservation should always be enabled/disabled
	//
	// FIXME: On second thought, field order is not observable unless
	// some other call passes uses preserve_order and inspects it.
	// Shouldn't it default to true?
	#[default(false)]
	#[cfg(feature = "exp-preserve-order")]
	preserve_order: bool,
) -> ObjValue {
	let mut new_obj = ObjValueBuilder::with_capacity(obj.len() - 1);
	for (k, v) in obj.iter(
		#[cfg(feature = "exp-preserve-order")]
		preserve_order,
	) {
		if k == key {
			continue;
		}
		new_obj.field(k).value(v.unwrap());
	}

	new_obj.build()
}
