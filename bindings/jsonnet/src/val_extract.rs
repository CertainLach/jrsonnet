//! Extract values from VM

use std::{
	ffi::CString,
	os::raw::{c_char, c_double, c_int},
};

use jrsonnet_evaluator::{State, Val};

#[no_mangle]
pub extern "C" fn jsonnet_json_extract_string(_vm: &State, v: &Val) -> *mut c_char {
	match v {
		Val::Str(s) => CString::new(s as &str).unwrap().into_raw(),
		_ => std::ptr::null_mut(),
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_number(_vm: &State, v: &Val, out: &mut c_double) -> c_int {
	match v {
		Val::Num(n) => {
			*out = *n;
			1
		}
		_ => 0,
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_bool(_vm: &State, v: &Val) -> c_int {
	match v {
		Val::Bool(false) => 0,
		Val::Bool(true) => 1,
		_ => 2,
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_null(_vm: &State, v: &Val) -> c_int {
	match v {
		Val::Null => 1,
		_ => 0,
	}
}
