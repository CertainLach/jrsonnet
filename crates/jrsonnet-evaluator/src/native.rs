use crate::{error::Result, Val};
use jrsonnet_parser::ParamsDesc;
use std::fmt::Debug;

pub struct NativeCallback {
	pub params: ParamsDesc,
	handler: Box<dyn Fn(&[Val]) -> Result<Val>>,
}
impl NativeCallback {
	pub fn new(params: ParamsDesc, handler: impl Fn(&[Val]) -> Result<Val> + 'static) -> Self {
		Self {
			params,
			handler: Box::new(handler),
		}
	}
	pub fn call(&self, args: &[Val]) -> Result<Val> {
		(self.handler)(args)
	}
}
impl Debug for NativeCallback {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NativeCallback").finish()
	}
}
