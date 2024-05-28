//! Create values in VM

use std::{
	ffi::CStr,
	os::raw::{c_char, c_double, c_int},
};

use jrsonnet_evaluator::{
	val::{ArrValue, NumValue},
	ObjValue, Val,
};

use crate::{OwnedVal, VMRef, VM};

/// Convert the given `UTF-8` string to a `JsonnetJsonValue`.
///
/// # Safety
///
/// `v` should be a NUL-terminated string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_make_string(vm: VMRef, val: *const c_char) -> OwnedVal {
	let val = unsafe { CStr::from_ptr(val) };
	let val = val.to_str().expect("string is not utf-8");
	let mut out = OwnedVal::invalid();
	vm.run_in_thread(|_| out = OwnedVal::new(Val::string(val)));
	out
}

/// Convert the given double to a `JsonnetJsonValue`.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_number(vm: VMRef, v: c_double) -> OwnedVal {
	let mut out = OwnedVal::invalid();
	vm.run_in_thread(|_| {
		out = OwnedVal::new(Val::Num(
			NumValue::new(v).expect("jsonnet numbers are finite"),
		));
	});
	out
}

/// Convert the given `bool` (`1` or `0`) to a `JsonnetJsonValue`.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_bool(_vm: &VM, v: c_int) -> OwnedVal {
	assert!(v == 0 || v == 1, "bad boolean value");
	OwnedVal::new(Val::Bool(v == 1))
}

/// Make a `JsonnetJsonValue` representing `null`.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_null(_vm: &VM) -> OwnedVal {
	OwnedVal::new(Val::Null)
}

/// Make a `JsonnetJsonValue` representing an array.
///
/// Assign elements with [`jsonnet_json_array_append`].
#[no_mangle]
pub extern "C" fn jsonnet_json_make_array(vm: VMRef) -> OwnedVal {
	let mut out = OwnedVal::invalid();
	vm.run_in_thread(|_| out = OwnedVal::new(Val::Arr(ArrValue::eager(vec![]))));
	out
}

/// Make a `JsonnetJsonValue` representing an object.
#[no_mangle]
pub extern "C" fn jsonnet_json_make_object(vm: VMRef) -> OwnedVal {
	let mut out = OwnedVal::invalid();
	vm.run_in_thread(|_| out = OwnedVal::new(Val::Obj(ObjValue::new_empty())));
	out
}
