use crate::{create_error, evaluate, lazy_val, resolved_lazy_val, Context, Error, Result};
use closure::closure;
use jsonnet_parser::{ArgsDesc, ParamsDesc};
use std::collections::HashMap;

/// Creates correct [context](Context) for function body evaluation, returning error on invalid call
///
/// * `ctx` used for passed argument expressions execution, and for body execution (if `body_ctx` is not set)
/// * `body_ctx` used for default parameter values execution, and for body execution (if set)
/// * `params` function parameters definition
/// * `args` passed function arguments
/// * `tailstruct` if true - function arguments is eager executed, otherwise - lazy
pub fn parse_function_call(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &ArgsDesc,
	tailstrict: bool,
) -> Result<Context> {
	inline_parse_function_call(ctx, body_ctx, params, args, tailstrict)
}

/// See [parse_function_call](parse_function_call)
///
/// ## Notes
/// This function is always inlined for tailstrict
#[inline(always)]
pub(crate) fn inline_parse_function_call(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &ArgsDesc,
	tailstrict: bool,
) -> Result<Context> {
	let mut out = HashMap::new();
	let mut positioned_args = vec![None; params.0.len()];
	for (id, arg) in args.iter().enumerate() {
		let idx = if let Some(name) = &arg.0 {
			params.iter().position(|p| &p.0 == name).ok_or_else(|| {
				create_error::<()>(Error::UnknownFunctionParameter(name.clone()))
					.err()
					.unwrap()
			})?
		} else {
			id
		};

		if idx >= params.len() {
			create_error(Error::TooManyArgsFunctionHas(params.len()))?;
		}
		if positioned_args[idx].is_some() {
			create_error(Error::BindingParameterASecondTime(params[idx].0.clone()))?;
		}
		positioned_args[idx] = Some(arg.1.clone());
	}
	// Fill defaults
	for (id, p) in params.iter().enumerate() {
		let (ctx, expr) = if let Some(arg) = &positioned_args[id] {
			(ctx.clone(), arg)
		} else if let Some(default) = &p.1 {
			(
				body_ctx
					.clone()
					.expect("no default context set for call with defined default parameter value"),
				default,
			)
		} else {
			create_error(Error::FunctionParameterNotBoundInCall(p.0.clone()))?;
			unreachable!()
		};
		let val = if tailstrict {
			resolved_lazy_val!(evaluate(ctx.clone(), expr)?)
		} else {
			lazy_val!(closure!(clone ctx, clone expr, ||evaluate(ctx.clone(), &expr)))
		};
		out.insert(p.0.clone(), val);
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None)?)
}
