use std::borrow::Cow;

use jrsonnet_gcmodule::Trace;

use super::{arglike::ArgsLike, parse::parse_builtin_call, CallLocation};
use crate::{error::Result, gc::TraceBox, Context, State, Val};

pub type BuiltinParamName = Cow<'static, str>;

#[derive(Clone, Trace)]
pub struct BuiltinParam {
	pub name: BuiltinParamName,
	pub has_default: bool,
}

/// Do not implement it directly, instead use #[builtin] macro
pub trait Builtin: Trace {
	fn name(&self) -> &str;
	fn params(&self) -> &[BuiltinParam];
	fn call(&self, s: State, ctx: Context, loc: CallLocation, args: &dyn ArgsLike) -> Result<Val>;
}

pub trait StaticBuiltin: Builtin + Send + Sync
where
	Self: 'static,
{
	// In impl, to make it object safe:
	// const INST: &'static Self;
}

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

	fn call(&self, s: State, ctx: Context, _loc: CallLocation, args: &dyn ArgsLike) -> Result<Val> {
		let args = parse_builtin_call(s.clone(), ctx, &self.params, args, true)?;
		let mut out_args = Vec::with_capacity(self.params.len());
		for p in &self.params {
			out_args.push(args[&p.name].evaluate(s.clone())?);
		}
		self.handler.call(s, &out_args)
	}
}

pub trait NativeCallbackHandler: Trace {
	fn call(&self, s: State, args: &[Val]) -> Result<Val>;
}
