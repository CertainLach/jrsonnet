//! Import resolution manipulation utilities

use std::{
	alloc::Layout,
	any::Any,
	cell::RefCell,
	collections::HashMap,
	env::current_dir,
	ffi::{c_void, CStr, CString},
	os::raw::{c_char, c_int},
	path::PathBuf,
	ptr::null_mut,
};

use jrsonnet_evaluator::{
	bail,
	error::{ErrorKind::*, Result},
	FileImportResolver, ImportResolver,
};
use jrsonnet_gcmodule::Trace;
use jrsonnet_parser::{SourceDirectory, SourceFile, SourcePath};

use crate::VM;

pub type JsonnetImportCallback = unsafe extern "C" fn(
	ctx: *mut c_void,
	base: *const c_char,
	rel: *const c_char,
	found_here: *mut *const c_char,
	buf: *mut *mut c_char,
	buflen: *mut usize,
) -> c_int;

/// Resolves imports using callback
#[derive(Trace)]
pub struct CallbackImportResolver {
	#[trace(skip)]
	cb: JsonnetImportCallback,
	#[trace(skip)]
	ctx: *mut c_void,
	out: RefCell<HashMap<SourcePath, Vec<u8>>>,
}
impl ImportResolver for CallbackImportResolver {
	fn resolve_from(&self, from: &SourcePath, path: &str) -> Result<SourcePath> {
		let base = if let Some(p) = from.downcast_ref::<SourceFile>() {
			let mut o = p.path().to_owned();
			o.pop();
			o
		} else if let Some(d) = from.downcast_ref::<SourceDirectory>() {
			d.path().to_owned()
		} else if from.is_default() {
			current_dir().map_err(|e| ImportIo(e.to_string()))?
		} else {
			unreachable!("can't resolve this path");
		};
		let base = unsafe { crate::unparse_path(&base) };
		let rel = CString::new(path).unwrap();
		let found_here: *mut c_char = null_mut();

		let mut buf = null_mut();
		let mut buf_len = 0;
		let success = unsafe {
			(self.cb)(
				self.ctx,
				base.as_ptr(),
				rel.as_ptr(),
				&mut found_here.cast_const(),
				&mut buf,
				&mut buf_len,
			)
		};
		let buf_slice: &[u8] = unsafe { std::slice::from_raw_parts(buf.cast(), buf_len) };
		unsafe {
			std::alloc::dealloc(
				buf.cast(),
				Layout::from_size_align(buf_len, 1).expect("layout is valid"),
			);
		};
		let buf_intern = buf_slice.to_vec();

		assert!(success == 0 || success == 1);
		if success == 0 {
			let result = String::from_utf8(buf_intern).expect("error should be valid string");
			bail!(ImportCallbackError(result));
		}

		let found_here_raw = unsafe { CStr::from_ptr(found_here) };
		let found_here_buf = SourcePath::new(SourceFile::new(PathBuf::from(
			found_here_raw.to_str().unwrap(),
		)));
		unsafe {
			let _ = CString::from_raw(found_here);
		}

		let mut out = self.out.borrow_mut();
		if !out.contains_key(&found_here_buf) {
			out.insert(found_here_buf.clone(), buf_intern);
		}

		Ok(found_here_buf)
	}
	fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>> {
		Ok(self.out.borrow().get(resolved).unwrap().clone())
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

/// # Safety
///
/// It should be safe to call `cb` using valid values with passed `ctx`
#[no_mangle]
pub unsafe extern "C" fn jsonnet_import_callback(
	vm: &VM,
	cb: JsonnetImportCallback,
	ctx: *mut c_void,
) {
	vm.state.set_import_resolver(CallbackImportResolver {
		cb,
		ctx,
		out: RefCell::new(HashMap::new()),
	});
}

/// # Safety
///
/// `path` should be a NUL-terminated string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_jpath_add(vm: &VM, path: *const c_char) {
	let cstr = unsafe { CStr::from_ptr(path) };
	let path = PathBuf::from(cstr.to_str().unwrap());
	let any_resolver = vm.state.import_resolver();
	let resolver = any_resolver
		.as_any()
		.downcast_ref::<FileImportResolver>()
		.expect("jpaths are not compatible with callback imports!");
	resolver.add_jpath(path);
}
