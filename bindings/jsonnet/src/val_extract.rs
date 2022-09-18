//! Extract values from VM

use std::{
	ffi::CString,
	os::raw::{c_char, c_double, c_int},
};

use jrsonnet_evaluator::{State, Val};

/// If the value is a string, return it as UTF-8, otherwise return `NULL`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_string(_vm: &State, v: &Val) -> *mut c_char {
	match v {
		Val::Str(s) => CString::new(s as &str).unwrap().into_raw(),
		_ => std::ptr::null_mut(),
	}
}

/// If the value is a number, return `1` and store the number in out, otherwise return `0`.
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

/// Return `0` if the value is `false`, `1` if it is `true`, and `2` if it is not a `bool`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_bool(_vm: &State, v: &Val) -> c_int {
	match v {
		Val::Bool(false) => 0,
		Val::Bool(true) => 1,
		_ => 2,
	}
}

/// Return `1` if the value is `null`, otherwise return `0`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_null(_vm: &State, v: &Val) -> c_int {
	match v {
		Val::Null => 1,
		_ => 0,
	}
}
