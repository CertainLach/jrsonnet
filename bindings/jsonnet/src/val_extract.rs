//! Extract values from VM

use std::os::raw::{c_double, c_int};

use jrsonnet_evaluator::Val;

use crate::{CStringResult, VMRef, ValRef};

/// If the value is a string, return it as UTF-8, otherwise return `NULL`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_string(vm: VMRef, v: ValRef) -> CStringResult {
	let mut out = CStringResult::invalid();
	vm.run_in_thread(|_| {
		out = match v.as_ref() {
			Val::Str(s) => {
				let s = s.clone().into_flat();
				CStringResult::new(s.as_str()).unwrap_or_else(CStringResult::invalid)
			}
			_ => CStringResult::invalid(),
		}
	});
	out
}

/// If the value is a number, return `1` and store the number in out, otherwise return `0`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_number(vm: VMRef, v: ValRef, out: &mut c_double) -> c_int {
	let mut res = 0;
	vm.run_in_thread(|_| {
		res = match v.as_ref() {
			Val::Num(n) => {
				*out = n.get();
				1
			}
			_ => 0,
		}
	});
	res
}

/// Return `0` if the value is `false`, `1` if it is `true`, and `2` if it is not a `bool`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_bool(vm: VMRef, v: ValRef) -> c_int {
	let mut out = 2;
	vm.run_in_thread(|_| {
		out = match v.as_ref() {
			Val::Bool(false) => 0,
			Val::Bool(true) => 1,
			_ => 2,
		}
	});
	out
}

/// Return `1` if the value is `null`, otherwise return `0`.
#[no_mangle]
pub extern "C" fn jsonnet_json_extract_null(vm: VMRef, v: ValRef) -> c_int {
	let mut out = 0;
	vm.run_in_thread(|_| {
		out = match v.as_ref() {
			Val::Null => 1,
			_ => 0,
		}
	});
	out
}
