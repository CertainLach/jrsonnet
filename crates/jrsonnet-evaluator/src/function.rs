use crate::{Context, FutureWrapper, GcHashMap, LazyVal, LazyValValue, Result, Val, error::Error::*, evaluate, evaluate_named, gc::TraceBox, throw};
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
				LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
					body_ctx,
					default,
				})))
			}
		} else {
			throw!(FunctionParameterNotBoundInCall(p.0.clone()));
		};
		out.insert(p.0.clone(), val);
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

pub fn place_args(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &[Val],
) -> Result<Context> {
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
			evaluate(ctx.clone(), default)?
		} else {
			throw!(FunctionParameterNotBoundInCall(p.0.clone()));
		};
		out.insert(p.0.clone(), LazyVal::new_resolved(val));
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

#[macro_export]
macro_rules! parse_args {
	($ctx: expr, $fn_name: expr, $args: expr, $total_args: expr, [
		$($id: expr, $name: ident: $ty: expr $(=>$match: path)?);+ $(;)?
	], $handler:block) => {{
		use $crate::{error::Error::*, throw, evaluate, push_description_frame, typed::CheckType};

		let args = $args;
		if args.unnamed.len() + args.named.len() > $total_args {
			throw!(TooManyArgsFunctionHas($total_args));
		}
		$(
			if args.unnamed.len() + args.named.len() <= $id {
				throw!(FunctionParameterNotBoundInCall(stringify!($name).into()));
			}
			// Is named
			let $name = if $id >= $args.unnamed.len() {
				let named = &args.named[$id - $args.unnamed.len()];
				if &named.0 != stringify!($name) {
					throw!(IntrinsicArgumentReorderingIsNotSupportedYet);
				}
				&named.1
			} else {
				&$args.unnamed[$id]
			};
			let $name = push_description_frame(|| format!("evaluating builtin argument {}", stringify!($name)), || {
				let value = evaluate($ctx.clone(), &$name)?;
				$ty.check(&value)?;
				Ok(value)
			})?;
			$(
				let $name = if let $match(v) = $name {
					v
				} else {
					unreachable!();
				};
			)?
		)+
		($handler as crate::Result<_>)
	}};
}
