use crate::{
	error::Error::*, evaluate, evaluate_named, gc::TraceBox, throw, Context, FutureWrapper,
	GcHashMap, LazyVal, LazyValValue, Result, Val,
};
use gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, LocExpr, ParamsDesc};
use std::collections::HashMap;

const NO_DEFAULT_CONTEXT: &str =
	"no default context set for call with defined default parameter value";

#[derive(Trace)]
struct EvaluateLazyVal {
	context: Context,
	expr: LocExpr,
}
impl LazyValValue for EvaluateLazyVal {
	fn get(self: Box<Self>) -> Result<Val> {
		evaluate(self.context, &self.expr)
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
	ctx: Context,
	body_ctx: Context,
	params: &ParamsDesc,
	args: &ArgsDesc,
	tailstrict: bool,
) -> Result<Context> {
	let mut passed_args = GcHashMap::with_capacity(params.len());
	if args.unnamed.len() > params.len() {
		throw!(TooManyArgsFunctionHas(params.len()))
	}

	let mut filled_args = 0;

	for (id, arg) in args.unnamed.iter().enumerate() {
		let name = params[id].0.clone();
		passed_args.insert(
			name,
			if tailstrict {
				LazyVal::new_resolved(evaluate(ctx.clone(), arg)?)
			} else {
				LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
					context: ctx.clone(),
					expr: arg.clone(),
				})))
			},
		);
		filled_args += 1;
	}

	for (name, value) in args.named.iter() {
		// FIXME: O(n) for arg existence check
		if !params.iter().any(|p| &p.0 == name) {
			throw!(UnknownFunctionParameter((name as &str).to_owned()));
		}
		if passed_args
			.insert(
				name.clone(),
				if tailstrict {
					LazyVal::new_resolved(evaluate(ctx.clone(), value)?)
				} else {
					LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
						context: ctx.clone(),
						expr: value.clone(),
					})))
				},
			)
			.is_some()
		{
			throw!(BindingParameterASecondTime(name.clone()));
		}
		filled_args += 1;
	}

	if filled_args < params.len() {
		// Some args are unset, but maybe we have defaults for them
		// Default values should be created in newly created context
		let future_context = FutureWrapper::<Context>::new();
		let mut defaults = GcHashMap::with_capacity(params.len() - filled_args);

		for param in params.iter().filter(|p| p.1.is_some()) {
			if passed_args.contains_key(&param.0.clone()) {
				continue;
			}
			#[derive(Trace)]
			struct LazyNamedBinding {
				future_context: FutureWrapper<Context>,
				name: IStr,
				value: LocExpr,
			}
			impl LazyValValue for LazyNamedBinding {
				fn get(self: Box<Self>) -> Result<Val> {
					evaluate_named(self.future_context.unwrap(), &self.value, self.name)
				}
			}
			LazyVal::new(TraceBox(Box::new(LazyNamedBinding {
				future_context: future_context.clone(),
				name: param.0.clone(),
				value: param.1.clone().unwrap(),
			})));

			defaults.insert(
				param.0.clone(),
				LazyVal::new(TraceBox(Box::new(LazyNamedBinding {
					future_context: future_context.clone(),
					name: param.0.clone(),
					value: param.1.clone().unwrap(),
				}))),
			);
			filled_args += 1;
		}

		// Some args still wasn't filled
		if filled_args != params.len() {
			for param in params.iter().skip(args.unnamed.len()) {
				if !args.named.iter().any(|a| a.0 == param.0) {
					throw!(FunctionParameterNotBoundInCall(param.0.clone()));
				}
			}
			unreachable!();
		}

		Ok(body_ctx
			.extend(passed_args, None, None, None)
			.extend_bound(defaults)
			.into_future(future_context))
	} else {
		let body_ctx = body_ctx.extend(passed_args, None, None, None);
		Ok(body_ctx)
	}
}

#[derive(Clone, Copy)]
pub struct BuiltinParam {
	pub name: &'static str,
	pub has_default: bool,
}

/// You shouldn't probally use this function, use jrsonnet_macros::builtin instead
///
/// ## Parameters
/// * `ctx`: used for passed argument expressions' execution and for body execution (if `body_ctx` is not set)
/// * `params`: function parameters' definition
/// * `args`: passed function arguments
/// * `tailstrict`: if set to `true` function arguments are eagerly executed, otherwise - lazily
pub fn parse_builtin_call<'k>(
	ctx: Context,
	params: &'static [BuiltinParam],
	args: &'k ArgsDesc,
	tailstrict: bool,
) -> Result<GcHashMap<&'k str, LazyVal>> {
	let mut passed_args = GcHashMap::with_capacity(params.len());
	if args.unnamed.len() > params.len() {
		throw!(TooManyArgsFunctionHas(params.len()))
	}

	let mut filled_args = 0;

	for (id, arg) in args.unnamed.iter().enumerate() {
		let name = params[id].name;
		passed_args.insert(
			name,
			if tailstrict {
				LazyVal::new_resolved(evaluate(ctx.clone(), arg)?)
			} else {
				LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
					context: ctx.clone(),
					expr: arg.clone(),
				})))
			},
		);
		filled_args += 1;
	}

	for (name, value) in args.named.iter() {
		// FIXME: O(n) for arg existence check
		if !params.iter().any(|p| p.name == name as &str) {
			throw!(UnknownFunctionParameter((name as &str).to_owned()));
		}
		if passed_args
			.insert(
				name,
				if tailstrict {
					LazyVal::new_resolved(evaluate(ctx.clone(), value)?)
				} else {
					LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
						context: ctx.clone(),
						expr: value.clone(),
					})))
				},
			)
			.is_some()
		{
			throw!(BindingParameterASecondTime(name.clone()));
		}
		filled_args += 1;
	}

	if filled_args < params.len() {
		for param in params.iter().filter(|p| p.has_default) {
			if passed_args.contains_key(&param.name) {
				continue;
			}
			filled_args += 1;
		}

		// Some args still wasn't filled
		if filled_args != params.len() {
			for param in params.iter().skip(args.unnamed.len()) {
				if !args.named.iter().any(|a| &a.0 as &str == param.name) {
					throw!(FunctionParameterNotBoundInCall(param.name.into()));
				}
			}
			unreachable!();
		}
	}
	Ok(passed_args)
}

pub fn parse_function_call_map(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &HashMap<IStr, Val>,
	tailstrict: bool,
) -> Result<Context> {
	let mut out = GcHashMap::with_capacity(params.len());
	let mut positioned_args = vec![None; params.0.len()];
	for (name, val) in args.iter() {
		let idx = params
			.iter()
			.position(|p| *p.0 == **name)
			.ok_or_else(|| UnknownFunctionParameter((name as &str).to_owned()))?;

		if idx >= params.len() {
			throw!(TooManyArgsFunctionHas(params.len()));
		}
		if positioned_args[idx].is_some() {
			throw!(BindingParameterASecondTime(params[idx].0.clone()));
		}
		positioned_args[idx] = Some(val.clone());
	}
	// Fill defaults
	for (id, p) in params.iter().enumerate() {
		let val = if let Some(arg) = positioned_args[id].take() {
			LazyVal::new_resolved(arg)
		} else if let Some(default) = &p.1 {
			if tailstrict {
				LazyVal::new_resolved(evaluate(
					body_ctx.clone().expect(NO_DEFAULT_CONTEXT),
					default,
				)?)
			} else {
				let body_ctx = body_ctx.clone();
				let default = default.clone();
				#[derive(Trace)]
				struct EvaluateLazyVal {
					body_ctx: Option<Context>,
					default: LocExpr,
				}
				impl LazyValValue for EvaluateLazyVal {
					fn get(self: Box<Self>) -> Result<Val> {
						evaluate(
							self.body_ctx.clone().expect(NO_DEFAULT_CONTEXT),
							&self.default,
						)
					}
				}
				LazyVal::new(TraceBox(Box::new(EvaluateLazyVal { body_ctx, default })))
			}
		} else {
			throw!(FunctionParameterNotBoundInCall(p.0.clone()));
		};
		out.insert(p.0.clone(), val);
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

pub fn place_args(body_ctx: Context, params: &ParamsDesc, args: &[Val]) -> Result<Context> {
	let mut out = GcHashMap::with_capacity(params.len());
	let mut positioned_args = vec![None; params.0.len()];
	for (id, arg) in args.iter().enumerate() {
		if id >= params.len() {
			throw!(TooManyArgsFunctionHas(params.len()));
		}
		positioned_args[id] = Some(arg);
	}
	// Fill defaults
	for (id, p) in params.iter().enumerate() {
		let val = if let Some(arg) = &positioned_args[id] {
			(*arg).clone()
		} else if let Some(default) = &p.1 {
			evaluate(body_ctx.clone(), default)?
		} else {
			throw!(FunctionParameterNotBoundInCall(p.0.clone()));
		};
		out.insert(p.0.clone(), LazyVal::new_resolved(val));
	}

	Ok(body_ctx.extend(out, None, None, None))
}
