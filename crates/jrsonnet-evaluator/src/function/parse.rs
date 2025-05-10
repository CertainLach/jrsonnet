use std::mem::replace;

use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ParamsDesc, RcVecExt};

use super::{ArgsLike, Param};
use crate::{
	bail,
	destructure::destruct_lazy,
	error::{ErrorKind::*, Result},
	evaluate_named,
	function::{ParamDefault, ParamName},
	gc::GcHashSet,
	BindingValue, BindingsMap, Context, ContextBuilder, Pending, Thunk,
};
#[derive(Default, Debug, Trace)]
pub struct PreparedCall {
	// Param, named input.
	named: Vec<(usize, usize)>,
	defaults: Vec<usize>,
}
pub fn prepare_call(params: &[Param], unnamed: usize, named: &[IStr]) -> Result<PreparedCall> {
	if unnamed > params.len() {
		bail!(TooManyArgsFunctionHas(
			params.len(),
			params
				.iter()
				.map(|p| (p.name().clone(), ParamDefault::exists(p.has_default())))
				.collect()
		))
	}

	let expected_defaults = params.len() - unnamed - named.len();
	let mut ops = PreparedCall {
		named: Vec::with_capacity(named.len()),
		defaults: Vec::with_capacity(expected_defaults),
	};

	// FIXME: bitmask
	let mut passed: GcHashSet<usize> = GcHashSet((0..unnamed).collect());

	for (input_id, name) in named.iter().enumerate() {
		// FIXME: O(n) for arg existence check
		let Some(param_idx) = params.iter().position(|p| p.name() == name) else {
			bail!(UnknownFunctionParameter(name.to_string()));
		};
		if !passed.insert(param_idx) {
			bail!(BindingParameterASecondTime(name.clone()));
		}
		ops.named.push((param_idx, input_id));
	}

	if named.len() + unnamed < params.len() {
		let mut defaults = 0;

		for (param_id, param) in params
			.iter()
			.enumerate()
			.skip(unnamed)
			.filter(|p| p.1.has_default())
		{
			// Skip already passed parameters
			if !param.name().is_anonymous() && passed.contains(&param_id) {
				continue;
			}
			defaults += 1;

			ops.defaults.push(param_id);
		}

		// Some args still weren't filled
		if defaults != expected_defaults {
			for param in params.iter().skip(unnamed) {
				let mut found = false;
				for name in named {
					if param.name() == name {
						found = true;
					}
				}
				if !found {
					bail!(FunctionParameterNotBoundInCall(
						param.name().clone(),
						params
							.iter()
							.map(|p| (p.name().clone(), ParamDefault::exists(p.has_default())))
							.collect()
					));
				}
			}
			unreachable!();
		}
	}

	Ok(ops)
}
pub fn parse_prepared_function_call(
	body_ctx: Context,
	prepared: &PreparedCall,
	params: &ParamsDesc,
	unnamed: &[BindingValue],
	named: &[BindingValue],
) -> Result<Context> {
	let mut passed_args =
		BindingsMap::with_capacity(params.iter().map(|p| p.0.capacity_hint()).sum());

	let destruct_ctx = Pending::new();

	for (param_idx, unnamed) in unnamed.iter().enumerate() {
		let name = params[param_idx].0.clone();
		destruct_lazy(
			&name,
			unnamed.clone(),
			destruct_ctx.clone(),
			&mut passed_args,
		)?;
	}

	for (param_idx, arg_idx) in prepared.named.iter().copied() {
		let name = params[param_idx].0.clone();
		destruct_lazy(
			&name,
			named[arg_idx].clone(),
			destruct_ctx.clone(),
			&mut passed_args,
		)?;
	}

	if prepared.defaults.is_empty() {
		let body_ctx = body_ctx
			.with_bindings(passed_args)
			.into_future(destruct_ctx);
		Ok(body_ctx)
	} else {
		let fctx = Context::new_future();
		let mut defaults = BindingsMap::with_capacity(
			params.iter().map(|p| p.0.capacity_hint()).sum::<usize>() - passed_args.len(),
		);
		for param_idx in prepared.defaults.iter().copied() {
			let param = params.0.rc_idx(param_idx);
			destruct_lazy(
				&param.0,
				{
					let ctx = fctx.clone();
					let param = param.clone();
					Thunk!(move || {
						let name = param.0.name().unwrap_or_else(|| "<destruct>".into());
						let value = param.1.as_ref().expect("default exists");
						evaluate_named(ctx.get(), value, name)
					})
				},
				fctx.clone(),
				&mut defaults,
			)?;
		}

		let mut ctx = ContextBuilder::extend(body_ctx);
		ctx.binds(passed_args);
		ctx.binds(defaults);
		Ok(ctx.build().into_future(fctx).into_future(destruct_ctx))
	}
}
pub fn parse_prepared_builtin_call(
	prepared: &PreparedCall,
	params: &[Param],
	unnamed: &[BindingValue],
	named: &[BindingValue],
) -> Result<Vec<Option<BindingValue>>> {
	let mut passed_args = vec![None; params.len()];

	for (param_idx, unnamed) in unnamed.iter().enumerate() {
		passed_args[param_idx] = Some(unnamed.clone());
	}

	for (param_idx, arg_idx) in prepared.named.iter().copied() {
		passed_args[param_idx] = Some(named[arg_idx].clone());
	}

	Ok(passed_args)
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
				.map(|p| (ParamName(p.0.name()), ParamDefault::exists(p.1.is_some())))
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
						ParamName(param.0.name()),
						params
							.iter()
							.map(|p| (ParamName(p.0.name()), ParamDefault::exists(p.1.is_some())))
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
				.map(|p| (p.name().clone(), p.default()))
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
						param.name().clone(),
						params
							.iter()
							.map(|p| (p.name().clone(), p.default()))
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
						ParamName::new(param_name),
						params
							.iter()
							.map(|p| (ParamName(p.0.name()), ParamDefault::exists(p.1.is_some())))
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
