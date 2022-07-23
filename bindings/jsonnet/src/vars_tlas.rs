//! Manipulate external variables and top level arguments

use std::{ffi::CStr, os::raw::c_char};

use jrsonnet_evaluator::State;

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_ext_var(vm: &State, name: *const c_char, value: *const c_char) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);

	let any_resolver = vm.context_initializer();
	any_resolver
		.as_any()
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_ext_str(
			name.to_str().unwrap().into(),
			value.to_str().unwrap().into(),
		)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_ext_code(vm: &State, name: *const c_char, value: *const c_char) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);

	let any_resolver = vm.context_initializer();
	any_resolver
		.as_any()
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_ext_code(name.to_str().unwrap(), value.to_str().unwrap().into())
		.unwrap()
}
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_tla_var(vm: &State, name: *const c_char, value: *const c_char) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_tla_str(
		name.to_str().unwrap().into(),
		value.to_str().unwrap().into(),
	)
}
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_tla_code(vm: &State, name: *const c_char, value: *const c_char) {
	let name = CStr::from_ptr(name);
	let value = CStr::from_ptr(value);
	vm.add_tla_code(name.to_str().unwrap().into(), value.to_str().unwrap())
		.unwrap()
}
