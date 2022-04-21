use std::{borrow::Cow, collections::HashMap};

use gcmodule::Trace;
use jrsonnet_interner::IStr;
pub use jrsonnet_macros::builtin;
use jrsonnet_parser::{ArgsDesc, ExprLocation, LocExpr, ParamsDesc};

use crate::{
	error::Error::*, evaluate, evaluate_named, gc::TraceBox, throw, typed::Typed,
	val::LazyValValue, Context, FutureWrapper, GcHashMap, LazyVal, Result, State, Val,
};

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

#[derive(Trace)]
struct EvaluateLazyVal {
	ctx: Context,
	expr: LocExpr,
}
impl LazyValValue for EvaluateLazyVal {
	fn get(self: Box<Self>, s: State) -> Result<Val> {
		evaluate(s, self.ctx, &self.expr)
	}
}

#[derive(Trace)]
struct EvaluateNamedLazyVal {
	ctx: FutureWrapper<Context>,
	name: IStr,
	value: LocExpr,
}
impl LazyValValue for EvaluateNamedLazyVal {
	fn get(self: Box<Self>, s: State) -> Result<Val> {
		evaluate_named(s, self.ctx.unwrap(), &self.value, self.name)
	}
}

pub trait ArgLike {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<LazyVal>;
}
impl ArgLike for &LocExpr {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<LazyVal> {
		Ok(if tailstrict {
			LazyVal::new_resolved(evaluate(s, ctx, self)?)
		} else {
			LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
				ctx,
				expr: (*self).clone(),
			})))
		})
	}
}
impl<T> ArgLike for T
where
	T: Typed + Clone,
{
	fn evaluate_arg(&self, s: State, _ctx: Context, _tailstrict: bool) -> Result<LazyVal> {
		let val = T::into_untyped(self.clone(), s)?;
		Ok(LazyVal::new_resolved(val))
	}
}
pub enum TlaArg {
	String(IStr),
	Code(LocExpr),
	Val(Val),
}
impl ArgLike for TlaArg {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<LazyVal> {
		match self {
			TlaArg::String(s) => Ok(LazyVal::new_resolved(Val::Str(s.clone()))),
			TlaArg::Code(code) => Ok(if tailstrict {
				LazyVal::new_resolved(evaluate(s, ctx, code)?)
			} else {
				LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
					ctx,
					expr: code.clone(),
				})))
			}),
			TlaArg::Val(val) => Ok(LazyVal::new_resolved(val.clone())),
		}
	}
}

pub trait ArgsLike {
	fn unnamed_len(&self) -> usize;
	fn unnamed_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()>;
	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()>;
	fn named_names(&self, handler: &mut dyn FnMut(&IStr));
}

impl ArgsLike for ArgsDesc {
	fn unnamed_len(&self) -> usize {
		self.unnamed.len()
	}

	fn unnamed_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (id, arg) in self.unnamed.iter().enumerate() {
			handler(
				id,
				if tailstrict {
					LazyVal::new_resolved(evaluate(s.clone(), ctx.clone(), arg)?)
				} else {
					LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
						ctx: ctx.clone(),
						expr: arg.clone(),
					})))
				},
			)?;
		}
		Ok(())
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (name, arg) in self.named.iter() {
			handler(
				name,
				if tailstrict {
					LazyVal::new_resolved(evaluate(s.clone(), ctx.clone(), arg)?)
				} else {
					LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
						ctx: ctx.clone(),
						expr: arg.clone(),
					})))
				},
			)?;
		}
		Ok(())
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		for (name, _) in self.named.iter() {
			handler(name)
		}
	}
}

impl<A: ArgLike> ArgsLike for [(IStr, A)] {
	fn unnamed_len(&self) -> usize {
		0
	}

	fn unnamed_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (name, val) in self.iter() {
			handler(name, val.evaluate_arg(s.clone(), ctx.clone(), tailstrict)?)?;
		}
		Ok(())
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		for (name, _) in self.iter() {
			handler(name);
		}
	}
}

impl<A: ArgLike> ArgsLike for HashMap<IStr, A> {
	fn unnamed_len(&self) -> usize {
		0
	}

	fn unnamed_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (name, value) in self.iter() {
			handler(
				name,
				value.evaluate_arg(s.clone(), ctx.clone(), tailstrict)?,
			)?;
		}
		Ok(())
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		for (name, _) in self.iter() {
			handler(name);
		}
	}
}

impl<A: ArgLike> ArgsLike for [A] {
	fn unnamed_len(&self) -> usize {
		self.len()
	}

	fn unnamed_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (i, arg) in self.iter().enumerate() {
			handler(i, arg.evaluate_arg(s.clone(), ctx.clone(), tailstrict)?)?;
		}
		Ok(())
	}

	fn named_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
}
impl<A: ArgLike> ArgsLike for &[A] {
	fn unnamed_len(&self) -> usize {
		(*self).unnamed_len()
	}

	fn unnamed_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		(*self).unnamed_iter(s, ctx, tailstrict, handler)
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		(*self).named_iter(s, ctx, tailstrict, handler)
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		(*self).named_names(handler)
	}
}

/// Creates correct [context](Context) for function body evaluation returning error on invalid call.
///
/// ## Parameters
/// * `ctx`: used for passed argument expressions' execution and for body execution (if `body_ctx` is not set)
/// * `body_ctx`: used for default parameter values' execution and for body execution (if set)
/// * `params`: function parameters' definition
/// * `args`: passed function arguments
/// * `tailstrict`: if set to `true` function arguments are eagerly executed, otherwise - lazily
pub fn parse_function_call(
	s: State,
	ctx: Context,
	body_ctx: Context,
	params: &ParamsDesc,
	args: &dyn ArgsLike,
	tailstrict: bool,
) -> Result<Context> {
	let mut passed_args = GcHashMap::with_capacity(params.len());
	if args.unnamed_len() > params.len() {
		throw!(TooManyArgsFunctionHas(params.len()))
	}

	let mut filled_args = 0;

	args.unnamed_iter(s.clone(), ctx.clone(), tailstrict, &mut |id, arg| {
		let name = params[id].0.clone();
		passed_args.insert(name, arg);
		filled_args += 1;
		Ok(())
	})?;

	args.named_iter(s, ctx, tailstrict, &mut |name, value| {
		// FIXME: O(n) for arg existence check
		if !params.iter().any(|p| &p.0 == name) {
			throw!(UnknownFunctionParameter((name as &str).to_owned()));
		}
		if passed_args.insert(name.clone(), value).is_some() {
			throw!(BindingParameterASecondTime(name.clone()));
		}
		filled_args += 1;
		Ok(())
	})?;

	if filled_args < params.len() {
		// Some args are unset, but maybe we have defaults for them
		// Default values should be created in newly created context
		let fctx = Context::new_future();
		let mut defaults = GcHashMap::with_capacity(params.len() - filled_args);

		for param in params.iter().filter(|p| p.1.is_some()) {
			if passed_args.contains_key(&param.0.clone()) {
				continue;
			}
			LazyVal::new(TraceBox(Box::new(EvaluateNamedLazyVal {
				ctx: fctx.clone(),
				name: param.0.clone(),
				value: param.1.clone().unwrap(),
			})));

			defaults.insert(
				param.0.clone(),
				LazyVal::new(TraceBox(Box::new(EvaluateNamedLazyVal {
					ctx: fctx.clone(),
					name: param.0.clone(),
					value: param.1.clone().unwrap(),
				}))),
			);
			filled_args += 1;
		}

		// Some args still wasn't filled
		if filled_args != params.len() {
			for param in params.iter().skip(args.unnamed_len()) {
				let mut found = false;
				args.named_names(&mut |name| {
					if name == &param.0 {
						found = true;
					}
				});
				if !found {
					throw!(FunctionParameterNotBoundInCall(param.0.clone()));
				}
			}
			unreachable!();
		}

		Ok(body_ctx
			.extend(passed_args, None, None, None)
			.extend_bound(defaults)
			.into_future(fctx))
	} else {
		let body_ctx = body_ctx.extend(passed_args, None, None, None);
		Ok(body_ctx)
	}
}

type BuiltinParamName = Cow<'static, str>;

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

/// You shouldn't probally use this function, use jrsonnet_macros::builtin instead
///
/// ## Parameters
/// * `ctx`: used for passed argument expressions' execution and for body execution (if `body_ctx` is not set)
/// * `params`: function parameters' definition
/// * `args`: passed function arguments
/// * `tailstrict`: if set to `true` function arguments are eagerly executed, otherwise - lazily
pub fn parse_builtin_call(
	s: State,
	ctx: Context,
	params: &[BuiltinParam],
	args: &dyn ArgsLike,
	tailstrict: bool,
) -> Result<GcHashMap<BuiltinParamName, LazyVal>> {
	let mut passed_args = GcHashMap::with_capacity(params.len());
	if args.unnamed_len() > params.len() {
		throw!(TooManyArgsFunctionHas(params.len()))
	}

	let mut filled_args = 0;

	args.unnamed_iter(s.clone(), ctx.clone(), tailstrict, &mut |id, arg| {
		let name = params[id].name.clone();
		passed_args.insert(name, arg);
		filled_args += 1;
		Ok(())
	})?;

	args.named_iter(s, ctx, tailstrict, &mut |name, arg| {
		// FIXME: O(n) for arg existence check
		let p = params
			.iter()
			.find(|p| p.name == name as &str)
			.ok_or_else(|| UnknownFunctionParameter((name as &str).to_owned()))?;
		if passed_args.insert(p.name.clone(), arg).is_some() {
			throw!(BindingParameterASecondTime(name.clone()));
		}
		filled_args += 1;
		Ok(())
	})?;

	if filled_args < params.len() {
		for param in params.iter().filter(|p| p.has_default) {
			if passed_args.contains_key(&param.name) {
				continue;
			}
			filled_args += 1;
		}

		// Some args still wasn't filled
		if filled_args != params.len() {
			for param in params.iter().skip(args.unnamed_len()) {
				let mut found = false;
				args.named_names(&mut |name| {
					if name as &str == &param.name as &str {
						found = true;
					}
				});
				if !found {
					throw!(FunctionParameterNotBoundInCall(param.name.clone().into()));
				}
			}
			unreachable!();
		}
	}
	Ok(passed_args)
}

/// Creates Context, which has all argument default values applied
/// and with unbound values causing error to be returned
pub fn parse_default_function_call(body_ctx: Context, params: &ParamsDesc) -> Context {
	let fctx = Context::new_future();

	let mut bindings = GcHashMap::new();

	#[derive(Trace)]
	struct DependsOnUnbound(IStr);
	impl LazyValValue for DependsOnUnbound {
		fn get(self: Box<Self>, _: State) -> Result<Val> {
			Err(FunctionParameterNotBoundInCall(self.0.clone()).into())
		}
	}

	for param in params.iter() {
		if let Some(v) = &param.1 {
			bindings.insert(
				param.0.clone(),
				LazyVal::new(TraceBox(Box::new(EvaluateNamedLazyVal {
					ctx: fctx.clone(),
					name: param.0.clone(),
					value: v.clone(),
				}))),
			);
		} else {
			bindings.insert(
				param.0.clone(),
				LazyVal::new(TraceBox(Box::new(DependsOnUnbound(param.0.clone())))),
			);
		}
	}

	body_ctx
		.extend(bindings, None, None, None)
		.into_future(fctx)
}
