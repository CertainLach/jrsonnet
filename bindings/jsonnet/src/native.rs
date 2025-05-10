use std::{
	any::Any,
	ffi::{c_void, CStr},
	os::raw::{c_char, c_int},
};

use jrsonnet_evaluator::{
	error::{Error, ErrorKind},
	function::{NativeCallback, NativeCallbackHandler},
	typed::FromUntyped,
	IStr, Val,
};

use crate::VM;

/// The returned `JsonnetJsonValue*` should be allocated with `jsonnet_realloc`. It will be cleaned up
/// along with the objects rooted at `argv` by `libjsonnet` when no-longer needed. Return a string upon
/// failure, which will appear in Jsonnet as an error. The `argv` pointer is an array whose size
/// matches the array of parameters supplied when the native callback was originally registered.
///
/// - `ctx` User pointer, given in `jsonnet_native_callback`.
/// - `argv` Array of arguments from Jsonnet code.
/// - `param` success Set this byref param to 1 to indicate success and 0 for failure.
///
/// Returns the content of the imported file, or an error message.
type JsonnetNativeCallback = unsafe extern "C" fn(
	ctx: *const c_void,
	argv: *const *const Val,
	success: *mut c_int,
) -> *mut Val;

#[derive(jrsonnet_gcmodule::Trace)]
struct JsonnetNativeCallbackHandler {
	#[trace(skip)]
	ctx: *const c_void,
	#[trace(skip)]
	cb: JsonnetNativeCallback,
}
impl NativeCallbackHandler for JsonnetNativeCallbackHandler {
	fn call(&self, args: &[Val]) -> Result<Val, Error> {
		let mut n_args = Vec::new();
		for a in args {
			n_args.push(Some(Box::new(a.clone())));
		}
		n_args.push(None);
		let mut success = 1;
		let v = unsafe { (self.cb)(self.ctx, n_args.as_ptr().cast(), &mut success) };
		let v = unsafe { *Box::from_raw(v) };
		if success == 1 {
			Ok(v)
		} else {
			let e = IStr::from_untyped(v).expect("error msg should be a string");
			Err(ErrorKind::RuntimeError(e).into())
		}
	}
}

/// Callback to provide native extensions to Jsonnet.
///
/// # Safety
///
/// `vm` should be a vm allocated by `jsonnet_make`
/// `name` should be a NUL-terminated string
/// `cb` should be a function pointer
/// `raw_params` should point to a NULL-terminated array of NUL-terminated strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jsonnet_native_callback(
	vm: &VM,
	name: *const c_char,
	cb: JsonnetNativeCallback,
	ctx: *const c_void,
	mut raw_params: *const *const c_char,
) {
	let name = unsafe { CStr::from_ptr(name).to_str().expect("name is not utf-8") };
	let mut params = Vec::new();
	loop {
		if (unsafe { *raw_params }).is_null() {
			break;
		}
		let param = unsafe {
			CStr::from_ptr(*raw_params)
				.to_str()
				.expect("param name is not utf-8")
		};
		params.push(param.into());
		raw_params = unsafe { raw_params.offset(1) };
	}

	let any_resolver = vm.state.context_initializer();
	(any_resolver as &dyn Any)
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_native(
			name,
			#[allow(deprecated)]
			NativeCallback::new(params, JsonnetNativeCallbackHandler { ctx, cb }),
		);
}
