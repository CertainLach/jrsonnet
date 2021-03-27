//! Jrsonnet specific additional binding helpers

use crate::{import::jsonnet_import_callback, native::jsonnet_native_callback};
use jrsonnet_evaluator::{EvaluationState, Val};
use std::{
	ffi::c_void,
	os::raw::{c_char, c_int},
};

extern "C" {
	pub fn _jrsonnet_static_import_callback(
		ctx: *mut c_void,
		base: *const c_char,
		rel: *const c_char,
		found_here: *mut *const c_char,
		success: &mut c_int,
	) -> *const c_char;

	#[allow(improper_ctypes)]
	pub fn _jrsonnet_static_native_callback(
		ctx: *const c_void,
		argv: *const *const Val,
		success: *mut c_int,
	) -> *mut Val;
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jrsonnet_apply_static_import_callback(
	vm: &EvaluationState,
	ctx: *mut c_void,
) {
	jsonnet_import_callback(vm, _jrsonnet_static_import_callback, ctx)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jrsonnet_apply_static_native_callback(
	vm: &EvaluationState,
	name: *const c_char,
	ctx: *mut c_void,
	raw_params: *const *const c_char,
) {
	jsonnet_native_callback(vm, name, _jrsonnet_static_native_callback, ctx, raw_params)
}

#[no_mangle]
pub extern "C" fn jrsonnet_set_trace_format(vm: &EvaluationState, format: u8) {
	use jrsonnet_evaluator::trace::JsFormat;
	match format {
		1 => vm.set_trace_format(Box::new(JsFormat)),
		_ => panic!("unknown trace format"),
	}
}
