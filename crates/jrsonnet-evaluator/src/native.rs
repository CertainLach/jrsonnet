#![allow(clippy::type_complexity)]

use crate::{error::Result, Val};
use gc::{Finalize, Trace};
use jrsonnet_parser::ParamsDesc;
use std::fmt::Debug;
use std::path::Path;
use std::rc::Rc;

pub trait NativeCallbackHandler: Trace {
	fn call(&self, from: Option<Rc<Path>>, args: &[Val]) -> Result<Val>;
}

#[derive(Trace, Finalize)]
pub struct NativeCallback {
	pub params: ParamsDesc,
	handler: Box<dyn NativeCallbackHandler>,
}
impl NativeCallback {
	pub fn new(params: ParamsDesc, handler: Box<dyn NativeCallbackHandler>) -> Self {
		Self { params, handler }
	}
	pub fn call(&self, caller: Option<Rc<Path>>, args: &[Val]) -> Result<Val> {
		self.handler.call(caller, args)
	}
}
impl Debug for NativeCallback {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NativeCallback").finish()
	}
}
