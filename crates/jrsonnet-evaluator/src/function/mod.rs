use std::{fmt::Debug, rc::Rc};

use educe::Educe;
use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
pub use jrsonnet_macros::builtin;
use jrsonnet_ir::{ArgsDesc, Destruct, Expr, ExprParams, Span, Spanned};

use self::{
	builtin::{Builtin, StaticBuiltin},
	parse::{parse_builtin_call, parse_default_function_call, parse_function_call},
	prepared::{parse_prepared_builtin_call, parse_prepared_function_call, PreparedCall},
};
use crate::{
	bail, error::ErrorKind::*, evaluate, evaluate_trivial, function::builtin::BuiltinFunc, Context,
	Result, Thunk, Val,
};

pub mod builtin;
mod native;
mod parse;
mod prepared;

pub use native::NativeFn;
pub use prepared::PreparedFuncVal;

pub use jrsonnet_ir::function::*;

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
#[derive(Trace, Educe)]
#[educe(Debug, PartialEq)]
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
	pub params: ExprParams,
	/// Function body
	pub body: Rc<Spanned<Expr>>,
}
impl FuncDesc {
	/// Create body context, but fill arguments without defaults with lazy error
	pub fn default_body_context(&self) -> Result<Context> {
		parse_default_function_call(self.ctx.clone(), &self.params)
	}

	/// Create context, with which body code will run
	pub(crate) fn call_body_context(
		&self,
		call_ctx: Context,
		args: &ArgsDesc,
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
	/// Function without arguments works just as a fancy thunk value.
	Thunk(Thunk<Val>),
	/// Standard library function.
	StaticBuiltin(#[trace(skip)] &'static dyn StaticBuiltin),
	/// User-provided function.
	Builtin(BuiltinFunc),
}

impl Debug for FuncVal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Id => f.debug_tuple("Id").finish(),
			Self::Thunk(arg0) => f.debug_tuple("Thunk").field(arg0).finish(),
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
		Self::Builtin(BuiltinFunc::new(builtin))
	}
	pub fn static_builtin(static_builtin: &'static dyn StaticBuiltin) -> Self {
		Self::StaticBuiltin(static_builtin)
	}

	pub fn params(&self) -> FunctionSignature {
		match self {
			Self::Id => ID.params(),
			Self::StaticBuiltin(i) => i.params(),
			Self::Builtin(i) => i.params(),
			Self::Normal(p) => p.params.signature.clone(),
			Self::Thunk(_) => FunctionSignature::empty(),
		}
	}
	/// Amount of non-default required arguments
	pub fn params_len(&self) -> usize {
		self.params().iter().filter(|p| !p.has_default()).count()
	}
	/// Function name, as defined in code.
	pub fn name(&self) -> IStr {
		match self {
			Self::Id => "id".into(),
			Self::Normal(normal) => normal.name.clone(),
			Self::StaticBuiltin(builtin) => builtin.name().into(),
			Self::Builtin(builtin) => builtin.name().into(),
			Self::Thunk(_) => "thunk".into(),
		}
	}
	/// Call function using arguments evaluated in specified `call_ctx` [`Context`].
	///
	/// If `tailstrict` is specified - then arguments will be evaluated before being passed to function body.
	pub fn evaluate(
		&self,
		call_ctx: Context,
		loc: CallLocation<'_>,
		args: &ArgsDesc,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Normal(func) => {
				let body_ctx = func.call_body_context(call_ctx, args, tailstrict)?;
				evaluate(body_ctx, &func.body)
			}
			Self::Thunk(thunk) => {
				if !args.named.is_empty() || !args.unnamed.is_empty() {
					bail!(TooManyArgsFunctionHas(0, FunctionSignature::empty()))
				}
				thunk.evaluate()
			}
			Self::Id => {
				let args = parse_builtin_call(call_ctx, ID.params(), args, tailstrict)?;
				ID.call(loc, &args)
			}
			Self::StaticBuiltin(b) => {
				let args = parse_builtin_call(call_ctx, b.params(), args, tailstrict)?;
				b.call(loc, &args)
			}
			Self::Builtin(b) => {
				let args = parse_builtin_call(call_ctx, b.params(), args, tailstrict)?;
				b.call(loc, &args)
			}
		}
	}

	pub(crate) fn evaluate_prepared(
		&self,
		prepared: &PreparedCall,
		loc: CallLocation<'_>,
		unnamed: &[Thunk<Val>],
		named: &[Thunk<Val>],
		_tailstrict: bool,
	) -> Result<Val> {
		match self {
			FuncVal::Normal(func) => {
				let body_ctx = parse_prepared_function_call(
					func.ctx.clone(),
					prepared,
					&func.params,
					unnamed,
					named,
				)?;
				evaluate(body_ctx, &func.body)
			}
			FuncVal::Thunk(t) => t.evaluate(),
			FuncVal::Id => {
				let args = parse_prepared_builtin_call(prepared, ID.params(), unnamed, named);
				ID.call(loc, &args)
			}
			FuncVal::StaticBuiltin(b) => {
				let args = parse_prepared_builtin_call(prepared, b.params(), unnamed, named);
				b.call(loc, &args)
			}
			FuncVal::Builtin(b) => {
				let args = parse_prepared_builtin_call(prepared, b.params(), unnamed, named);
				b.call(loc, &args)
			}
		}
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
				let param = &desc.params.exprs[0];
				if param.default.is_some() {
					return false;
				}

				#[allow(clippy::infallible_destructuring_match)]
				let id = match &param.destruct {
					Destruct::Full(id) => id,
					#[cfg(feature = "exp-destruct")]
					_ => return false,
				};
				**desc.body == Expr::Var(id.clone())
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
