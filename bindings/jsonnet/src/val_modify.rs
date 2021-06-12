//! Modify VM values
//! Only tested with variables, which haven't altered by code before appearing here
//! In jrsonnet every value is immutable, and this code is probally broken

use gc::Gc;
use jrsonnet_evaluator::{ArrValue, EvaluationState, LazyBinding, LazyVal, ObjMember, Val};
use jrsonnet_parser::Visibility;
use std::{ffi::CStr, os::raw::c_char};

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
			*arr = Val::Arr(ArrValue::Lazy(Gc::new(new)));
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
		Val::Obj(old) => {
			let new_obj = old.clone().extend_with_field(
				CStr::from_ptr(name).to_str().unwrap().into(),
				ObjMember {
					add: false,
					visibility: Visibility::Normal,
					invoke: LazyBinding::Bound(LazyVal::new_resolved(val.clone())),
					location: None,
				},
			);

			*obj = Val::Obj(new_obj);
		}
		_ => panic!("should receive object"),
	}
}
