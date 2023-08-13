use std::fmt::Debug;

pub use arglike::{ArgLike, ArgsLike, TlaArg};
use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
pub use jrsonnet_macros::builtin;
use jrsonnet_parser::{Destruct, Expr, ExprLocation, LocExpr, ParamsDesc};

use self::{
	arglike::OptionalContext,
	builtin::{Builtin, BuiltinParam, ParamName, StaticBuiltin},
	native::NativeDesc,
	parse::{parse_default_function_call, parse_function_call},
};
use crate::{
	evaluate, evaluate_trivial, gc::TraceBox, tb, Context, ContextBuilder, Result,
	Val,
};

pub mod arglike;
pub mod builtin;
pub mod native;
pub mod parse;

/// Function callsite location.
/// Either from other jsonnet code, specified by expression location, or from native (without location).
#[derive(Clone, Copy)]
pub struct CallLocation<'l>(pub Option<&'l ExprLocation>);
impl<'l> CallLocation<'l> {
	/// Construct new location for calls coming from specified jsonnet expression location.
	pub const fn new(loc: &'l ExprLocation) -> Self {
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
#[derive(Debug, PartialEq, Trace)]
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
		call_ctx: Context,
		args: &dyn ArgsLike,
		tailstrict: bool,
	) -> Result<Context> {
		parse_function_call(call_ctx, self.ctx.clone(), &self.params, args, tailstrict)
	}

	pub fn evaluate_trivial(&self) -> Option<Val> {
		evaluate_trivial(&self.body)
	}
}

/// Represents a Jsonnet function value, including plain functions and user-provided builtins.
#[allow(clippy::module_name_repetitions)]
#[derive(Trace, Clone)]
pub enum FuncVal {
	/// Identity function, kept this way for comparsions.
	Id,
	/// Plain function implemented in jsonnet.
	Normal(Cc<FuncDesc>),
	/// Standard library function.
	StaticBuiltin(#[trace(skip)] &'static dyn StaticBuiltin),
	/// User-provided function.
	Builtin(Cc<TraceBox<dyn Builtin>>),
}

impl Debug for FuncVal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Id => f.debug_tuple("Id").finish(),
			Self::Normal(arg0) => f.debug_tuple("Normal").field(arg0).finish(),
			Self::StaticBuiltin(arg0) => {
				f.debug_tuple("StaticBuiltin").field(&arg0.name()).finish()
			}
			Self::Builtin(arg0) => f.debug_tuple("Builtin").field(&arg0.name()).finish(),
		}
	}
}

#[allow(clippy::unnecessary_wraps)]
#[builtin]
const fn builtin_id(x: Val) -> Val {
	x
}
static ID: &builtin_id = &builtin_id {};

impl FuncVal {
	pub fn builtin(builtin: impl Builtin) -> Self {
		Self::Builtin(Cc::new(tb!(builtin)))
	}
	pub fn static_builtin(static_builtin: &'static dyn StaticBuiltin) -> Self {
		Self::StaticBuiltin(static_builtin)
	}

	pub fn params(&self) -> Vec<BuiltinParam> {
		match self {
			Self::Id => ID.params().to_vec(),
			Self::StaticBuiltin(i) => i.params().to_vec(),
			Self::Builtin(i) => i.params().to_vec(),
			Self::Normal(p) => p
				.params
				.iter()
				.map(|p| {
					BuiltinParam::new(
						p.0.name()
							.as_ref()
							.map(IStr::to_string)
							.map_or(ParamName::ANONYMOUS, ParamName::new_dynamic),
						p.1.is_some(),
					)
				})
				.collect(),
		}
	}
	/// Amount of non-default required arguments
	pub fn params_len(&self) -> usize {
		match self {
			Self::Id => 1,
			Self::Normal(n) => n.params.iter().filter(|p| p.1.is_none()).count(),
			Self::StaticBuiltin(i) => i.params().iter().filter(|p| !p.has_default()).count(),
			Self::Builtin(i) => i.params().iter().filter(|p| !p.has_default()).count(),
		}
	}
	/// Function name, as defined in code.
	pub fn name(&self) -> IStr {
		match self {
			Self::Id => "id".into(),
			Self::Normal(normal) => normal.name.clone(),
			Self::StaticBuiltin(builtin) => builtin.name().into(),
			Self::Builtin(builtin) => builtin.name().into(),
		}
	}
	/// Call function using arguments evaluated in specified `call_ctx` [`Context`].
	///
	/// If `tailstrict` is specified - then arguments will be evaluated before being passed to function body.
	pub fn evaluate(
		&self,
		call_ctx: Context,
		loc: CallLocation<'_>,
		args: &dyn ArgsLike,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Id => ID.call(call_ctx, loc, args),
			Self::Normal(func) => {
				let body_ctx = func.call_body_context(call_ctx, args, tailstrict)?;
				evaluate(body_ctx, &func.body)
			}
			Self::StaticBuiltin(b) => b.call(call_ctx, loc, args),
			Self::Builtin(b) => b.call(call_ctx, loc, args),
		}
	}
	pub fn evaluate_simple<A: ArgsLike + OptionalContext>(
		&self,
		args: &A,
		tailstrict: bool,
	) -> Result<Val> {
		self.evaluate(
			ContextBuilder::dangerous_empty_state().build(),
			CallLocation::native(),
			args,
			tailstrict,
		)
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
				&desc.body.0 as &Expr == &Expr::Var(id.clone())
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
			FuncVal::Normal(n) => n.evaluate_trivial(),
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
