//! Create values in VM

use std::{
	ffi::CStr,
	os::raw::{c_char, c_double, c_int},
};

use jrsonnet_evaluator::{
	val::{ArrValue, NumValue},
	ObjValue, Val,
};

use crate::VM;

/// Convert the given `UTF-8` string to a `JsonnetJsonValue`.
///
/// # Safety
///
/// `v` should be a NUL-terminated string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_make_string(_vm: &VM, val: *const c_char) -> *mut Val {
	let val = unsafe { CStr::from_ptr(val) };
	let val = val.to_str().expect("string is not utf-8");
	Box::into_raw(Box::new(Val::string(val)))
}

/// Convert the given double to a `JsonnetJsonValue`.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_number(_vm: &VM, v: c_double) -> *mut Val {
	Box::into_raw(Box::new(Val::Num(
		NumValue::new(v).expect("jsonnet numbers are finite"),
	)))
}

/// Convert the given `bool` (`1` or `0`) to a `JsonnetJsonValue`.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_bool(_vm: &VM, v: c_int) -> *mut Val {
	assert!(v == 0 || v == 1, "bad boolean value");
	Box::into_raw(Box::new(Val::Bool(v == 1)))
}

/// Make a `JsonnetJsonValue` representing `null`.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_null(_vm: &VM) -> *mut Val {
	Box::into_raw(Box::new(Val::Null))
}

/// Make a `JsonnetJsonValue` representing an array.
///
/// Assign elements with [`jsonnet_json_array_append`].
#[no_mangle]
pub extern "C" fn jsonnet_json_make_array(_vm: &VM) -> *mut Val {
	Box::into_raw(Box::new(Val::Arr(ArrValue::eager(Vec::new()))))
}

/// Make a `JsonnetJsonValue` representing an object.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_object(_vm: &VM) -> *mut Val {
	Box::into_raw(Box::new(Val::Obj(ObjValue::empty())))
}
