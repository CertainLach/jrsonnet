use std::{
	ffi::{c_void, CStr},
	os::raw::{c_char, c_int},
};

use jrsonnet_evaluator::{
	error::{Error, LocError},
	function::builtin::{BuiltinParam, NativeCallback, NativeCallbackHandler},
	tb,
	typed::Typed,
	IStr, State, Val,
};
use jrsonnet_gcmodule::Cc;

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
			let e = IStr::from_untyped(v, s).expect("error msg");
			Err(Error::RuntimeError(e).into())
		}
	}
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_native_callback(
	vm: &State,
	name: *const c_char,
	cb: JsonnetNativeCallback,
	ctx: *const c_void,
	mut raw_params: *const *const c_char,
) {
	let name = CStr::from_ptr(name).to_str().expect("utf8 name").into();
	let mut params = Vec::new();
	loop {
		if (*raw_params).is_null() {
			break;
		}
		let param = CStr::from_ptr(*raw_params).to_str().expect("not utf8");
		params.push(BuiltinParam {
			name: param.into(),
			has_default: false,
		});
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
