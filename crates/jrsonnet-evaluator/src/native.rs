#![allow(clippy::type_complexity)]

use crate::function::{parse_builtin_call, ArgsLike, Builtin, BuiltinParam};
use crate::gc::TraceBox;
use crate::Context;
use crate::{error::Result, Val};
use gcmodule::Trace;
use jrsonnet_parser::ExprLocation;
use std::path::Path;
use std::rc::Rc;

#[derive(Trace)]
pub struct NativeCallback {
	pub(crate) params: Vec<BuiltinParam>,
	handler: TraceBox<dyn NativeCallbackHandler>,
}
impl NativeCallback {
	#[deprecated = "prefer using builtins directly, use this interface only for bindings"]
	pub fn new(params: Vec<BuiltinParam>, handler: TraceBox<dyn NativeCallbackHandler>) -> Self {
		Self { params, handler }
	}
}

impl Builtin for NativeCallback {
	fn name(&self) -> &str {
		// TODO: standard natives gets their names from definition
		// But builitins should already have them
		"<native>"
	}

	fn params(&self) -> &[BuiltinParam] {
		&self.params
	}

	fn call(
		&self,
		context: Context,
		loc: Option<&ExprLocation>,
		args: &dyn ArgsLike,
	) -> Result<Val> {
		let args = parse_builtin_call(context, &self.params, args, true)?;
		let mut out_args = Vec::with_capacity(self.params.len());
		for p in self.params.iter() {
			out_args.push(args[&p.name].evaluate()?);
		}
		self.handler.call(loc.map(|l| l.0.clone()), &out_args)
	}
}

pub trait NativeCallbackHandler: Trace {
	fn call(&self, from: Option<Rc<Path>>, args: &[Val]) -> Result<Val>;
}
