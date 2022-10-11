use std::{
	borrow::Cow,
	ffi::{c_void, CStr},
	os::raw::{c_char, c_int},
};

use jrsonnet_evaluator::{
	error::{Error, LocError},
	function::builtin::{NativeCallback, NativeCallbackHandler},
	tb,
	typed::Typed,
	IStr, State, Val,
};
use jrsonnet_gcmodule::Cc;

/// The returned `JsonnetJsonValue*` should be allocated with `jsonnet_realloc`. It will be cleaned up
/// along with the objects rooted at `argv` by `libjsonnet` when no-longer needed. Return a string upon
/// failure, which will appear in Jsonnet as an error. The `argv` pointer is an array whose size
/// matches the array of parameters supplied when the native callback was originally registered.
///
/// - `ctx` User pointer, given in jsonnet_native_callback.
/// - `argv` Array of arguments from Jsonnet code.
/// - `param` success Set this byref param to 1 to indicate success and 0 for failure.
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
	fn call(&self, s: State, args: &[Val]) -> Result<Val, LocError> {
		let mut n_args = Vec::new();
		for a in args {
			n_args.push(Some(Box::new(a.clone())));
		}
		n_args.push(None);
		let mut success = 1;
		let v = unsafe {
			(self.cb)(
				self.ctx,
				&n_args as *const _ as *const *const Val,
				&mut success,
			)
		};
		let v = unsafe { *Box::from_raw(v) };
		if success == 1 {
			Ok(v)
		} else {
			let e = IStr::from_untyped(v, s).expect("error msg should be a string");
			Err(Error::RuntimeError(e).into())
		}
	}
}

/// Callback to provide native extensions to Jsonnet.
///
/// # Safety
///
/// `vm` should be a vm allocated by `jsonnet_make`
/// `cb` should be a correct function pointer
/// `raw_params` should point to a NULL-terminated string array
/// `name`, `raw_params` elements should be a \0-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_native_callback(
	vm: &State,
	name: *const c_char,
	cb: JsonnetNativeCallback,
	ctx: *const c_void,
	mut raw_params: *const *const c_char,
) {
	let name = CStr::from_ptr(name)
		.to_str()
		.expect("name is not utf-8")
		.into();
	let mut params = Vec::new();
	loop {
		if (*raw_params).is_null() {
			break;
		}
		let param = CStr::from_ptr(*raw_params)
			.to_str()
			.expect("param name is not utf-8");
		params.push(Cow::Owned(param.into()));
		raw_params = raw_params.offset(1);
	}

	let any_resolver = vm.context_initializer();
	any_resolver
		.as_any()
		.downcast_ref::<jrsonnet_stdlib::ContextInitializer>()
		.expect("only stdlib context initializer supported")
		.add_native(
			name,
			#[allow(deprecated)]
			Cc::new(tb!(NativeCallback::new(
				params,
				tb!(JsonnetNativeCallbackHandler { ctx, cb }),
			))),
		)
}
