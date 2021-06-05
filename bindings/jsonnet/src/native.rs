use jrsonnet_evaluator::{error::Error, native::NativeCallback, EvaluationState, Val};
use jrsonnet_parser::{Param, ParamsDesc};
use std::{
	ffi::{c_void, CStr},
	os::raw::{c_char, c_int},
	rc::Rc,
};

type JsonnetNativeCallback = unsafe extern "C" fn(
	ctx: *const c_void,
	argv: *const *const Val,
	success: *mut c_int,
) -> *mut Val;

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_native_callback(
	vm: &EvaluationState,
	name: *const c_char,
	cb: JsonnetNativeCallback,
	ctx: *const c_void,
	mut raw_params: *const *const c_char,
) {
	todo!()
}
