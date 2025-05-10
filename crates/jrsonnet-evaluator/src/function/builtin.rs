use std::{any::Any, fmt::Display, rc::Rc};

use jrsonnet_gcmodule::{Trace, TraceBox};
use jrsonnet_interner::IStr;

use super::CallLocation;
use crate::{BindingValue, Result, Val};

#[derive(Clone, Trace, Debug)]
pub struct ParamName(pub Option<IStr>);
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

impl Display for ParamName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(name) = self.as_str() {
			name.fmt(f)
		} else {
			"<anonymous>".fmt(f)
		}
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
	fn call(&self, loc: CallLocation<'_>, args: &[Option<BindingValue>]) -> Result<Val>;
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

	fn call(&self, _loc: CallLocation<'_>, args: &[Option<BindingValue>]) -> Result<Val> {
		let args = args
			.into_iter()
			.cloned()
			.map(|a| a.expect("legacy natives have no default params"))
			.map(|a| a.evaluate())
			.collect::<Result<Vec<Val>>>()?;
		self.handler.call(&args)
	}
}

pub trait NativeCallbackHandler: Trace {
	fn call(&self, args: &[Val]) -> Result<Val>;
}
