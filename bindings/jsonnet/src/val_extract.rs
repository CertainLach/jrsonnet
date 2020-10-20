//! Extract values from VM

use jrsonnet_evaluator::{EvaluationState, Val};

use std::{
	ffi::CString,
	os::raw::{c_char, c_double, c_int},
};

#[no_mangle]
pub extern "C" fn jsonnet_json_extract_string(_vm: &EvaluationState, v: &Val) -> *mut c_char {
	match &v.unwrap_if_lazy().unwrap() {
		Val::Str(s) => CString::new(&*s as &str).unwrap().into_raw(),
		_ => std::ptr::null_mut(),
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_number(
	_vm: &EvaluationState,
	v: &Val,
	out: &mut c_double,
) -> c_int {
	match v.unwrap_if_lazy().unwrap() {
		Val::Num(n) => {
			*out = n;
			1
		}
		_ => 0,
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_bool(_vm: &EvaluationState, v: &Val) -> c_int {
	match v.unwrap_if_lazy().unwrap() {
		Val::Bool(false) => 0,
		Val::Bool(true) => 1,
		_ => 2,
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_null(_vm: &EvaluationState, v: &Val) -> c_int {
	match v.unwrap_if_lazy().unwrap() {
		Val::Null => 1,
		_ => 0,
	}
}
