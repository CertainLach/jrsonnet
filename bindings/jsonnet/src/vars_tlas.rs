//! Manipulate external variables and top level arguments

use std::{ffi::CStr, os::raw::c_char};

use jrsonnet_evaluator::EvaluationState;

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_ext_var(
	vm: &EvaluationState,
	name: *const c_char,
	value: *const c_char,
) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_ext_str(
		name.to_str().unwrap().into(),
		value.to_str().unwrap().into(),
	)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_ext_code(
	vm: &EvaluationState,
	name: *const c_char,
	value: *const c_char,
) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_ext_code(
		name.to_str().unwrap().into(),
		value.to_str().unwrap().into(),
	)
	.unwrap()
}
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_tla_var(
	vm: &EvaluationState,
	name: *const c_char,
	value: *const c_char,
) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_tla_str(
		name.to_str().unwrap().into(),
		value.to_str().unwrap().into(),
	)
}
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_tla_code(
	vm: &EvaluationState,
	name: *const c_char,
	value: *const c_char,
) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_tla_code(
		name.to_str().unwrap().into(),
		value.to_str().unwrap().into(),
	)
	.unwrap()
}
