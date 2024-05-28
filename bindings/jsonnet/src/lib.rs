#![allow(clippy::box_default)]

pub mod interop;

pub mod import;
pub mod native;
pub mod val_extract;
pub mod val_make;
pub mod val_modify;
pub mod vars_tlas;

use std::{
	alloc::Layout,
	any::Any,
	borrow::Cow,
	cell::RefCell,
	ffi::{c_char, CStr, CString, OsStr},
	os::raw::{c_double, c_int, c_uint},
	path::{Path, PathBuf},
	ptr::{self, null, null_mut},
	sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use jrsonnet_evaluator::{
	apply_tla, bail,
	function::TlaArg,
	gc::{GcHashMap, TraceBox},
	manifest::{JsonFormat, ManifestFormat, ToStringFormat},
	stack::set_stack_depth_limit,
	tb,
	trace::{CompactFormat, PathResolver, TraceFormat},
	FileImportResolver, IStr, ImportResolver, Result, State, Val,
};
use jrsonnet_gcmodule::Trace;
use jrsonnet_parser::SourcePath;

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
	b"v0.20.0\0"
}

unsafe fn parse_path(input: &CStr) -> Cow<Path> {
	#[cfg(target_family = "unix")]
	{
		use std::os::unix::ffi::OsStrExt;
		let str = OsStr::from_bytes(input.to_bytes());
		Cow::Borrowed(Path::new(str))
	}
	#[cfg(not(target_family = "unix"))]
	{
		let string = input.to_str().expect("bad utf-8");
		Cow::Borrowed(string.as_ref())
	}
}

unsafe fn unparse_path(input: &Path) -> Cow<CStr> {
	#[cfg(target_family = "unix")]
	{
		use std::os::unix::ffi::OsStrExt;
		let str = CString::new(input.as_os_str().as_bytes()).expect("input has zero byte in it");
		Cow::Owned(str)
	}
	#[cfg(not(target_family = "unix"))]
	{
		let str = input.as_os_str().to_str().expect("bad utf-8");
		let cstr = CString::new(str).expect("input has NUL inside");
		Cow::Owned(cstr)
	}
}

#[derive(Trace)]
struct VMImportResolver {
	#[trace(tracking(force))]
	inner: RefCell<TraceBox<dyn ImportResolver>>,
}
impl VMImportResolver {
	fn new(value: impl ImportResolver) -> Self {
		Self {
			inner: RefCell::new(tb!(value)),
		}
	}
}
impl ImportResolver for VMImportResolver {
	fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>> {
		self.inner.borrow().load_file_contents(resolved)
	}

	fn resolve_from(&self, from: &SourcePath, path: &str) -> Result<SourcePath> {
		self.inner.borrow().resolve_from(from, path)
	}

	fn resolve_from_default(&self, path: &str) -> Result<SourcePath> {
		self.inner.borrow().resolve_from_default(path)
	}

	fn resolve(&self, path: &Path) -> Result<SourcePath> {
		self.inner.borrow().resolve(path)
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}

/// Same as defined by `ReentrantLock` in stdlib
pub(crate) fn current_thread_unique_ptr() -> usize {
	// Use a non-drop type to make sure it's still available during thread destruction.
	thread_local! { static X: u8 = const { 0 } }
	X.with(|x| ptr::from_ref(x) as usize)
}

pub struct VM {
	state: State,
	manifest_format: Box<dyn ManifestFormat>,
	trace_format: Box<dyn TraceFormat>,
	tla_args: GcHashMap<IStr, TlaArg>,
	thread_state: AtomicPtr<crate::interop::threading::ThreadCTX>,
	// Assuming user can't arbitrarily exit thread (because only
	// primitive available is run_in_thread), we don't need to have lock count,
	// we can just track current owner, and panic if VM is used from
	// multiple threads at the same time.
	entered_in: AtomicUsize,
	max_stack: usize,
}
impl VM {
	fn replace_import_resolver(&self, resolver: impl ImportResolver) {
		*self
			.state
			.import_resolver()
			.as_any()
			.downcast_ref::<VMImportResolver>()
			.expect("valid resolver ty")
			.inner
			.borrow_mut() = tb!(resolver);
	}
	fn add_jpath(&self, path: PathBuf) {
		self.state
			.import_resolver()
			.as_any()
			.downcast_ref::<VMImportResolver>()
			.expect("valid resolver ty")
			.inner
			.borrow_mut()
			.as_any_mut()
			.downcast_mut::<FileImportResolver>()
			.expect("jpaths are not compatible with callback imports!")
			.add_jpath(path);
	}
}

#[repr(transparent)]
pub struct VMRef(*mut VM);
impl VMRef {
	fn destroy(self) {
		use crossbeam::atomic::AtomicConsume;

		let thread_ctx_ptr = unsafe { &(*self.0).thread_state };
		let thread_ctx = thread_ctx_ptr.load_consume();

		assert!(!thread_ctx.is_null(), "can't destroy not finished VM!");

		thread_ctx_ptr.store(null_mut(), Ordering::Relaxed);

		let thread_ctx = AtomicPtr::new(thread_ctx);
		rayon::scope(move |s| {
			s.spawn(move |_| {
				let _stack =
					jrsonnet_evaluator::stack::limit_stack_depth(unsafe { &*self.0 }.max_stack);
				crate::interop::threading::jrsonnet_reenter_thread(thread_ctx);
				let vm: Self = self;
				let _ = unsafe { Box::from_raw(vm.0) };
				jrsonnet_gcmodule::collect_thread_cycles();
			});
		});
	}
	fn run_in_thread<F: FnOnce(&VM) + Send>(&self, call: F) {
		use crossbeam::atomic::AtomicConsume;

		let thread_ctx_ptr = unsafe { &(*self.0).thread_state };
		let thread_ctx = thread_ctx_ptr.load_consume();

		let entered_in_ptr = unsafe { &(*self.0).entered_in };

		if thread_ctx.is_null() {
			println!("reenter");
			let entered_in = entered_in_ptr.load(Ordering::Relaxed);
			assert_eq!(
				entered_in,
				current_thread_unique_ptr(),
				"vm is used from other thread/in non-reentrant way"
			);
			// This thread has already entered correct thread_ctx
			call(unsafe { &*self.0 });
		} else {
			thread_ctx_ptr.store(null_mut(), Ordering::Relaxed);
			println!("root {thread_ctx:?}");
			let thread_ctx = AtomicPtr::new(thread_ctx);
			rayon::scope(|s| {
				s.spawn(|_| {
					let _stack =
						jrsonnet_evaluator::stack::limit_stack_depth(unsafe { &*self.0 }.max_stack);
					entered_in_ptr.store(current_thread_unique_ptr(), Ordering::Relaxed);
					crate::interop::threading::jrsonnet_reenter_thread(thread_ctx);
					println!("body");
					call(unsafe { &*self.0 });
					println!("body end");
					let thread_ctx = unsafe { crate::interop::threading::jrsonnet_exit_thread() };
					thread_ctx_ptr.store(thread_ctx.into_inner(), Ordering::Relaxed);
					entered_in_ptr.store(0, Ordering::Relaxed);
				});
			});
			println!("root end");
		};
	}
	fn run_in_thread_mut<F: FnOnce(&mut VM) + Send>(&self, call: F) {
		use crossbeam::atomic::AtomicConsume;

		let thread_ctx_ptr = unsafe { &(*self.0).thread_state };
		let thread_ctx = thread_ctx_ptr.load_consume();

		if thread_ctx.is_null() {
			panic!("run_in_thread_mut is not reentrant!")
		} else {
			thread_ctx_ptr.store(null_mut(), Ordering::Relaxed);

			let thread_ctx = AtomicPtr::new(thread_ctx);
			rayon::scope(|s| {
				s.spawn(|_| {
					let _stack =
						jrsonnet_evaluator::stack::limit_stack_depth(unsafe { &*self.0 }.max_stack);
					// Not updating entered_in, as we won't allow reentrancy
					// for VM mutation
					crate::interop::threading::jrsonnet_reenter_thread(thread_ctx);
					call(unsafe { &mut *self.0 });
					let thread_ctx = unsafe { crate::interop::threading::jrsonnet_exit_thread() };
					thread_ctx_ptr.store(thread_ctx.into_inner(), Ordering::Relaxed);
				});
			});
		};
	}
}

unsafe impl Send for VMRef {}
unsafe impl Sync for VMRef {}

#[repr(transparent)]
pub struct OwnedVal(*mut Val);

impl OwnedVal {
	fn new(v: Val) -> Self {
		Self(Box::into_raw(Box::new(v)))
	}
	fn invalid() -> Self {
		Self(null_mut())
	}
	fn into_inner(self) -> Box<Val> {
		unsafe { Box::from_raw(self.0) }
	}
}

unsafe impl Send for OwnedVal {}
unsafe impl Sync for OwnedVal {}

#[repr(transparent)]
pub struct ValRef(*mut Val);

impl ValRef {
	fn as_ref(&self) -> &Val {
		unsafe { &*self.0 }
	}
	fn as_mut(&mut self) -> &mut Val {
		unsafe { &mut *self.0 }
	}
}

unsafe impl Send for ValRef {}
unsafe impl Sync for ValRef {}

#[repr(transparent)]
pub struct CStringResult(*mut c_char);

impl CStringResult {
	fn new(v: &str) -> Option<Self> {
		CString::new(v).map_or(None, |res| Some(Self(res.into_raw())))
	}
	fn new_multi(values: Vec<(IStr, IStr)>) -> Option<Self> {
		/* NUL-separation between key and value */
		/* And between KV pairs */
		/* Terminator */
		let mut out = Vec::with_capacity(
			values
				.iter()
				.enumerate()
				.map(|(i, (key, value))| key.len() + 1 + value.len() + usize::from(i != 0))
				.sum::<usize>() + 2,
		);

		for (i, (key, value)) in values.into_iter().enumerate() {
			if i != 0 {
				out.push(0);
			}
			let key = key.as_bytes();
			if key.contains(&0) {
				return None;
			}
			let value = value.as_bytes();
			if value.contains(&0) {
				return None;
			}
			out.extend_from_slice(key);
			out.push(0);
			out.extend_from_slice(value);
		}
		out.push(0);
		out.push(0);
		assert!(out.len() == out.capacity(), "precalculated length");

		let data = out.as_mut_ptr();
		std::mem::forget(out);
		Some(Self(data.cast()))
	}
	fn new_stream(values: Vec<IStr>) -> Option<Self> {
		/* Between values */
		/* Terminator */
		let mut out = Vec::with_capacity(
			values
				.iter()
				.enumerate()
				.map(|(i, value)| value.len() + usize::from(i != 0))
				.sum::<usize>() + 2,
		);

		for (i, value) in values.into_iter().enumerate() {
			if i != 0 {
				out.push(0);
			}
			let value = value.as_bytes();
			if value.contains(&0) {
				return None;
			}
			out.extend_from_slice(value);
		}
		out.push(0);
		out.push(0);
		assert!(out.len() == out.capacity(), "precalculated length");

		let data = out.as_mut_ptr();
		std::mem::forget(out);
		Some(Self(data.cast()))
	}
	fn invalid() -> Self {
		Self(null_mut())
	}
	fn invalid_error() -> Self {
		Self::new("error: resulting string contained internal NUL")
			.expect("hardcoded message is valid")
	}
	fn is_null(&self) -> bool {
		self.0.is_null()
	}
}

unsafe impl Send for CStringResult {}
unsafe impl Sync for CStringResult {}

/// Creates a new Jsonnet virtual machine.
#[no_mangle]
#[allow(clippy::box_default)]
pub extern "C" fn jsonnet_make() -> VMRef {
	let mut out = VMRef(null_mut());
	rayon::scope(|s| {
		s.spawn(|_| {
			let mut state = State::builder();
			state
				.import_resolver(VMImportResolver::new(FileImportResolver::default()))
				.context_initializer(jrsonnet_stdlib::ContextInitializer::new(
					PathResolver::new_cwd_fallback(),
				));
			let state = state.build();

			out = VMRef(Box::into_raw(Box::new(VM {
				state,
				manifest_format: Box::new(JsonFormat::default()),
				trace_format: Box::new(CompactFormat::default()),
				tla_args: GcHashMap::new(),
				thread_state: unsafe { crate::interop::threading::jrsonnet_exit_thread() },
				entered_in: AtomicUsize::new(0),
				max_stack: 200,
			})));
		});
	});
	out
}

/// Complement of [`jsonnet_vm_make`].
#[no_mangle]
#[allow(clippy::boxed_local)]
pub extern "C" fn jsonnet_destroy(vm: VMRef) {
	vm.destroy();
}

/// Set the maximum stack depth.
#[no_mangle]
pub extern "C" fn jsonnet_max_stack(vm: VMRef, v: c_uint) {
	vm.run_in_thread_mut(|vm| vm.max_stack = v as usize);
}

/// Set the number of objects required before a garbage collection cycle is allowed.
///
/// No-op for now
#[no_mangle]
pub extern "C" fn jsonnet_gc_min_objects(_vm: &VM, _v: c_uint) {}

/// Run the garbage collector after this amount of growth in the number of objects
///
/// No-op for now
#[no_mangle]
pub extern "C" fn jsonnet_gc_growth_trigger(_vm: &VM, _v: c_double) {}

/// Expect a string as output and don't JSON encode it.
#[no_mangle]
pub extern "C" fn jsonnet_string_output(vm: VMRef, v: c_int) {
	vm.run_in_thread_mut(|vm| {
		vm.manifest_format = match v {
			0 => Box::new(JsonFormat::default()),
			1 => Box::new(ToStringFormat),
			_ => panic!("incorrect output format"),
		};
	});
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
pub unsafe extern "C" fn jsonnet_realloc(_vm: &VM, buf: *mut u8, sz: usize) -> *mut u8 {
	if buf.is_null() {
		if sz == 0 {
			return ptr::null_mut();
		}
		return unsafe {
			std::alloc::alloc(Layout::from_size_align(sz, std::mem::align_of::<u8>()).unwrap())
		};
	}
	// TODO: Somehow store size of allocation, because its real size is probally not 16 :D
	// OR (Alternative way of fixing this TODO)
	// TODO: Standard allocator uses malloc, and it doesn't uses allocation size,
	// TODO: so it should work in normal cases. Maybe force allocator for this library?
	let old_layout = Layout::from_size_align(16, std::mem::align_of::<u8>()).unwrap();
	if sz == 0 {
		unsafe { std::alloc::dealloc(buf, old_layout) };
		return ptr::null_mut();
	}
	unsafe { std::alloc::realloc(buf, old_layout, sz) }
}

/// Clean up a JSON subtree.
///
/// This is useful if you want to abort with an error mid-way through building a complex value.
#[no_mangle]
#[allow(clippy::boxed_local)]
pub extern "C" fn jsonnet_json_destroy(vm: VMRef, v: OwnedVal) {
	vm.run_in_thread(move |_| drop(v.into_inner()));
}

/// Set the number of lines of stack trace to display (0 for all of them).
#[no_mangle]
pub extern "C" fn jsonnet_max_trace(vm: VMRef, v: c_uint) {
	vm.run_in_thread_mut(|vm| {
		if let Some(format) = vm.trace_format.as_any_mut().downcast_mut::<CompactFormat>() {
			format.max_trace = v as usize;
		}
	});
}

/// Evaluate a file containing Jsonnet code, return a JSON string.
///
/// The returned string should be cleaned up with `jsonnet_realloc`.
///
/// # Safety
///
/// `filename` should be a NUL-terminated string
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file(
	vm: VMRef,
	filename: *const c_char,
	error: &mut c_int,
) -> CStringResult {
	let filename = unsafe { parse_path(CStr::from_ptr(filename)) };
	let mut result = CStringResult::invalid();
	vm.run_in_thread(|vm| {
		result = match vm
			.state
			.import(filename)
			.and_then(|val| apply_tla(vm.state.clone(), &vm.tla_args, val))
			.and_then(|val| val.manifest(&vm.manifest_format))
		{
			Ok(v) => {
				*error = 0;
				CStringResult::new(v.as_str()).map_or_else(
					|| {
						*error = 1;
						CStringResult::invalid_error()
					},
					|res| res,
				)
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				vm.trace_format.write_trace(&mut out, &e).unwrap();
				CStringResult::new(out.as_str()).unwrap_or_else(CStringResult::invalid_error)
			}
		};
	});
	result
}

/// Evaluate a string containing Jsonnet code, return a JSON string.
///
/// The returned string should be cleaned up with `jsonnet_realloc`.
///
/// # Safety
///
/// `filename`, `snippet` should be a NUL-terminated strings
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet(
	vm: VMRef,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> CStringResult {
	let filename = unsafe { CStr::from_ptr(filename) };
	let snippet = unsafe { CStr::from_ptr(snippet) };
	let mut out = CStringResult::invalid();
	vm.run_in_thread(|vm| {
		out = match vm
			.state
			.evaluate_snippet(filename.to_str().unwrap(), snippet.to_str().unwrap())
			.and_then(|val| apply_tla(vm.state.clone(), &vm.tla_args, val))
			.and_then(|val| val.manifest(&*vm.manifest_format))
		{
			Ok(v) => {
				*error = 0;
				CStringResult::new(v.as_str()).map_or_else(
					|| {
						*error = 1;
						CStringResult::invalid_error()
					},
					|res| res,
				)
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				vm.trace_format.write_trace(&mut out, &e).unwrap();
				CStringResult::new(out.as_str()).unwrap_or_else(CStringResult::invalid_error)
			}
		}
	});
	out
}

fn val_to_multi(val: Val, format: &dyn ManifestFormat) -> Result<Vec<(IStr, IStr)>> {
	let Val::Obj(val) = val else {
		bail!("expected object as multi output")
	};
	let mut out = Vec::new();
	for (k, v) in val.iter(
		#[cfg(feature = "exp-preserve-order")]
		false,
	) {
		out.push((k, v?.manifest(format)?.into()));
	}
	Ok(out)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file_multi(
	vm: VMRef,
	filename: *const c_char,
	error: &mut c_int,
) -> CStringResult {
	let filename = unsafe { parse_path(CStr::from_ptr(filename)) };
	let mut out = CStringResult::invalid();
	vm.run_in_thread(|vm| {
		out = match vm
			.state
			.import(filename)
			.and_then(|val| apply_tla(vm.state.clone(), &vm.tla_args, val))
			.and_then(|val| val_to_multi(val, &vm.manifest_format))
		{
			Ok(v) => {
				*error = 0;
				CStringResult::new_multi(v).map_or_else(
					|| {
						*error = 1;
						CStringResult::invalid_error()
					},
					|v| v,
				)
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				vm.trace_format.write_trace(&mut out, &e).unwrap();
				CStringResult::new(out.as_str()).unwrap_or_else(CStringResult::invalid_error)
			}
		}
	});
	out
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet_multi(
	vm: VMRef,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> CStringResult {
	let filename = unsafe { CStr::from_ptr(filename) };
	let snippet = unsafe { CStr::from_ptr(snippet) };
	let mut out = CStringResult::invalid();

	vm.run_in_thread(|vm| {
		out = match vm
			.state
			.evaluate_snippet(filename.to_str().unwrap(), snippet.to_str().unwrap())
			.and_then(|val| apply_tla(vm.state.clone(), &vm.tla_args, val))
			.and_then(|val| val_to_multi(val, &vm.manifest_format))
		{
			Ok(v) => {
				*error = 0;
				CStringResult::new_multi(v).map_or_else(
					|| {
						*error = 1;
						CStringResult::invalid_error()
					},
					|v| v,
				)
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				vm.trace_format.write_trace(&mut out, &e).unwrap();
				CStringResult::new(out.as_str()).unwrap_or_else(CStringResult::invalid_error)
			}
		};
	});
	out
}

fn val_to_stream(val: Val, format: &dyn ManifestFormat) -> Result<Vec<IStr>> {
	let Val::Arr(val) = val else {
		bail!("expected array as stream output")
	};
	let mut out = Vec::new();
	for item in val.iter() {
		out.push(item?.manifest(format)?.into());
	}
	Ok(out)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_file_stream(
	vm: VMRef,
	filename: *const c_char,
	error: &mut c_int,
) -> CStringResult {
	let filename = unsafe { parse_path(CStr::from_ptr(filename)) };

	let mut out = CStringResult::invalid();
	vm.run_in_thread(|vm| {
		out = match vm
			.state
			.import(filename)
			.and_then(|val| apply_tla(vm.state.clone(), &vm.tla_args, val))
			.and_then(|val| val_to_stream(val, &vm.manifest_format))
		{
			Ok(v) => {
				*error = 0;
				CStringResult::new_stream(v).map_or_else(
					|| {
						*error = 1;
						CStringResult::invalid_error()
					},
					|v| v,
				)
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				vm.trace_format.write_trace(&mut out, &e).unwrap();
				CStringResult::new(out.as_str()).unwrap_or_else(CStringResult::invalid_error)
			}
		}
	});
	out
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn jsonnet_evaluate_snippet_stream(
	vm: VMRef,
	filename: *const c_char,
	snippet: *const c_char,
	error: &mut c_int,
) -> CStringResult {
	let filename = unsafe { CStr::from_ptr(filename) };
	let snippet = unsafe { CStr::from_ptr(snippet) };

	let mut out = CStringResult::invalid();
	vm.run_in_thread(|vm| {
		out = match vm
			.state
			.evaluate_snippet(filename.to_str().unwrap(), snippet.to_str().unwrap())
			.and_then(|val| apply_tla(vm.state.clone(), &vm.tla_args, val))
			.and_then(|val| val_to_stream(val, &vm.manifest_format))
		{
			Ok(v) => {
				*error = 0;
				CStringResult::new_stream(v).map_or_else(
					|| {
						*error = 1;
						CStringResult::invalid_error()
					},
					|v| v,
				)
			}
			Err(e) => {
				*error = 1;
				let mut out = String::new();
				vm.trace_format.write_trace(&mut out, &e).unwrap();
				CStringResult::new(out.as_str()).unwrap_or_else(CStringResult::invalid_error)
			}
		}
	});
	out
}
