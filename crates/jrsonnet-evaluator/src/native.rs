#![allow(clippy::type_complexity)]

use crate::gc::TraceBox;
use crate::{error::Result, Val};
use gcmodule::Trace;
use jrsonnet_parser::ParamsDesc;
use std::fmt::Debug;
use std::path::Path;
use std::rc::Rc;

#[deprecated(note = "Use builtins instead")]
pub trait NativeCallbackHandler: Trace {
	fn call(&self, from: Rc<Path>, args: &[Val]) -> Result<Val>;
}

#[derive(Trace)]
pub struct NativeCallback {
	pub params: ParamsDesc,
	handler: TraceBox<dyn NativeCallbackHandler>,
}
impl NativeCallback {
	pub fn new(params: ParamsDesc, handler: TraceBox<dyn NativeCallbackHandler>) -> Self {
		Self { params, handler }
	}
	pub fn call(&self, caller: Rc<Path>, args: &[Val]) -> Result<Val> {
		self.handler.call(caller, args)
	}
}
impl Debug for NativeCallback {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NativeCallback").finish()
	}
}
