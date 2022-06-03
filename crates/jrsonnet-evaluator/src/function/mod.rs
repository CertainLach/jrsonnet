use std::fmt::Debug;

pub use arglike::{ArgLike, ArgsLike, TlaArg};
use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
pub use jrsonnet_macros::builtin;
use jrsonnet_parser::{ExprLocation, LocExpr, ParamsDesc};

use self::{
	builtin::{Builtin, StaticBuiltin},
	native::NativeDesc,
	parse::{parse_default_function_call, parse_function_call},
};
use crate::{evaluate, gc::TraceBox, typed::Any, Context, Result, State, Val};

pub mod arglike;
pub mod builtin;
pub mod native;
pub mod parse;

#[derive(Clone, Copy)]
pub struct CallLocation<'l>(pub Option<&'l ExprLocation>);
impl<'l> CallLocation<'l> {
	pub const fn new(loc: &'l ExprLocation) -> Self {
		Self(Some(loc))
	}
}
impl CallLocation<'static> {
	pub const fn native() -> Self {
		Self(None)
	}
}

/// Function implemented in jsonnet
#[derive(Debug, PartialEq, Trace)]
pub struct FuncDesc {
	/// In expressions like
	/// ```jsonnet
	/// local a = function() ...
	/// local a() ...
	/// { a: function() ... }
	/// { a() = ... }
	/// ```
	///
	/// Deducted to `a`, unspecified otherwise
	pub name: IStr,
	/// Context, in which this function was evaluated
	///
	/// I.e in
	/// ```jsonnet
	/// local a = 2;
	/// function() ...
	/// ```
	/// context will contain `a`
	pub ctx: Context,

	pub params: ParamsDesc,
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
		s: State,
		call_ctx: Context,
		args: &dyn ArgsLike,
		tailstrict: bool,
	) -> Result<Context> {
		parse_function_call(
			s,
			call_ctx,
			self.ctx.clone(),
			&self.params,
			args,
			tailstrict,
		)
	}
}

/// Any possible function value, including plain functions and user-provided builtins
#[allow(clippy::module_name_repetitions)]
#[derive(Trace, Clone)]
pub enum FuncVal {
	/// std.id
	Id,
	/// Plain function implemented in jsonnet
	Normal(Cc<FuncDesc>),
	/// Standard library function
	StaticBuiltin(#[skip_trace] &'static dyn StaticBuiltin),
	/// User-provided function
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

impl FuncVal {
	pub fn into_native<D: NativeDesc>(self) -> D::Value {
		D::into_native(self)
	}
	pub fn params_len(&self) -> usize {
		match self {
			Self::Id => 1,
			Self::Normal(n) => n.params.iter().filter(|p| p.1.is_none()).count(),
			Self::StaticBuiltin(i) => i.params().iter().filter(|p| !p.has_default).count(),
			Self::Builtin(i) => i.params().iter().filter(|p| !p.has_default).count(),
		}
	}
	pub fn name(&self) -> IStr {
		match self {
			Self::Id => "id".into(),
			Self::Normal(normal) => normal.name.clone(),
			Self::StaticBuiltin(builtin) => builtin.name().into(),
			Self::Builtin(builtin) => builtin.name().into(),
		}
	}
	pub fn evaluate(
		&self,
		s: State,
		call_ctx: Context,
		loc: CallLocation,
		args: &dyn ArgsLike,
		tailstrict: bool,
	) -> Result<Val> {
		match self {
			Self::Id => {
				#[allow(clippy::unnecessary_wraps)]
				#[builtin]
				const fn builtin_id(v: Any) -> Result<Any> {
					Ok(v)
				}
				static ID: &builtin_id = &builtin_id {};

				ID.call(s, call_ctx, loc, args)
			}
			Self::Normal(func) => {
				let body_ctx = func.call_body_context(s.clone(), call_ctx, args, tailstrict)?;
				evaluate(s, body_ctx, &func.body)
			}
			Self::StaticBuiltin(b) => b.call(s, call_ctx, loc, args),
			Self::Builtin(b) => b.call(s, call_ctx, loc, args),
		}
	}
	pub fn evaluate_simple(&self, s: State, args: &dyn ArgsLike) -> Result<Val> {
		self.evaluate(s, Context::default(), CallLocation::native(), args, true)
	}

	pub const fn is_identity(&self) -> bool {
		matches!(self, Self::Id)
	}
	pub const fn identity() -> Self {
		Self::Id
	}
}
