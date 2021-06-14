#![allow(clippy::type_complexity)]

use crate::{error::Result, Val};
use jrsonnet_parser::ParamsDesc;
use std::fmt::Debug;
use std::path::Path;
use std::rc::Rc;

pub struct NativeCallback {
	pub params: ParamsDesc,
	handler: Box<dyn Fn(Option<Rc<Path>>, &[Val]) -> Result<Val>>,
}
impl NativeCallback {
	pub fn new(
		params: ParamsDesc,
		handler: impl Fn(Option<Rc<Path>>, &[Val]) -> Result<Val> + 'static,
	) -> Self {
		Self {
			params,
			handler: Box::new(handler),
		}
	}
	pub fn call(&self, caller: Option<Rc<Path>>, args: &[Val]) -> Result<Val> {
		(self.handler)(caller, args)
	}
}
impl Debug for NativeCallback {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NativeCallback").finish()
	}
}
