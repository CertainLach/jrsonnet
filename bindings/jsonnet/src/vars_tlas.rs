//! Manipulate external variables and top level arguments

use std::{any::Any, ffi::CStr, os::raw::c_char};

use jrsonnet_evaluator::{function::TlaArg, IStr};

use crate::VM;

/// Binds a Jsonnet external variable to the given string.
///
/// Argument values are copied so memory should be managed by the caller.
///
/// # Safety
///
/// `name`, `code` should be a NUL-terminated strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jsonnet_ext_var(vm: &VM, name: *const c_char, value: *const c_char) {
	let name = unsafe { CStr::from_ptr(name) };
	let value = unsafe { CStr::from_ptr(value) };

	let any_initializer = vm.state.context_initializer();
	(any_initializer as &dyn Any)
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_ext_str(
			name.to_str().expect("name is not utf-8").into(),
			value.to_str().expect("value is not utf-8").into(),
		);
}

/// Binds a Jsonnet external variable to the given code.
///
/// Argument values are copied so memory should be managed by the caller.
///
/// # Safety
///
/// `name`, `code` should be a NUL-terminated strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jsonnet_ext_code(vm: &VM, name: *const c_char, code: *const c_char) {
	let name = unsafe { CStr::from_ptr(name) };
	let code = unsafe { CStr::from_ptr(code) };

	let any_initializer = vm.state.context_initializer();
	(any_initializer as &dyn Any)
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_ext_code(
			name.to_str().expect("name is not utf-8"),
			code.to_str().expect("code is not utf-8"),
		)
		.expect("can't parse ext code");
}

/// Binds a top-level string argument for a top-level parameter.
///
/// Argument values are copied so memory should be managed by the caller.
///
/// # Safety
///
/// `name`, `value` should be a NUL-terminated strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jsonnet_tla_var(vm: &mut VM, name: *const c_char, value: *const c_char) {
	let name = unsafe { CStr::from_ptr(name) };
	let value = unsafe { CStr::from_ptr(value) };
	vm.tla_args.insert(
		name.to_str().expect("name is not utf-8").into(),
		TlaArg::String(value.to_str().expect("value is not utf-8").into()),
	);
}

/// Binds a top-level code argument for a top-level parameter.
///
/// Argument values are copied so memory should be managed by the caller.
///
/// # Safety
///
/// `name`, `code` should be a NUL-terminated strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jsonnet_tla_code(vm: &mut VM, name: *const c_char, code: *const c_char) {
	let name = unsafe { CStr::from_ptr(name) };
	let code = unsafe { CStr::from_ptr(code) };

	let name: IStr = name.to_str().expect("name is not utf-8").into();
	let code: String = code.to_str().expect("code is not utf-8").to_owned();

	vm.tla_args.insert(name, TlaArg::InlineCode(code));
}
