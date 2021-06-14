#[cfg(feature = "interop")]
pub mod interop;

pub mod import;
pub mod native;
pub mod val_extract;
pub mod val_make;
pub mod val_modify;
pub mod vars_tlas;

use import::NativeImportResolver;
use jrsonnet_evaluator::{EvaluationState, IStr, ManifestFormat, Val};
use std::{
	alloc::Layout,
	ffi::{CStr, CString},
	os::raw::{c_char, c_double, c_int, c_uint},
	path::PathBuf,
	rc::Rc,
};

/// WASM stub
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn _start() {}

#[no_mangle]
pub extern "C" fn jsonnet_version() -> &'static [u8; 8] {
	b"v0.16.0\0"
}

#[no_mangle]
pub extern "C" fn jsonnet_make() -> *mut EvaluationState {
	let state = EvaluationState::default();
	state.with_stdlib();
	state.settings_mut().import_resolver = Box::new(NativeImportResolver::default());
	Box::into_raw(Box::new(state))
}

/// # Safety
#[no_mangle]
#[allow(clippy::boxed_local)]
pub unsafe extern "C" fn jsonnet_destroy(vm: *mut EvaluationState) {
	Box::from_raw(vm);
}

#[no_mangle]
pub extern "C" fn jsonnet_max_stack(vm: &EvaluationState, v: c_uint) {
	vm.settings_mut().max_stack = v as usize;
}

// jrsonnet currently have no GC, so these functions is no-op
#[no_mangle]
pub extern "C" fn jsonnet_gc_min_objects(_vm: &EvaluationState, _v: c_uint) {}
#[no_mangle]
pub extern "C" fn jsonnet_gc_growth_trigger(_vm: &EvaluationState, _v: c_double) {}

#[no_mangle]
pub extern "C" fn jsonnet_string_output(vm: &EvaluationState, v: c_int) {
	match v {
		1 => vm.set_manifest_format(ManifestFormat::String),
		0 => vm.set_manifest_format(ManifestFormat::Json(4)),
		_ => panic!("incorrect output format"),
	}
}

/// # Safety
///
/// This function is most definitely broken, but it works somehow, see TODO inside
#[no_mangle]
pub unsafe extern "C" fn jsonnet_realloc(
	_vm: &EvaluationState,
	buf: *mut u8,
	sz: usize,
) -> *mut u8 {
	if buf.is_null() {
		assert!(sz != 0);
		return std::alloc::alloc(Layout::from_size_align(sz, std::mem::align_of::<u8>()).unwrap());
	}
	// TODO: Somehow store size of allocation, because its real size is probally not 16 :D
	// OR (Alternative way of fixing this TODO)
	// TODO: Standard allocator uses malloc, and it doesn't uses allocation size,
	// TODO: so it should work in normal cases. Maybe force allocator for this library?
	let old_layout = Layout::from_size_align(16, std::mem::align_of::<u8>()).unwrap();
	if sz == 0 {
		std::alloc::dealloc(buf, old_layout);
		return std::ptr::null_mut();
	}
	std::alloc::realloc(buf, old_layout, sz)
}

/// # Safety
#[no_mangle]
#[allow(clippy::boxed_local)]
pub unsafe extern "C" fn jsonnet_json_destroy(_vm: &EvaluationState, v: *mut Val) {
	Box::from_raw(v);
}

#[no_mangle]
pub extern "C" fn jsonnet_max_trace(vm: &EvaluationState, v: c_uint) {
	vm.set_max_trace(v as usize)
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
		let filename = CStr::from_ptr(filename);
		match vm
			.evaluate_file_raw_nocwd(&PathBuf::from(filename.to_str().unwrap()))
			.and_then(|v| vm.with_tla(v))
			.and_then(|v| vm.manifest(v))
		{
			Ok(v) => {
				*error = 0;
				CString::new(&*v as &str).unwrap().into_raw()
			}
			Err(e) => {
				*error = 1;
				let out = vm.stringify_err(&e);
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}

/// # Safety
///
/// This function is safe, if received v is a pointer to normal C string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet(
	vm: &EvaluationState,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	vm.run_in_state(|| {
		let filename = CStr::from_ptr(filename);
		let snippet = CStr::from_ptr(snippet);
		match vm
			.evaluate_snippet_raw(
				Rc::new(PathBuf::from(filename.to_str().unwrap())),
				snippet.to_str().unwrap().into(),
			)
			.and_then(|v| vm.with_tla(v))
			.and_then(|v| vm.manifest(v))
		{
			Ok(v) => {
				*error = 0;
				CString::new(&*v as &str).unwrap().into_raw()
			}
			Err(e) => {
				*error = 1;
				let out = vm.stringify_err(&e);
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}

fn multi_to_raw(multi: Vec<(IStr, IStr)>) -> *const c_char {
	let mut out = Vec::new();
	for (i, (k, v)) in multi.iter().enumerate() {
		if i != 0 {
			out.push(0);
		}
		out.extend_from_slice(k.as_bytes());
		out.push(0);
		out.extend_from_slice(v.as_bytes());
	}
	out.push(0);
	out.push(0);
	let v = out.as_ptr();
	std::mem::forget(out);
	v as *const c_char
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file_multi(
	vm: &EvaluationState,
	filename: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	vm.run_in_state(|| {
		let filename = CStr::from_ptr(filename);
		match vm
			.evaluate_file_raw_nocwd(&PathBuf::from(filename.to_str().unwrap()))
			.and_then(|v| vm.with_tla(v))
			.and_then(|v| vm.manifest_multi(v))
		{
			Ok(v) => {
				*error = 0;
				multi_to_raw(v)
			}
			Err(e) => {
				*error = 1;
				let out = vm.stringify_err(&e);
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet_multi(
	vm: &EvaluationState,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	vm.run_in_state(|| {
		let filename = CStr::from_ptr(filename);
		let snippet = CStr::from_ptr(snippet);
		match vm
			.evaluate_snippet_raw(
				Rc::new(PathBuf::from(filename.to_str().unwrap())),
				snippet.to_str().unwrap().into(),
			)
			.and_then(|v| vm.with_tla(v))
			.and_then(|v| vm.manifest_multi(v))
		{
			Ok(v) => {
				*error = 0;
				multi_to_raw(v)
			}
			Err(e) => {
				*error = 1;
				let out = vm.stringify_err(&e);
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}

fn stream_to_raw(multi: Vec<IStr>) -> *const c_char {
	let mut out = Vec::new();
	for (i, v) in multi.iter().enumerate() {
		if i != 0 {
			out.push(0);
		}
		out.extend_from_slice(v.as_bytes());
	}
	out.push(0);
	out.push(0);
	let v = out.as_ptr();
	std::mem::forget(out);
	v as *const c_char
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file_stream(
	vm: &EvaluationState,
	filename: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	vm.run_in_state(|| {
		let filename = CStr::from_ptr(filename);
		match vm
			.evaluate_file_raw_nocwd(&PathBuf::from(filename.to_str().unwrap()))
			.and_then(|v| vm.with_tla(v))
			.and_then(|v| vm.manifest_stream(v))
		{
			Ok(v) => {
				*error = 0;
				stream_to_raw(v)
			}
			Err(e) => {
				*error = 1;
				let out = vm.stringify_err(&e);
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet_stream(
	vm: &EvaluationState,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	vm.run_in_state(|| {
		let filename = CStr::from_ptr(filename);
		let snippet = CStr::from_ptr(snippet);
		match vm
			.evaluate_snippet_raw(
				Rc::new(PathBuf::from(filename.to_str().unwrap())),
				snippet.to_str().unwrap().into(),
			)
			.and_then(|v| vm.with_tla(v))
			.and_then(|v| vm.manifest_stream(v))
		{
			Ok(v) => {
				*error = 0;
				stream_to_raw(v)
			}
			Err(e) => {
				*error = 1;
				let out = vm.stringify_err(&e);
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
}
