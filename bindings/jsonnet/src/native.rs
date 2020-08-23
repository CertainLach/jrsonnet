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
	let name = CStr::from_ptr(name).to_str().expect("utf8 name").into();
	let mut params = Vec::new();
	loop {
		if (*raw_params).is_null() {
			break;
		}
		let param = CStr::from_ptr(*raw_params).to_str().expect("not utf8");
		params.push(Param(param.into(), None));
		raw_params = raw_params.offset(1);
	}
	let params = ParamsDesc(Rc::new(params));

	vm.add_native(
		name,
		Rc::new(NativeCallback::new(params, move |args| {
			let mut n_args = Vec::new();
			for a in args {
				n_args.push(Some(Box::new(a.clone())));
			}
			n_args.push(None);
			let mut success = 1;
			let v = cb(ctx, &n_args as *const _ as *const *const Val, &mut success);
			let v = *Box::from_raw(v);
			if success == 1 {
				Ok(v)
			} else {
				let e = v.try_cast_str("native error").expect("error msg");
				Err(Error::RuntimeError(e).into())
			}
		})),
	)
}
