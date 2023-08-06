use std::mem::replace;

use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{LocExpr, ParamsDesc};

use super::{arglike::ArgsLike, builtin::BuiltinParam};
use crate::{
	destructure::destruct,
	error::{ErrorKind::*, Result},
	evaluate_named,
	gc::GcHashMap,
	throw,
	val::ThunkValue,
	Context, Pending, Thunk, Val,
};

#[derive(Trace)]
struct EvaluateNamedThunk {
	ctx: Pending<Context>,
	name: IStr,
	value: LocExpr,
}

impl ThunkValue for EvaluateNamedThunk {
	type Output = Val;
	fn get(self: Box<Self>) -> Result<Val> {
		evaluate_named(self.ctx.unwrap(), &self.value, self.name)
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
	args: &dyn ArgsLike,
	tailstrict: bool,
) -> Result<Context> {
	let mut passed_args =
		GcHashMap::with_capacity(params.iter().map(|p| p.0.capacity_hint()).sum());
	if args.unnamed_len() > params.len() {
		throw!(TooManyArgsFunctionHas(
			params.len(),
			params.iter().map(|p| (p.0.name(), p.1.is_some())).collect()
		))
	}

	let mut filled_named = 0;
	let mut filled_positionals = 0;

	args.unnamed_iter(ctx.clone(), tailstrict, &mut |id, arg| {
		let name = params[id].0.clone();
		destruct(
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
			throw!(UnknownFunctionParameter((name as &str).to_owned()));
		}
		if passed_args.insert(name.clone(), value).is_some() {
			throw!(BindingParameterASecondTime(name.clone()));
		}
		filled_named += 1;
		Ok(())
	})?;

	if filled_named + filled_positionals < params.len() {
		// Some args are unset, but maybe we have defaults for them
		// Default values should be created in newly created context
		let fctx = Context::new_future();
		let mut defaults = GcHashMap::with_capacity(
			params.iter().map(|p| p.0.capacity_hint()).sum::<usize>()
				- filled_named - filled_positionals,
		);

		for (idx, param) in params.iter().enumerate().filter(|p| p.1 .1.is_some()) {
			if let Some(name) = param.0.name() {
				if passed_args.contains_key(&name) {
					continue;
				}
			} else if idx < filled_positionals {
				continue;
			}

			destruct(
				&param.0,
				Thunk::new(EvaluateNamedThunk {
					ctx: fctx.clone(),
					name: param.0.name().unwrap_or_else(|| "<destruct>".into()),
					value: param.1.clone().expect("default exists"),
				}),
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
					throw!(FunctionParameterNotBoundInCall(
						param.0.clone().name(),
						params.iter().map(|p| (p.0.name(), p.1.is_some())).collect()
					));
				}
			}
			unreachable!();
		}

		Ok(body_ctx
			.extend(passed_args, None, None, None)
			.extend(defaults, None, None, None)
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
	ctx: Context,
	params: &[BuiltinParam],
	args: &dyn ArgsLike,
	tailstrict: bool,
) -> Result<Vec<Option<Thunk<Val>>>> {
	let mut passed_args: Vec<Option<Thunk<Val>>> = vec![None; params.len()];
	if args.unnamed_len() > params.len() {
		throw!(TooManyArgsFunctionHas(
			params.len(),
			params
				.iter()
				.map(|p| (p.name().as_str().map(IStr::from), p.has_default()))
				.collect()
		))
	}

	let mut filled_args = 0;

	args.unnamed_iter(ctx.clone(), tailstrict, &mut |id, arg| {
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
			throw!(BindingParameterASecondTime(name.clone()));
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
					throw!(FunctionParameterNotBoundInCall(
						param.name().as_str().map(IStr::from),
						params
							.iter()
							.map(|p| (p.name().as_str().map(IStr::from), p.has_default()))
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
	#[derive(Trace)]
	struct DependsOnUnbound(IStr, ParamsDesc);
	impl ThunkValue for DependsOnUnbound {
		type Output = Val;
		fn get(self: Box<Self>) -> Result<Val> {
			Err(FunctionParameterNotBoundInCall(
				Some(self.0.clone()),
				self.1.iter().map(|p| (p.0.name(), p.1.is_some())).collect(),
			)
			.into())
		}
	}

	let fctx = Context::new_future();

	let mut bindings = GcHashMap::with_capacity(params.iter().map(|p| p.0.capacity_hint()).sum());

	for param in params.iter() {
		if let Some(v) = &param.1 {
			destruct(
				&param.0.clone(),
				Thunk::new(EvaluateNamedThunk {
					ctx: fctx.clone(),
					name: param.0.name().unwrap_or_else(|| "<destruct>".into()),
					value: v.clone(),
				}),
				fctx.clone(),
				&mut bindings,
			)?;
		} else {
			destruct(
				&param.0,
				Thunk::new(DependsOnUnbound(
					param.0.name().unwrap_or_else(|| "<destruct>".into()),
					params.clone(),
				)),
				fctx.clone(),
				&mut bindings,
			)?;
		}
	}

	Ok(body_ctx
		.extend(bindings, None, None, None)
		.into_future(fctx))
}
