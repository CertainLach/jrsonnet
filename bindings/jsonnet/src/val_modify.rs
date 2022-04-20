//! Modify VM values
//! Only tested with variables, which haven't altered by code before appearing here
//! In jrsonnet every value is immutable, and this code is probally broken

use std::{ffi::CStr, os::raw::c_char};

use gcmodule::Cc;
use jrsonnet_evaluator::{val::ArrValue, EvaluationState, LazyVal, Val};

/// # Safety
///
/// Received arr value should be correct pointer to array allocated by make_array
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_array_append(
	_vm: &EvaluationState,
	arr: &mut Val,
	val: &Val,
) {
	match arr {
		Val::Arr(old) => {
			let mut new = Vec::new();
			for item in old.iter_lazy() {
				new.push(item);
			}
			new.push(LazyVal::new_resolved(val.clone()));
			*arr = Val::Arr(ArrValue::Lazy(Cc::new(new)));
		}
		_ => panic!("should receive array"),
	}
}

/// # Safety
///
/// This function is safe if passed name is ok
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_object_append(
	_vm: &EvaluationState,
	obj: &mut Val,
	name: *const c_char,
	val: &Val,
) {
	match obj {
		Val::Obj(old) => old
			.extend_field(CStr::from_ptr(name).to_str().unwrap().into())
			.value(val.clone()),
		_ => panic!("should receive object"),
	}
}
