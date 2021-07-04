use crate::{error::Error::*, evaluate, throw, Context, LazyVal, LazyValValue, Result, Val};
use jrsonnet_gc::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, LocExpr, ParamsDesc};
use rustc_hash::FxHashMap;
use std::{collections::HashMap, hash::BuildHasherDefault};

const NO_DEFAULT_CONTEXT: &str =
	"no default context set for call with defined default parameter value";

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
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &ArgsDesc,
	tailstrict: bool,
) -> Result<Context> {
	let mut out = HashMap::with_capacity_and_hasher(params.len(), BuildHasherDefault::default());
	let mut positioned_args = vec![None; params.0.len()];
	for (id, arg) in args.iter().enumerate() {
		let idx = if let Some(name) = &arg.0 {
			params
				.iter()
				.position(|p| *p.0 == *name)
				.ok_or_else(|| UnknownFunctionParameter(name.clone()))?
		} else {
			id
		};

		if idx >= params.len() {
			throw!(TooManyArgsFunctionHas(params.len()));
		}
		if positioned_args[idx].is_some() {
			throw!(BindingParameterASecondTime(params[idx].0.clone()));
		}
		positioned_args[idx] = Some(arg.1.clone());
	}
	// Fill defaults
	for (id, p) in params.iter().enumerate() {
		let (ctx, expr) = if let Some(arg) = &positioned_args[id] {
			(ctx.clone(), arg)
		} else if let Some(default) = &p.1 {
			(body_ctx.clone().expect(NO_DEFAULT_CONTEXT), default)
		} else {
			throw!(FunctionParameterNotBoundInCall(p.0.clone()));
		};
		let val = if tailstrict {
			LazyVal::new_resolved(evaluate(ctx, expr)?)
		} else {
			#[derive(Trace)]
			#[trivially_drop]
			struct EvaluateLazyVal {
				context: Context,
				expr: LocExpr,
			}
			impl LazyValValue for EvaluateLazyVal {
				fn get(self: Box<Self>) -> Result<Val> {
					evaluate(self.context, &self.expr)
				}
			}

			LazyVal::new(Box::new(EvaluateLazyVal {
				context: ctx.clone(),
				expr: expr.clone(),
			}))
		};
		out.insert(p.0.clone(), val);
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

pub fn parse_function_call_map(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &HashMap<IStr, Val>,
	tailstrict: bool,
) -> Result<Context> {
	let mut out = FxHashMap::with_capacity_and_hasher(params.len(), BuildHasherDefault::default());
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
				#[trivially_drop]
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
				LazyVal::new(Box::new(EvaluateLazyVal { body_ctx, default }))
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
	let mut out = FxHashMap::with_capacity_and_hasher(params.len(), BuildHasherDefault::default());
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
		use $crate::{error::Error::*, throw, evaluate, push_stack_frame, typed::CheckType};

		let args = $args;
		if args.len() > $total_args {
			throw!(TooManyArgsFunctionHas($total_args));
		}
		$(
			if args.len() <= $id {
				throw!(FunctionParameterNotBoundInCall(stringify!($name).into()));
			}
			let $name = &args[$id];
			if $name.0.is_some() {
				if $name.0.as_ref().unwrap() != stringify!($name) {
					throw!(IntrinsicArgumentReorderingIsNotSupportedYet);
				}
			}
			let $name = push_stack_frame(None, || format!("evaluating argument"), || {
				let value = evaluate($ctx.clone(), &$name.1)?;
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
