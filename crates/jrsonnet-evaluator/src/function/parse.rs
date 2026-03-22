use std::rc::Rc;

use jrsonnet_parser::{
	function::{FunctionSignature, ParamName},
	ArgsDesc, Expr, ExprParams, Spanned,
};
use rustc_hash::FxHashMap;

use crate::{
	bail,
	destructure::destruct,
	error::{ErrorKind::*, Result},
	evaluate, evaluate_named_param,
	gc::WithCapacityExt as _,
	Context, Pending, Thunk, Val,
};

fn eval_arg(ctx: Context, arg: &Rc<Spanned<Expr>>, tailstrict: bool) -> Result<Thunk<Val>> {
	if tailstrict {
		Ok(Thunk::evaluated(evaluate(ctx, arg)?))
	} else {
		let arg = arg.clone();
		Ok(Thunk!(move || evaluate(ctx, &arg)))
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
pub(crate) fn parse_function_call(
	ctx: Context,
	body_ctx: Context,
	params: &ExprParams,
	args: &ArgsDesc,
	tailstrict: bool,
) -> Result<Context> {
	let mut passed_args = FxHashMap::with_capacity(params.binds_len());
	if args.unnamed.len() > params.signature.len() {
		bail!(TooManyArgsFunctionHas(
			params.signature.len(),
			params.signature.clone(),
		))
	}

	let mut filled_named = 0;
	let mut filled_positionals = 0;

	for (id, arg) in args.unnamed.iter().enumerate() {
		destruct(
			&params.exprs[id].destruct,
			eval_arg(ctx.clone(), arg, tailstrict)?,
			Pending::new_filled(ctx.clone()),
			&mut passed_args,
		)?;
		filled_positionals += 1;
	}

	for (name, value) in &args.named {
		// FIXME: O(n) for arg existence check
		if !params.exprs.iter().any(|p| &p.destruct.name() == name) {
			bail!(UnknownFunctionParameter(name.clone()));
		}
		if passed_args
			.insert(name.clone(), eval_arg(ctx.clone(), value, tailstrict)?)
			.is_some()
		{
			bail!(BindingParameterASecondTime(name.clone()));
		}
		filled_named += 1;
	}

	if filled_named + filled_positionals < params.len() {
		// Some args are unset, but maybe we have defaults for them
		// Default values should be created in newly created context
		let fctx = Context::new_future();
		let mut defaults =
			FxHashMap::with_capacity(params.binds_len() - filled_named - filled_positionals);

		for (idx, into, default) in params
			.exprs
			.iter()
			.enumerate()
			.filter_map(|(i, p)| Some((i, &p.destruct, p.default.as_ref()?)))
		{
			if let ParamName::Named(name) = into.name() {
				if passed_args.contains_key(&name) {
					continue;
				}
			} else if idx < filled_positionals {
				continue;
			}

			destruct(
				into,
				{
					let ctx = fctx.clone();
					let name = into.name();
					let value = default.clone();
					Thunk!(move || evaluate_named_param(ctx.unwrap(), &value, name))
				},
				fctx.clone(),
				&mut defaults,
			)?;
			if into.name().is_named() {
				filled_named += 1;
			} else {
				filled_positionals += 1;
			}
		}

		// Some args still weren't filled
		if filled_named + filled_positionals != params.len() {
			for param in params.exprs.iter().skip(args.unnamed.len()) {
				let mut found = false;
				for (name, _) in &args.named {
					if &param.destruct.name() == name {
						found = true;
					}
				}
				if !found {
					bail!(FunctionParameterNotBoundInCall(
						param.destruct.name(),
						params.signature.clone()
					));
				}
			}
			unreachable!();
		}

		Ok(body_ctx
			.extend_bindings(passed_args)
			.extend_bindings(defaults)
			.into_future(fctx))
	} else {
		let body_ctx = body_ctx.extend_bindings(passed_args);
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
	ctx: Context,
	params: FunctionSignature,
	args: &ArgsDesc,
	tailstrict: bool,
) -> Result<Vec<Option<Thunk<Val>>>> {
	let mut passed_args: Vec<Option<Thunk<Val>>> = vec![None; params.len()];
	if args.unnamed.len() > params.len() {
		bail!(TooManyArgsFunctionHas(params.len(), params,))
	}

	let mut filled_args = 0;

	for (id, arg) in args.unnamed.iter().enumerate() {
		passed_args[id] = Some(eval_arg(ctx.clone(), arg, tailstrict)?);
		filled_args += 1;
	}

	for (name, arg) in &args.named {
		// FIXME: O(n) for arg existence check
		let id = params
			.iter()
			.position(|p| p.name() == name)
			.ok_or_else(|| UnknownFunctionParameter(name.clone()))?;
		if passed_args[id]
			.replace(eval_arg(ctx.clone(), arg, tailstrict)?)
			.is_some()
		{
			bail!(BindingParameterASecondTime(name.clone()));
		}
		filled_args += 1;
	}

	if filled_args < params.len() {
		for (id, _) in params.iter().enumerate().filter(|(_, p)| p.has_default()) {
			if passed_args[id].is_some() {
				continue;
			}
			filled_args += 1;
		}

		// Some args still wasn't filled
		if filled_args != params.len() {
			for param in params.iter().skip(args.unnamed.len()) {
				let mut found = false;
				for (name, _) in &args.named {
					if param.name() == name {
						found = true;
					}
				}
				if !found {
					bail!(FunctionParameterNotBoundInCall(
						param.name().clone(),
						params,
					));
				}
			}
			unreachable!();
		}
	}
	Ok(passed_args)
}

/// Creates Context, which has all argument default values applied
/// and with unbound values causing error to be returned
pub fn parse_default_function_call(body_ctx: Context, params: &ExprParams) -> Result<Context> {
	let fctx = Context::new_future();

	let mut bindings = FxHashMap::with_capacity(params.binds_len());

	for param in params.exprs.iter() {
		if let Some(v) = &param.default {
			destruct(
				&param.destruct.clone(),
				{
					let ctx = fctx.clone();
					let name = param.destruct.name();
					let value = v.clone();
					Thunk!(move || evaluate_named_param(ctx.unwrap(), &value, name))
				},
				fctx.clone(),
				&mut bindings,
			)?;
		} else {
			destruct(
				&param.destruct,
				{
					let param_name = param.destruct.name();
					let params = params.clone();
					Thunk!(move || Err(FunctionParameterNotBoundInCall(
						param_name,
						params.signature
					)
					.into()))
				},
				fctx.clone(),
				&mut bindings,
			)?;
		}
	}

	Ok(body_ctx.extend_bindings(bindings).into_future(fctx))
}
