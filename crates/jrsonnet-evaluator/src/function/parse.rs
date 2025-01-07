use std::mem::replace;

use jrsonnet_interner::IStr;
use jrsonnet_parser::ParamsDesc;

use super::{ArgsLike, Param};
use crate::{
	bail,
	destructure::destruct_lazy,
	error::{ErrorKind::*, Result},
	evaluate_named,
	function::ParamDefault,
	BindingValue, BindingsMap, Context, ContextBuilder, Pending, Thunk,
};

/// Creates correct [context](Context) for function body evaluation returning error on invalid call.
///
/// ## Parameters
/// * `ctx`: used for passed argument expressions' execution and for body execution (if `body_ctx` is not set)
/// * `body_ctx`: used for default parameter values' execution and for body execution (if set)
/// * `params`: function parameters' definition
/// * `args`: passed function arguments
/// * `tailstrict`: if set to `true` function arguments are eagerly executed, otherwise - lazily
pub fn parse_function_call(
	ctx: &Context,
	body_ctx: Context,
	params: &ParamsDesc,
	args: &impl ArgsLike,
	tailstrict: bool,
) -> Result<Context> {
	let mut passed_args =
		BindingsMap::with_capacity(params.iter().map(|p| p.0.capacity_hint()).sum());
	if args.unnamed_len() > params.len() {
		bail!(TooManyArgsFunctionHas(
			params.len(),
			params
				.iter()
				.map(|p| (p.0.name(), ParamDefault::exists(p.1.is_some())))
				.collect()
		))
	}

	let mut filled_named = 0;
	let mut filled_positionals = 0;

	args.unnamed_iter(ctx, tailstrict, &mut |id, arg| {
		let name = params[id].0.clone();
		destruct_lazy(
			&name,
			arg,
			Pending::new_filled(ctx.clone()),
			&mut passed_args,
		)?;
		filled_positionals += 1;
		Ok(())
	})?;

	args.named_iter(ctx, tailstrict, &mut |name, value| {
		// FIXME: O(n) for arg existence check
		if !params.iter().any(|p| p.0.name().as_ref() == Some(name)) {
			bail!(UnknownFunctionParameter((name as &str).to_owned()));
		}
		if !passed_args.insert(name.clone(), value) {
			bail!(BindingParameterASecondTime(name.clone()));
		}
		filled_named += 1;
		Ok(())
	})?;

	if filled_named + filled_positionals < params.len() {
		// Some args are unset, but maybe we have defaults for them
		// Default values should be created in newly created context
		let fctx = Context::new_future();
		let mut defaults = BindingsMap::with_capacity(
			params.iter().map(|p| p.0.capacity_hint()).sum::<usize>()
				- filled_named
				- filled_positionals,
		);

		for (idx, param) in params.iter().enumerate().filter(|p| p.1 .1.is_some()) {
			if let Some(name) = param.0.name() {
				if passed_args.contains_key(&name) {
					continue;
				}
			} else if idx < filled_positionals {
				continue;
			}

			destruct_lazy(
				&param.0,
				{
					let ctx = fctx.clone();
					let name = param.0.name().unwrap_or_else(|| "<destruct>".into());
					let value = param.1.clone().expect("default exists");
					Thunk!(move || evaluate_named(ctx.get(), &value, name))
				},
				fctx.clone(),
				&mut defaults,
			)?;
			if param.0.name().is_some() {
				filled_named += 1;
			} else {
				filled_positionals += 1;
			}
		}

		// Some args still weren't filled
		if filled_named + filled_positionals != params.len() {
			for param in params.iter().skip(args.unnamed_len()) {
				let mut found = false;
				args.named_names(&mut |name| {
					if Some(name) == param.0.name().as_ref() {
						found = true;
					}
				});
				if !found {
					bail!(FunctionParameterNotBoundInCall(
						param.0.clone().name(),
						params
							.iter()
							.map(|p| (p.0.name(), ParamDefault::exists(p.1.is_some())))
							.collect()
					));
				}
			}
			unreachable!();
		}

		let mut ctx = ContextBuilder::extend(body_ctx);
		ctx.binds(passed_args).binds(defaults);
		Ok(ctx.build().into_future(fctx))
	} else {
		let mut ctx = ContextBuilder::extend(body_ctx);
		ctx.binds(passed_args);
		Ok(ctx.build())
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
	ctx: &Context,
	params: &[Param],
	args: &dyn ArgsLike,
	tailstrict: bool,
) -> Result<Vec<Option<BindingValue>>> {
	let mut passed_args: Vec<Option<BindingValue>> = vec![None; params.len()];
	if args.unnamed_len() > params.len() {
		bail!(TooManyArgsFunctionHas(
			params.len(),
			params
				.iter()
				.map(|p| (p.name().as_str().map(IStr::from), p.default()))
				.collect()
		))
	}

	let mut filled_args = 0;

	args.unnamed_iter(ctx, tailstrict, &mut |id, arg| {
		passed_args[id] = Some(arg);
		filled_args += 1;
		Ok(())
	})?;

	args.named_iter(ctx, tailstrict, &mut |name, arg| {
		// FIXME: O(n) for arg existence check
		let id = params
			.iter()
			.position(|p| p.name() == name)
			.ok_or_else(|| UnknownFunctionParameter((name as &str).to_owned()))?;
		if replace(&mut passed_args[id], Some(arg)).is_some() {
			bail!(BindingParameterASecondTime(name.clone()));
		}
		filled_args += 1;
		Ok(())
	})?;

	if filled_args < params.len() {
		for (id, _) in params.iter().enumerate().filter(|(_, p)| p.has_default()) {
			if passed_args[id].is_some() {
				continue;
			}
			filled_args += 1;
		}

		// Some args still wasn't filled
		if filled_args != params.len() {
			for param in params.iter().skip(args.unnamed_len()) {
				let mut found = false;
				args.named_names(&mut |name| {
					if param.name() == name {
						found = true;
					}
				});
				if !found {
					bail!(FunctionParameterNotBoundInCall(
						param.name().as_str().map(IStr::from),
						params
							.iter()
							.map(|p| (p.name().as_str().map(IStr::from), p.default()))
							.collect()
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
pub fn parse_default_function_call(body_ctx: Context, params: &ParamsDesc) -> Result<Context> {
	let fctx = Context::new_future();

	let mut bindings = BindingsMap::with_capacity(params.iter().map(|p| p.0.capacity_hint()).sum());

	for param in params.iter() {
		if let Some(v) = &param.1 {
			destruct_lazy(
				&param.0.clone(),
				{
					let ctx = fctx.clone();
					let name = param.0.name().unwrap_or_else(|| "<destruct>".into());
					let value = v.clone();
					BindingValue::Thunk(Thunk!(move || evaluate_named(ctx.get(), &value, name)))
				},
				fctx.clone(),
				&mut bindings,
			)?;
		} else {
			destruct_lazy(
				&param.0,
				{
					let param_name = param.0.name().unwrap_or_else(|| "<destruct>".into());
					let params = params.clone();
					BindingValue::Thunk(Thunk!(move || Err(FunctionParameterNotBoundInCall(
						Some(param_name),
						params
							.iter()
							.map(|p| (p.0.name(), ParamDefault::exists(p.1.is_some())))
							.collect(),
					)
					.into())))
				},
				fctx.clone(),
				&mut bindings,
			)?;
		}
	}

	Ok(body_ctx.with_bindings(bindings).into_future(fctx))
}
