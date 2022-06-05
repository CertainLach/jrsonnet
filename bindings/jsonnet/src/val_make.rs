//! Create values in VM

use std::{
	ffi::CStr,
	os::raw::{c_char, c_double, c_int},
};

use jrsonnet_evaluator::{val::ArrValue, ObjValue, State, Val};
use jrsonnet_gcmodule::Cc;

/// # Safety
///
/// This function is safe, if received v is a pointer to normal C string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_make_string(_vm: &State, v: *const c_char) -> *mut Val {
	let cstr = CStr::from_ptr(v);
	let str = cstr.to_str().unwrap();
	Box::into_raw(Box::new(Val::Str(str.into())))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_number(_vm: &State, v: c_double) -> *mut Val {
	Box::into_raw(Box::new(Val::Num(v)))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_bool(_vm: &State, v: c_int) -> *mut Val {
	assert!(v == 0 || v == 1);
	Box::into_raw(Box::new(Val::Bool(v == 1)))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_null(_vm: &State) -> *mut Val {
	Box::into_raw(Box::new(Val::Null))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_array(_vm: &State) -> *mut Val {
	Box::into_raw(Box::new(Val::Arr(ArrValue::Eager(Cc::new(Vec::new())))))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_object(_vm: &State) -> *mut Val {
	Box::into_raw(Box::new(Val::Obj(ObjValue::new_empty())))
}
