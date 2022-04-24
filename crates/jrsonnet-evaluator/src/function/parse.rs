use gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{LocExpr, ParamsDesc};

use super::{
	arglike::ArgsLike,
	builtin::{BuiltinParam, BuiltinParamName},
};
use crate::{
	error::{Error::*, Result},
	evaluate_named,
	gc::{GcHashMap, TraceBox},
	throw,
	val::LazyValValue,
	Context, FutureWrapper, LazyVal, State, Val,
};

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

			defaults.insert(
				param.0.clone(),
				LazyVal::new(TraceBox(Box::new(EvaluateNamedLazyVal {
					ctx: fctx.clone(),
					name: param.0.clone(),
					value: param.1.clone().expect("default exists"),
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

/// You shouldn't probally use this function, use `jrsonnet_macros::builtin` instead
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
	#[derive(Trace)]
	struct DependsOnUnbound(IStr);
	impl LazyValValue for DependsOnUnbound {
		fn get(self: Box<Self>, _: State) -> Result<Val> {
			Err(FunctionParameterNotBoundInCall(self.0.clone()).into())
		}
	}

	let fctx = Context::new_future();

	let mut bindings = GcHashMap::new();

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
