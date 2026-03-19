use std::any::Any;

use jrsonnet_gcmodule::{cc_dyn, Acyclic, Trace, TraceBox};
use jrsonnet_interner::IStr;

use super::{arglike::ArgsLike, parse::parse_builtin_call, CallLocation};
use crate::{Context, Result, Val};

#[derive(Clone, Acyclic)]
pub struct ParamName(Option<IStr>);
impl ParamName {
	pub const ANONYMOUS: Self = Self(None);
	pub fn new(name: IStr) -> Self {
		Self(Some(name))
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
		self.0
			.as_ref()
			.map_or(false, |s| s.as_bytes() == other.as_bytes())
	}
}

#[derive(Clone, Copy, Debug, Acyclic)]
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

#[macro_export]
macro_rules! params {
	(@name unnamed) => { ParamName::ANONYMOUS };
	(@name named $name:literal) => { ParamName::new($crate::IStr::from($name)) };
	($($(#[$meta:meta])* [$kind:ident $(($lit:literal))? => $default:expr]),* $(,)?) => {
		thread_local! {
			static PARAMS: [ParamParse; { const N: usize = <[u8]>::len(&[$($(#[$meta])* 0u8),*]); N }] = [
				$($(#[$meta])* ParamParse::new(params!(@name $kind $($lit)?), $default)),*
			];
		}
	};
}

#[derive(Clone, Acyclic)]
pub struct ParamParse {
	name: ParamName,
	default: ParamDefault,
}
impl ParamParse {
	pub fn new(name: ParamName, default: ParamDefault) -> Self {
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

cc_dyn!(
	#[derive(Clone)]
	BuiltinFunc,
	Builtin,
	pub(crate) fn new() {...}
);
impl Builtin for BuiltinFunc {
	fn name(&self) -> &str {
		self.0.name()
	}

	fn params(&self) -> &[ParamParse] {
		self.0.params()
	}

	fn call(&self, ctx: Context, loc: CallLocation<'_>, args: &dyn ArgsLike) -> Result<Val> {
		self.0.call(ctx, loc, args)
	}

	fn as_any(&self) -> &dyn Any {
		self.0.as_any()
	}
}

/// Description of function defined by native code
///
/// Prefer to use #[builtin] macro, instead of manual implementation of this trait
pub trait Builtin: Trace {
	/// Function name to be used in stack traces
	fn name(&self) -> &str;
	/// Parameter names for named calls
	fn params(&self) -> &[ParamParse];
	/// Call the builtin
	fn call(&self, ctx: Context, loc: CallLocation<'_>, args: &dyn ArgsLike) -> Result<Val>;

	fn as_any(&self) -> &dyn Any;
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
	pub(crate) params: Vec<ParamParse>,
	handler: TraceBox<dyn NativeCallbackHandler>,
}
impl NativeCallback {
	#[deprecated = "prefer using builtins directly, use this interface only for bindings"]
	pub fn new(params: Vec<String>, handler: impl NativeCallbackHandler) -> Self {
		Self {
			params: params
				.into_iter()
				.map(|n| ParamParse {
					name: ParamName::new(n.into()),
					default: ParamDefault::None,
				})
				.collect(),
			handler: TraceBox(Box::new(handler)),
		}
	}
}

impl Builtin for NativeCallback {
	fn name(&self) -> &str {
		// TODO: standard natives gets their names from definition
		// But builitins should already have them
		"<native>"
	}

	fn params(&self) -> &[ParamParse] {
		&self.params
	}

	fn call(&self, ctx: Context, _loc: CallLocation<'_>, args: &dyn ArgsLike) -> Result<Val> {
		let args = parse_builtin_call(ctx, &self.params, args, true)?;
		let args = args
			.into_iter()
			.map(|a| a.expect("legacy natives have no default params"))
			.map(|a| a.evaluate())
			.collect::<Result<Vec<Val>>>()?;
		self.handler.call(&args)
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

pub trait NativeCallbackHandler: Trace {
	fn call(&self, args: &[Val]) -> Result<Val>;
}
