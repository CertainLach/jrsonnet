//! Jrsonnet specific additional binding helpers

#[cfg(feature = "interop-wasm")]
pub mod wasm {
	use std::ffi::{c_char, c_int, c_void};

	use jrsonnet_evaluator::Val;

	use crate::VM;

	extern "C" {

		pub fn _jrsonnet_static_import_callback(
			ctx: *mut c_void,
			base: *const c_char,
			rel: *const c_char,
			found_here: *mut *const c_char,
			buf: *mut *mut c_char,
			buflen: *mut usize,
		) -> c_int;

		#[allow(improper_ctypes)]
		pub fn _jrsonnet_static_native_callback(
			ctx: *const c_void,
			argv: *const *const Val,
			success: *mut c_int,
		) -> *mut Val;
	}

	#[no_mangle]
	#[cfg(feature = "interop-wasm")]
	// ctx arg is passed as-is to callback
	#[allow(clippy::not_unsafe_ptr_arg_deref)]
	pub extern "C" fn jrsonnet_apply_static_import_callback(vm: &VM, ctx: *mut c_void) {
		unsafe { crate::import::jsonnet_import_callback(vm, _jrsonnet_static_import_callback, ctx) }
	}

	/// # Safety
	///
	/// `name` and `raw_params` should be correctly initialized
	#[no_mangle]
	#[cfg(feature = "interop-wasm")]
	pub unsafe extern "C" fn jrsonnet_apply_static_native_callback(
		vm: &VM,
		name: *const c_char,
		ctx: *mut c_void,
		raw_params: *const *const c_char,
	) {
		unsafe {
			crate::native::jsonnet_native_callback(
				vm,
				name,
				_jrsonnet_static_native_callback,
				ctx,
				raw_params,
			);
		}
	}
}

#[cfg(feature = "interop-common")]
mod common {
	use jrsonnet_evaluator::trace::{CompactFormat, HiDocFormat, JsFormat, PathResolver};

	use crate::VM;

	#[no_mangle]
	pub extern "C" fn jrsonnet_set_trace_format(vm: &mut VM, format: u8) {
		match format {
			0 => {
				vm.trace_format = Box::new(CompactFormat {
					max_trace: 20,
					resolver: PathResolver::new_cwd_fallback(),
					padding: 4,
				});
			}
			1 => vm.trace_format = Box::new(JsFormat { max_trace: 20 }),
			2 => {
				vm.trace_format = Box::new(HiDocFormat {
					resolver: PathResolver::new_cwd_fallback(),
					max_trace: 20,
				});
			}
			_ => panic!("unknown trace format"),
		}
	}
}

#[cfg(feature = "interop-threading")]
mod threading {
	use std::{ffi::c_int, thread::ThreadId};

	pub struct ThreadCTX {
		interner: *mut jrsonnet_interner::interop::PoolState,
		gc: *mut jrsonnet_gcmodule::interop::GcState,
	}

	/// Golang jrsonnet bindings require Jsonnet VM to be movable.
	/// Jrsonnet uses `thread_local` in some places, thus making VM
	/// immovable by default. By using `jrsonnet_exit_thread` and
	/// `jrsonnet_reenter_thread`, you can move `thread_local` state to
	/// where it is more convinient to use it.
	///
	/// # Safety
	///
	/// Current thread GC will be broken after this call, need to call
	/// `jrsonet_enter_thread` before doing anything.
	#[no_mangle]
	pub unsafe extern "C" fn jrsonnet_exit_thread() -> *mut ThreadCTX {
		Box::into_raw(Box::new(ThreadCTX {
			interner: jrsonnet_interner::interop::exit_thread(),
			gc: unsafe { jrsonnet_gcmodule::interop::exit_thread() },
		}))
	}

	#[no_mangle]
	pub extern "C" fn jrsonnet_reenter_thread(mut ctx: Box<ThreadCTX>) {
		use std::ptr::null_mut;
		assert!(
			!ctx.interner.is_null() && !ctx.gc.is_null(),
			"reused context?"
		);
		unsafe { jrsonnet_interner::interop::reenter_thread(ctx.interner) }
		unsafe { jrsonnet_gcmodule::interop::reenter_thread(ctx.gc) }
		// Just in case
		ctx.interner = null_mut();
		ctx.gc = null_mut();
	}

	// ThreadId is compatible with u64, and there is unstable cast
	// method... But until it is stabilized, lets erase its type by
	// boxing.
	pub enum JrThreadId {}

	#[no_mangle]
	pub extern "C" fn jrsonnet_thread_id() -> *mut JrThreadId {
		Box::into_raw(Box::new(std::thread::current().id())).cast()
	}

	#[no_mangle]
	pub extern "C" fn jrsonnet_thread_id_compare(
		a: *const JrThreadId,
		b: *const JrThreadId,
	) -> c_int {
		let a: &ThreadId = unsafe { *a.cast() };
		let b: &ThreadId = unsafe { *b.cast() };
		i32::from(*a == *b)
	}

	#[no_mangle]
	pub unsafe extern "C" fn jrsonnet_thread_id_free(id: *mut JrThreadId) {
		let _id: Box<ThreadId> = unsafe { Box::from_raw(id.cast()) };
	}
}
