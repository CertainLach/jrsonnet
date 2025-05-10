use std::{fmt::Debug, rc::Rc};

pub use arglike::TlaArg;
use arglike::{ArgsLike, OptionalContext};
pub use builtin::{
	Builtin, CcBuiltin, NativeCallback, NativeCallbackHandler, Param, ParamDefault, ParamName,
	StaticBuiltin,
};
use educe::Educe;
use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
pub use jrsonnet_macros::builtin;
use jrsonnet_parser::{Destruct, Expr, LocExpr, ParamsDesc, Span};
pub use native::Desc as NativeDesc;
use parse::{parse_default_function_call, parse_function_call};

#[doc(hidden)]
pub mod macro_internal {
	pub use super::{arglike::ArgsLike, parse::parse_builtin_call};
}

use crate::{
	bail, error::ErrorKind::*, evaluate, evaluate_trivial, paramlist, Context, ContextBuilder,
	Result, Thunk, Val,
};

mod arglike;
mod builtin;
mod native;
mod parse;

/// Function callsite location.
/// Either from other jsonnet code, specified by expression location, or from native (without location).
#[derive(Clone, Copy)]
pub struct CallLocation<'l>(pub Option<&'l Span>);
impl<'l> CallLocation<'l> {
	/// Construct new location for calls coming from specified jsonnet expression location.
	pub const fn new(loc: &'l Span) -> Self {
		Self(Some(loc))
	}
}
impl CallLocation<'static> {
	/// Construct new location for calls coming from native code.
	pub const fn native() -> Self {
		Self(None)
	}
}

/// Represents Jsonnet function defined in code.
#[derive(Debug, Trace)]
pub struct FuncDesc {
	/// # Example
	///
	/// In expressions like this, deducted to `a`, unspecified otherwise.
	/// ```jsonnet
	/// local a = function() ...
	/// local a() ...
	/// { a: function() ... }
	/// { a() = ... }
	/// ```
	pub name: IStr,
	/// Context, in which this function was evaluated.
	///
	/// # Example
	/// In
	/// ```jsonnet
	/// local a = 2;
	/// function() ...
	/// ```
	/// context will contain `a`.
	pub ctx: Context,

	/// Function parameter definition
	pub params: ParamsDesc,
	/// Function body
	pub body: LocExpr,
}
impl FuncDesc {
	/// Create body context, but fill arguments without defaults with lazy error
	pub fn default_body_context(&self) -> Result<Context> {
		parse_default_function_call(self.ctx.clone(), &self.params)
	}

	/// Create context, with which body code will run
	pub fn call_body_context(
		&self,
		call_ctx: &Context,
		args: &impl ArgsLike,
		tailstrict: bool,
	) -> Result<Context> {
		parse_function_call(call_ctx, self.ctx.clone(), &self.params, args, tailstrict)
	}

	pub fn evaluate_trivial(&self) -> Option<Val> {
		evaluate_trivial(&self.body)
	}
}

/// Represents a Jsonnet function value, including plain functions and user-provided builtins.
#[derive(Trace, Clone, Educe)]
#[educe(Debug)]
pub enum FuncVal {
	/// Identity function, kept this way for comparsions.
	Id,
	/// Plain function implemented in jsonnet.
	Normal(Cc<FuncDesc>),
	/// Function without arguments works just as a fancy thunk value.
	Thunk(Thunk<Val>),
	/// Standard library function.
	StaticBuiltin(
		#[trace(skip)]
		#[educe(Debug(ignore))]
		&'static dyn StaticBuiltin,
	),
	/// User-provided function.
	Builtin(#[educe(Debug(ignore))] CcBuiltin),
}

#[allow(clippy::unnecessary_wraps)]
#[builtin]
const fn builtin_id(x: Val) -> Val {
	x
}
static ID: &builtin_id = &builtin_id {};

impl FuncVal {
	pub fn builtin(builtin: impl Builtin) -> Self {
		Self::Builtin(CcBuiltin::make(builtin))
	}
	pub fn static_builtin(static_builtin: &'static dyn StaticBuiltin) -> Self {
		Self::StaticBuiltin(static_builtin)
	}

	pub fn params(&self) -> Rc<[Param]> {
		paramlist!(empty_params:);
		match self {
			Self::Id => ID.params(),
			Self::StaticBuiltin(i) => i.params(),
			Self::Builtin(i) => i.as_ref().params(),
			Self::Normal(p) => p
				.params
				.iter()
				.map(|p| {
					Param::new(
						p.0.name()
							.as_ref()
							.map(IStr::to_string)
							.map_or(ParamName::ANONYMOUS, ParamName::new),
						ParamDefault::exists(p.1.is_some()),
					)
				})
				.collect(),
			Self::Thunk(_) => empty_params(),
		}
	}
	/// Amount of non-default required arguments
	pub fn params_len(&self) -> usize {
		match self {
			Self::Id => 1,
			Self::Normal(n) => n.params.iter().filter(|p| p.1.is_none()).count(),
			Self::StaticBuiltin(i) => i.params().iter().filter(|p| !p.has_default()).count(),
			Self::Builtin(i) => i
				.as_ref()
				.params()
				.iter()
				.filter(|p| !p.has_default())
				.count(),
			Self::Thunk(_) => 0,
		}
	}
	/// Function name, as defined in code.
	pub fn name(&self) -> IStr {
		match self {
			Self::Id => "id".into(),
			Self::Normal(normal) => normal.name.clone(),
			Self::StaticBuiltin(builtin) => builtin.name().into(),
			Self::Builtin(builtin) => builtin.as_ref().name().into(),
			Self::Thunk(_) => "thunk".into(),
		}
	}
	/// Call function using arguments evaluated in specified `call_ctx` [`Context`].
	///
	/// If `tailstrict` is specified - then arguments will be evaluated before being passed to function body.
	pub fn evaluate(
		&self,
		call_ctx: &Context,
		loc: CallLocation<'_>,
		args: &impl ArgsLike,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Id => ID.call(call_ctx, loc, args),
			Self::Normal(func) => {
				let body_ctx = func.call_body_context(call_ctx, args, tailstrict)?;
				evaluate(&body_ctx, &func.body)
			}
			Self::Thunk(thunk) => {
				if args.is_empty() {
					bail!(TooManyArgsFunctionHas(0, vec![],))
				}
				thunk.evaluate()
			}
			Self::StaticBuiltin(b) => b.call(call_ctx, loc, args),
			Self::Builtin(b) => b.as_ref().call(call_ctx, loc, args),
		}
	}
	pub fn evaluate_simple<A: ArgsLike + OptionalContext>(
		&self,
		args: &A,
		tailstrict: bool,
	) -> Result<Val> {
		let ctx = ContextBuilder::new().build();
		self.evaluate(&ctx, CallLocation::native(), args, tailstrict)
	}
	/// Convert jsonnet function to plain `Fn` value.
	pub fn into_native<D: NativeDesc>(self) -> D::Value {
		D::into_native(self)
	}

	/// Is this function an indentity function.
	///
	/// Currently only works for builtin `std.id`, aka `Self::Id` value, and `function(x) x`.
	///
	/// This function should only be used for optimization, not for the conditional logic, i.e code should work with syntetic identity function too
	pub fn is_identity(&self) -> bool {
		match self {
			Self::Id => true,
			Self::Normal(desc) => {
				if desc.params.len() != 1 {
					return false;
				}
				let param = &desc.params[0];
				if param.1.is_some() {
					return false;
				}
				#[allow(clippy::infallible_destructuring_match)]
				let id = match &param.0 {
					Destruct::Full(id) => id,
					#[cfg(feature = "exp-destruct")]
					_ => return false,
				};
				desc.body.expr() == &Expr::Var(id.clone())
			}
			_ => false,
		}
	}
	/// Identity function value.
	pub const fn identity() -> Self {
		Self::Id
	}

	pub fn evaluate_trivial(&self) -> Option<Val> {
		match self {
			Self::Normal(n) => n.evaluate_trivial(),
			_ => None,
		}
	}
}

impl<T> From<T> for FuncVal
where
	T: Builtin,
{
	fn from(value: T) -> Self {
		Self::builtin(value)
	}
}
impl From<&'static dyn StaticBuiltin> for FuncVal {
	fn from(value: &'static dyn StaticBuiltin) -> Self {
		Self::static_builtin(value)
	}
}
