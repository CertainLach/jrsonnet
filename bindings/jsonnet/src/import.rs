//! Import resolution manipulation utilities

use std::{
	any::Any,
	cell::RefCell,
	collections::HashMap,
	ffi::{c_void, CStr, CString},
	fs::File,
	io::Read,
	os::raw::{c_char, c_int},
	path::{Path, PathBuf},
	ptr::null_mut,
};

use jrsonnet_evaluator::{
	error::{Error::*, Result},
	throw, ImportResolver, State,
};

pub type JsonnetImportCallback = unsafe extern "C" fn(
	ctx: *mut c_void,
	base: *const c_char,
	rel: *const c_char,
	found_here: *mut *const c_char,
	success: &mut c_int,
) -> *mut c_char;

/// Resolves imports using callback
pub struct CallbackImportResolver {
	cb: JsonnetImportCallback,
	ctx: *mut c_void,
	out: RefCell<HashMap<PathBuf, Vec<u8>>>,
}
impl ImportResolver for CallbackImportResolver {
	fn resolve_file(&self, from: &Path, path: &str) -> Result<PathBuf> {
		let base = CString::new(from.to_str().unwrap()).unwrap().into_raw();
		let rel = CString::new(path).unwrap().into_raw();
		let found_here: *mut c_char = null_mut();
		let mut success: i32 = 0;
		let result_ptr = unsafe {
			(self.cb)(
				self.ctx,
				base,
				rel,
				&mut (found_here as *const _),
				&mut success,
			)
		};
		// Release memory occipied by arguments passed
		unsafe {
			let _ = CString::from_raw(base);
			let _ = CString::from_raw(rel);
		}
		let result_raw = unsafe { CStr::from_ptr(result_ptr) };
		let result_str = result_raw.to_str().unwrap();
		assert!(success == 0 || success == 1);
		if success == 0 {
			unsafe { CString::from_raw(result_ptr) };
			let result = result_str.to_owned();
			throw!(ImportCallbackError(result));
		}

		let found_here_raw = unsafe { CStr::from_ptr(found_here) };
		let found_here_buf = PathBuf::from(found_here_raw.to_str().unwrap());
		unsafe {
			let _ = CString::from_raw(found_here);
		}

		let mut out = self.out.borrow_mut();
		if !out.contains_key(&found_here_buf) {
			out.insert(found_here_buf.clone(), result_str.into());
			unsafe { CString::from_raw(result_ptr) };
		}

		Ok(found_here_buf)
	}
	fn load_file_contents(&self, resolved: &Path) -> Result<Vec<u8>> {
		Ok(self.out.borrow().get(resolved).unwrap().clone())
	}

	unsafe fn as_any(&self) -> &dyn Any {
		self
	}
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_import_callback(
	vm: &State,
	cb: JsonnetImportCallback,
	ctx: *mut c_void,
) {
	vm.set_import_resolver(Box::new(CallbackImportResolver {
		cb,
		ctx,
		out: RefCell::new(HashMap::new()),
	}))
}

/// Standard FS import resolver
#[derive(Default)]
pub struct NativeImportResolver {
	library_paths: RefCell<Vec<PathBuf>>,
}
impl NativeImportResolver {
	fn add_jpath(&self, path: PathBuf) {
		self.library_paths.borrow_mut().push(path);
	}
}
impl ImportResolver for NativeImportResolver {
	fn resolve_file(&self, from: &Path, path: &str) -> Result<PathBuf> {
		let mut new_path = from.to_owned();
		new_path.push(path);
		if new_path.exists() {
			Ok(new_path)
		} else {
			for library_path in self.library_paths.borrow().iter() {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(cloned);
				}
			}
			throw!(ImportFileNotFound(from.to_owned(), path.to_owned()))
		}
	}
	fn load_file_contents(&self, id: &Path) -> Result<Vec<u8>> {
		let mut file = File::open(id).map_err(|_e| ResolvedFileNotFound(id.to_owned()))?;
		let mut out = Vec::new();
		file.read_to_end(&mut out)
			.map_err(|e| ImportIo(e.to_string()))?;
		Ok(out)
	}
	unsafe fn as_any(&self) -> &dyn Any {
		self
	}
}

/// # Safety
///
/// This function is safe, if received v is a pointer to normal C string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_jpath_add(vm: &State, v: *const c_char) {
	let cstr = CStr::from_ptr(v);
	let path = PathBuf::from(cstr.to_str().unwrap());
	let any_resolver = vm.import_resolver();
	let resolver = any_resolver
		.as_any()
		.downcast_ref::<NativeImportResolver>()
		.expect("jpaths are not compatible with callback imports!");
	resolver.add_jpath(path);
}
