//! Manipulate external variables and top level arguments

use std::{ffi::CStr, os::raw::c_char};

use jrsonnet_evaluator::State;

/// Bind a Jsonnet external var to the given string.
///
/// Argument values are copied so memory should be managed by caller.
///
/// # Safety
///
/// Caller should pass correct pointers as `name` and `code`, they need to be \0-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_ext_var(vm: &State, name: *const c_char, value: *const c_char) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);

	let any_initializer = vm.context_initializer();
	any_initializer
		.as_any()
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_ext_str(
			name.to_str().expect("name is not utf-8").into(),
			value.to_str().expect("value is not utf-8").into(),
		)
}

/// Bind a Jsonnet external var to the given code.
///
/// Argument values are copied so memory should be managed by caller.
///
/// # Safety
///
/// Caller should pass correct pointers as `name` and `code`, they need to be \0-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_ext_code(vm: &State, name: *const c_char, code: *const c_char) {
	let name = CStr::from_ptr(name);
	let code = CStr::from_ptr(code);

	let any_initializer = vm.context_initializer();
	any_initializer
		.as_any()
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_ext_code(
			name.to_str().expect("name is not utf-8"),
			code.to_str().expect("code is not utf-8"),
		)
		.expect("can't parse ext code")
}

/// Bind a string top-level argument for a top-level parameter.
///
/// Argument values are copied so memory should be managed by caller.
///
/// # Safety
///
/// Caller should pass correct pointers as `name` and `value`, they need to be \0-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_tla_var(vm: &State, name: *const c_char, value: *const c_char) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_tla_str(
		name.to_str().expect("name is not utf-8").into(),
		value.to_str().expect("value is not utf-8").into(),
	)
}

/// Bind a code top-level argument for a top-level parameter.
///
/// Argument values are copied so memory should be managed by caller.
///
/// # Safety
///
/// Caller should pass correct pointers as `name` and `code`, they need to be \0-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_tla_code(vm: &State, name: *const c_char, code: *const c_char) {
	let name = CStr::from_ptr(name);
	let code = CStr::from_ptr(code);
	vm.add_tla_code(
		name.to_str().expect("name is not utf-8").into(),
		code.to_str().expect("code is not utf-8"),
	)
	.expect("can't parse tla code")
}
