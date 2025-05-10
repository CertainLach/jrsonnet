use std::{any::Any, rc::Rc};

use jrsonnet_gcmodule::{Trace, TraceBox};
use jrsonnet_interner::IStr;

use super::{arglike::ArgsLike, parse::parse_builtin_call, CallLocation};
use crate::{Context, Result, Val};

#[derive(Clone, Trace)]
pub struct ParamName(Option<IStr>);
impl ParamName {
	pub const ANONYMOUS: Self = Self(None);
	pub fn new(name: impl Into<IStr>) -> Self {
		Self(Some(name.into()))
	}
	pub fn as_str(&self) -> Option<&str> {
		self.0.as_deref()
	}
	pub fn is_anonymous(&self) -> bool {
		self.0.is_none()
	}
}
impl PartialEq<IStr> for ParamName {
	fn eq(&self, other: &IStr) -> bool {
		self.0.as_ref().is_some_and(|s| s == other)
	}
}

#[derive(Clone, Copy, Debug, Trace)]
pub enum ParamDefault {
	None,
	Exists,
	Literal(&'static str),
}
impl ParamDefault {
	pub const fn exists(is_exists: bool) -> Self {
		if is_exists {
			Self::Exists
		} else {
			Self::None
		}
	}
}

#[derive(Clone, Trace)]
pub struct Param {
	name: ParamName,
	default: ParamDefault,
}
impl Param {
	pub const fn new(name: ParamName, default: ParamDefault) -> Self {
		Self { name, default }
	}
	/// Parameter name for named call parsing
	pub fn name(&self) -> &ParamName {
		&self.name
	}
	pub fn default(&self) -> ParamDefault {
		self.default
	}
	pub fn has_default(&self) -> bool {
		!matches!(self.default, ParamDefault::None)
	}
}

jrsonnet_gcmodule::cc_dyn!(CcBuiltin, Builtin);
impl Clone for CcBuiltin {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}
impl CcBuiltin {
	pub(crate) fn make(builtin: impl Builtin) -> Self {
		Self::new(builtin)
	}
}
impl AsRef<dyn Builtin> for CcBuiltin {
	fn as_ref(&self) -> &dyn Builtin {
		&*self.0
	}
}

/// Description of function defined by native code
///
/// Prefer to use #[builtin] macro, instead of manual implementation of this trait
#[allow(clippy::module_name_repetitions)]
pub trait Builtin: Trace + Any {
	/// Function name to be used in stack traces
	fn name(&self) -> &str;
	/// Parameter names for named calls
	fn params(&self) -> Rc<[Param]>;
	/// Call the builtin
	fn call(&self, ctx: &Context, loc: CallLocation<'_>, args: &dyn ArgsLike) -> Result<Val>;
}

#[allow(clippy::module_name_repetitions)]
pub trait StaticBuiltin: Builtin + Send + Sync
where
	Self: 'static,
{
	// In impl, to make it object safe:
	// const INST: &'static Self;
}

#[derive(Trace)]
pub struct NativeCallback {
	#[trace(skip)]
	pub(crate) params: Rc<[Param]>,
	handler: TraceBox<dyn NativeCallbackHandler>,
}
impl NativeCallback {
	#[deprecated = "prefer using builtins directly, use this interface only for bindings"]
	pub fn new(params: Vec<String>, handler: impl NativeCallbackHandler) -> Self {
		Self {
			params: params
				.into_iter()
				.map(|n| Param {
					name: ParamName::new(n),
					default: ParamDefault::None,
				})
				.collect(),
			handler: TraceBox(Box::new(handler)),
		}
	}
}

impl Builtin for NativeCallback {
	fn name(&self) -> &'static str {
		// TODO: standard natives gets their names from definition
		// But builitins should already have them
		"<native>"
	}

	fn params(&self) -> Rc<[Param]> {
		self.params.clone()
	}

	fn call(&self, ctx: &Context, _loc: CallLocation<'_>, args: &dyn ArgsLike) -> Result<Val> {
		let args = parse_builtin_call(ctx, &self.params, args, true)?;
		let args = args
			.into_iter()
			.map(|a| a.expect("legacy natives have no default params"))
			.map(|a| a.evaluate())
			.collect::<Result<Vec<Val>>>()?;
		self.handler.call(&args)
	}
}

pub trait NativeCallbackHandler: Trace {
	fn call(&self, args: &[Val]) -> Result<Val>;
}
