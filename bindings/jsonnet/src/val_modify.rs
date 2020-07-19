//! Modify VM values
//! Only tested with variables, which haven't altered by code before appearing here
//! In jrsonnet every value is immutable, and this code is probally broken

use jrsonnet_evaluator::{EvaluationState, LazyBinding, LazyVal, ObjMember, ObjValue, Val};
use jrsonnet_parser::Visibility;
use std::{collections::HashMap, ffi::CStr, os::raw::c_char, rc::Rc};

/// # Safety
///
/// Received arr value should be correct pointer to array allocated by make_array
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_array_append(
	_vm: &EvaluationState,
	arr: *mut Val,
	val: &Val,
) {
	match *Box::from_raw(arr) {
		Val::Arr(old) => {
			let mut new = Rc::try_unwrap(old).expect("arr with no refs");
			new.push(val.clone());
			*arr = Val::Arr(Rc::new(new));
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
			let mut new = HashMap::new();
			new.insert(
				CStr::from_ptr(name).to_str().unwrap().into(),
				ObjMember {
					add: false,
					visibility: Visibility::Normal,
					invoke: LazyBinding::Bound(LazyVal::new_resolved(val.clone())),
					location: None,
				},
			);
			let new_obj = ObjValue::new(Some(old.clone()), Rc::new(new));
			*obj = Val::Obj(new_obj);
		}
		_ => panic!("should receive object"),
	}
}
