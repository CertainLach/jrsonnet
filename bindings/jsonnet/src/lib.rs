use jrsonnet_evaluator::{EvaluationState, ObjValue, Val};
use libc::{c_char, c_double, c_int, c_uint};
use std::{
	ffi::{CStr, CString},
	path::PathBuf,
	rc::Rc,
};

#[no_mangle]
pub extern "C" fn jsonnet_version() -> &'static [u8; 8] {
	b"v0.16.0\0"
}

#[no_mangle]
pub extern "C" fn jsonnet_make() -> Box<EvaluationState> {
	Box::new(EvaluationState::default())
}

// TODO
#[no_mangle]
pub extern "C" fn jsonnet_max_stack(_vm: &EvaluationState, _v: c_uint) {}

// jrsonnet currently have no GC, so these functions is no-op
#[no_mangle]
pub extern "C" fn jsonnet_gc_min_objects(_vm: &EvaluationState, _v: c_uint) {}
#[no_mangle]
pub extern "C" fn jsonnet_gc_growth_trigger(_vm: &EvaluationState, _v: c_double) {}

// TODO
#[no_mangle]
pub extern "C" fn jsonnet_string_output(_vm: &EvaluationState, _v: c_int) {}

#[no_mangle]
pub extern "C" fn jsonnet_json_extract_string(_vm: &EvaluationState, v: &Val) -> *mut c_char {
	match v.unwrap_if_lazy().unwrap() {
		Val::Str(s) => CString::new(&*s as &str).unwrap().into_raw(),
		_ => std::ptr::null_mut(),
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_number(
	_vm: &EvaluationState,
	v: &Val,
	out: &mut c_double,
) -> c_int {
	match v.unwrap_if_lazy().unwrap() {
		Val::Num(n) => {
			*out = n;
			1
		}
		_ => 0,
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_bool(_vm: &EvaluationState, v: &Val) -> c_int {
	match v.unwrap_if_lazy().unwrap() {
		Val::Bool(false) => 0,
		Val::Bool(true) => 1,
		_ => 2,
	}
}
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_null(_vm: &EvaluationState, v: &Val) -> c_int {
	match v.unwrap_if_lazy().unwrap() {
		Val::Null => 1,
		_ => 0,
	}
}

/// # Safety
///
/// This function is safe, if received v is a pointer to normal C string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_make_string(
	_vm: &EvaluationState,
	v: *const c_char,
) -> Box<Val> {
	let cstr = CStr::from_ptr(v);
	let str = cstr.to_str().unwrap();
	Box::new(Val::Str(str.into()))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_number(_vm: &EvaluationState, v: c_double) -> Box<Val> {
	Box::new(Val::Num(v))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_bool(_vm: &EvaluationState, v: c_int) -> Box<Val> {
	assert!(v == 0 || v == 1);
	Box::new(Val::Bool(v == 1))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_null(_vm: &EvaluationState) -> Box<Val> {
	Box::new(Val::Null)
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_array(_vm: &EvaluationState) -> Box<Val> {
	Box::new(Val::Arr(Rc::new(Vec::new())))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_array_append(_vm: &EvaluationState, arr: &mut Val, val: &Val) {
	match arr {
		Val::Arr(old) => {
			let mut new = Vec::new();
			new.extend(old.iter().cloned());
			new.push(val.clone());
			*arr = Val::Arr(Rc::new(new));
		}
		_ => panic!("should receive array"),
	}
}

#[no_mangle]
pub extern "C" fn jsonnet_json_make_object(_vm: &EvaluationState) -> Box<Val> {
	Box::new(Val::Obj(ObjValue::new_empty()))
}

#[no_mangle]
pub extern "C" fn jsonnet_json_object_append(
	_vm: &EvaluationState,
	_obj: &mut Val,
	_name: *const c_char,
	_val: &Val,
) {
	todo!()
}

#[no_mangle]
pub extern "C" fn jsonnet_realloc(_vm: &EvaluationState, _buf: *const u8, _sz: usize) -> *const u8 {
	todo!()
}

#[no_mangle]
#[allow(clippy::boxed_local)]
pub extern "C" fn jsonnet_json_destroy(_vm: &EvaluationState, _v: Box<Val>) {}

#[no_mangle]
pub extern "C" fn jsonnet_import_callback() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_native_callback() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_ext_var() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_ext_code() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_tla_var() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_tla_code() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_max_trace() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_jpath_add() {
	todo!()
}

/// # Safety
///
/// This function is safe, if received v is a pointer to normal C string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file(
	vm: &EvaluationState,
	filename: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	vm.run_in_state(|| {
		use std::fmt::Write;
		let filename = CStr::from_ptr(filename);
		match vm.evaluate_file_to_json(&PathBuf::from(filename.to_str().unwrap())) {
			Ok(v) => {
				*error = 0;
				CString::new(&*v as &str).unwrap().into_raw()
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				writeln!(out, "{:?}", e.0).unwrap();
				for i in (e.1).0.iter() {
					writeln!(out, "{:?}", i).unwrap();
				}
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}
#[no_mangle]
pub extern "C" fn jsonnet_evaluate_snippet() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_evaluate_file_multi() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_evaluate_snippet_multi() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_evaluate_file_stream() {
	todo!()
}
#[no_mangle]
pub extern "C" fn jsonnet_evaluate_snippet_stream() {
	todo!()
}

#[no_mangle]
#[allow(clippy::boxed_local)]
pub extern "C" fn jsonnet_destroy(_vm: Box<EvaluationState>) {}
