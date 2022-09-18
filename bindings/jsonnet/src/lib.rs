#[cfg(feature = "interop")]
pub mod interop;

pub mod import;
pub mod native;
pub mod val_extract;
pub mod val_make;
pub mod val_modify;
pub mod vars_tlas;

use std::{
	alloc::Layout,
	borrow::Cow,
	ffi::{CStr, CString, OsStr},
	os::raw::{c_char, c_double, c_int, c_uint},
	path::Path,
};

use jrsonnet_evaluator::{
	trace::PathResolver, FileImportResolver, IStr, ManifestFormat, State, Val,
};

/// WASM stub
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn _start() {}

/// Return the version string of the Jsonnet interpreter.
/// Conforms to [semantic versioning](http://semver.org/).
/// If this does not match `LIB_JSONNET_VERSION`
/// then there is a mismatch between header and compiled library.
#[no_mangle]
pub extern "C" fn jsonnet_version() -> &'static [u8; 8] {
	b"v0.16.0\0"
}

unsafe fn parse_path(input: &CStr) -> Cow<Path> {
	#[cfg(target_family = "unix")]
	{
		use std::os::unix::ffi::OsStrExt;
		let str = OsStr::from_bytes(input.to_bytes());
		Cow::Borrowed(Path::new(str))
	}
	#[cfg(target_family = "windows")]
	{
		use std::os::windows::ffi::OsStringExt;
		let str = input.to_str().expect("input is not utf8");
		let wide = str.encode_utf16().collect::<Vec<_>>();
		let wide = OsString::from_wide(&wide);
		Cow::Owned(PathBuf::new(wide))
	}
	#[cfg(not(any(target_family = "unix", target_family = "windows")))]
	{
		compile_error!("unsupported os")
	}
}

unsafe fn unparse_path(input: &Path) -> Cow<CStr> {
	#[cfg(target_family = "unix")]
	{
		use std::os::unix::ffi::OsStrExt;
		let str = CString::new(input.as_os_str().as_bytes()).expect("input has zero byte in it");
		Cow::Owned(str)
	}
	#[cfg(not(any(target_family = "unix", target_family = "windows")))]
	{
		compile_error!("unsupported os")
	}
}

/// Creates a new Jsonnet virtual machine.
#[no_mangle]
pub extern "C" fn jsonnet_make() -> *mut State {
	let state = State::default();
	state.settings_mut().import_resolver = Box::new(FileImportResolver::default());
	state.settings_mut().context_initializer = Box::new(jrsonnet_stdlib::ContextInitializer::new(
		state.clone(),
		PathResolver::new_cwd_fallback(),
	));
	Box::into_raw(Box::new(state))
}

/// Complement of [`jsonnet_vm_make`].
#[no_mangle]
#[allow(clippy::boxed_local)]
pub extern "C" fn jsonnet_destroy(vm: Box<State>) {
	drop(vm);
}

/// Set the maximum stack depth.
#[no_mangle]
pub extern "C" fn jsonnet_max_stack(vm: &State, v: c_uint) {
	vm.settings_mut().max_stack = v as usize;
}

/// Set the number of objects required before a garbage collection cycle is allowed.
///
/// No-op for now
#[no_mangle]
pub extern "C" fn jsonnet_gc_min_objects(_vm: &State, _v: c_uint) {}

/// Run the garbage collector after this amount of growth in the number of objects
///
/// No-op for now
#[no_mangle]
pub extern "C" fn jsonnet_gc_growth_trigger(_vm: &State, _v: c_double) {}

/// Expect a string as output and don't JSON encode it.
#[no_mangle]
pub extern "C" fn jsonnet_string_output(vm: &State, v: c_int) {
	match v {
		1 => vm.set_manifest_format(ManifestFormat::String),
		0 => vm.set_manifest_format(ManifestFormat::Json {
			padding: 4,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order: false,
		}),
		_ => panic!("incorrect output format"),
	}
}

/// Allocate, resize, or free a buffer.  This will abort if the memory cannot be allocated. It will
/// only return NULL if sz was zero.
///
/// # Safety
///
/// `buf` should be either previosly allocated by this library, or NULL
///
/// This function is most definitely broken, but it works somehow, see TODO inside
#[no_mangle]
pub unsafe extern "C" fn jsonnet_realloc(_vm: &State, buf: *mut u8, sz: usize) -> *mut u8 {
	if buf.is_null() {
		if sz == 0 {
			return std::ptr::null_mut();
		}
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

/// Clean up a JSON subtree.
///
/// This is useful if you want to abort with an error mid-way through building a complex value.
#[no_mangle]
#[allow(clippy::boxed_local)]
pub extern "C" fn jsonnet_json_destroy(_vm: &State, v: Box<Val>) {
	drop(v);
}

/// Set the number of lines of stack trace to display (0 for all of them).
#[no_mangle]
pub extern "C" fn jsonnet_max_trace(vm: &State, v: c_uint) {
	vm.set_max_trace(v as usize)
}

/// Evaluate a file containing Jsonnet code, return a JSON string.
///
/// The returned string should be cleaned up with jsonnet_realloc.
///
/// # Safety
///
/// `filename` should be a \0-terminated string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file(
	vm: &State,
	filename: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	let filename = parse_path(CStr::from_ptr(filename));
	match vm
		.import(&filename)
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
}

/// Evaluate a string containing Jsonnet code, return a JSON string.
///
/// The returned string should be cleaned up with jsonnet_realloc.
///
/// # Safety
///
/// `filename`, `snippet` should be a \0-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet(
	vm: &State,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	let filename = CStr::from_ptr(filename);
	let snippet = CStr::from_ptr(snippet);
	match vm
		.evaluate_snippet(filename.to_str().unwrap(), snippet.to_str().unwrap())
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
	vm: &State,
	filename: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	let filename = parse_path(CStr::from_ptr(filename));
	match vm
		.import(&filename)
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
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet_multi(
	vm: &State,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	let filename = CStr::from_ptr(filename);
	let snippet = CStr::from_ptr(snippet);
	match vm
		.evaluate_snippet(filename.to_str().unwrap(), snippet.to_str().unwrap())
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
	vm: &State,
	filename: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	let filename = parse_path(CStr::from_ptr(filename));
	match vm
		.import(&filename)
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
			CString::new(&out as &str)
				.expect("there should be no \\0 in the error string")
				.into_raw()
		}
	}
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet_stream(
	vm: &State,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> *const c_char {
	let filename = CStr::from_ptr(filename);
	let snippet = CStr::from_ptr(snippet);
	match vm
		.evaluate_snippet(
			filename.to_str().expect("filename is not utf-8"),
			snippet.to_str().expect("snippet is not utf-8"),
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
			CString::new(&out as &str)
				.expect("there should be no \\0 in the error string")
				.into_raw()
		}
	}
}
