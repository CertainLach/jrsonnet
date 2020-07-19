//! Jrsonnet specific additional binding helpers

use crate::import::jsonnet_import_callback;
use jrsonnet_evaluator::EvaluationState;
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
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jrsonnet_apply_static_import_callback(
	vm: &EvaluationState,
	ctx: *mut c_void,
) {
	jsonnet_import_callback(vm, _jrsonnet_static_import_callback, ctx)
}

#[no_mangle]
pub extern "C" fn jrsonnet_set_trace_format(vm: &EvaluationState, format: u8) {
	use jrsonnet_evaluator::trace::JSFormat;
	match format {
		1 => vm.set_trace_format(Box::new(JSFormat)),
		_ => panic!("unknown trace format"),
	}
}
