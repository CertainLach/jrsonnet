use jrsonnet_evaluator::{
	create_error, create_error_result, Error, EvaluationState, ImportResolver, LazyBinding,
	LazyVal, ObjMember, ObjValue, Result, Val,
};
use jrsonnet_parser::Visibility;
use libc::{c_char, c_double, c_int, c_uint};
use std::{
	alloc::Layout,
	any::Any,
	cell::RefCell,
	collections::HashMap,
	ffi::{CStr, CString},
	fs::File,
	io::Read,
	path::PathBuf,
	rc::Rc,
};

#[no_mangle]
#[cfg(target = "wasm32-wasi")]
pub extern "C" fn _start() {}

#[no_mangle]
pub extern "C" fn jsonnet_version() -> &'static [u8; 8] {
	b"v0.16.0\0"
}

#[derive(Default)]
struct NativeImportResolver {
	library_paths: RefCell<Vec<PathBuf>>,
}
impl NativeImportResolver {
	fn add_jpath(&self, path: PathBuf) {
		self.library_paths.borrow_mut().push(path);
	}
}
impl ImportResolver for NativeImportResolver {
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>> {
		let mut new_path = from.clone();
		new_path.push(path);
		if new_path.exists() {
			Ok(Rc::new(new_path))
		} else {
			for library_path in self.library_paths.borrow().iter() {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(Rc::new(cloned));
				}
			}
			create_error_result(Error::ImportFileNotFound(from.clone(), path.clone()))
		}
	}
	fn load_file_contents(&self, id: &PathBuf) -> Result<Rc<str>> {
		let mut file =
			File::open(id).map_err(|_e| create_error(Error::ResolvedFileNotFound(id.clone())))?;
		let mut out = String::new();
		file.read_to_string(&mut out)
			.map_err(|_e| create_error(Error::ImportBadFileUtf8(id.clone())))?;
		Ok(out.into())
	}
	unsafe fn as_any(&self) -> &dyn Any {
		self
	}
}

#[no_mangle]
pub extern "C" fn jsonnet_make() -> Box<EvaluationState> {
	let state = EvaluationState::default();
	state.with_stdlib();
	state.set_import_resolver(Box::new(NativeImportResolver::default()));
	Box::new(state)
}

#[no_mangle]
pub extern "C" fn jsonnet_max_stack(vm: &EvaluationState, v: c_uint) {
	vm.set_max_stack(v as usize);
}

// jrsonnet currently have no GC, so these functions is no-op
#[no_mangle]
pub extern "C" fn jsonnet_gc_min_objects(_vm: &EvaluationState, _v: c_uint) {}
#[no_mangle]
pub extern "C" fn jsonnet_gc_growth_trigger(_vm: &EvaluationState, _v: c_double) {}

// TODO
#[no_mangle]
pub extern "C" fn jsonnet_string_output(_vm: &EvaluationState, _v: c_int) {
	todo!()
}

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
			// TODO: Mutate array, instead of recreating them
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

/// # Safety
///
/// This function is safe if passed name is ok
#[no_mangle]
pub unsafe extern "C" fn jsonnet_json_object_append(
	_vm: &EvaluationState,
	obj: &mut Val,
	name: *const c_char,
	val: &Val,
) {
	match obj {
		Val::Obj(old) => {
			let mut new = HashMap::new();
			new.insert(
				CStr::from_ptr(name).to_str().unwrap().into(),
				ObjMember {
					add: false,
					visibility: Visibility::Normal,
					invoke: LazyBinding::Bound(LazyVal::new_resolved(val.clone())),
				},
			);
			let new_obj = ObjValue::new(Some(old.clone()), Rc::new(new));
			*obj = Val::Obj(new_obj);
		}
		_ => panic!("should receive array"),
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

/// # Safety
///
/// This function is safe, if received v is a pointer to normal C string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_jpath_add(vm: &EvaluationState, v: *const c_char) {
	let cstr = CStr::from_ptr(v);
	let path = PathBuf::from(cstr.to_str().unwrap());
	let any_resolver = vm.import_resolver();
	let resolver = any_resolver
		.as_any()
		.downcast_ref::<NativeImportResolver>()
		.unwrap();
	resolver.add_jpath(path);
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
					writeln!(out, "{:?}", i.0).unwrap();
				}
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
		use std::fmt::Write;
		let filename = CStr::from_ptr(filename);
		let snippet = CStr::from_ptr(snippet);
		match vm.evaluate_snippet_to_json(
			&PathBuf::from(filename.to_str().unwrap()),
			&snippet.to_str().unwrap(),
		) {
			Ok(v) => {
				*error = 0;
				CString::new(&*v as &str).unwrap().into_raw()
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				writeln!(out, "{:?}", e.0).unwrap();
				for i in (e.1).0.iter() {
					writeln!(out, "{:?} ---- {}", i.0, i.1).unwrap();
				}
				CString::new(&out as &str).unwrap().into_raw()
			}
		}
	})
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
