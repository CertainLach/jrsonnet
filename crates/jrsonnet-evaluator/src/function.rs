use crate::{error::Error::*, evaluate, lazy_val, resolved_lazy_val, throw, Context, Result, Val};
use closure::closure;
use jrsonnet_parser::{ArgsDesc, ParamsDesc};
use std::{collections::HashMap, rc::Rc};

const NO_DEFAULT_CONTEXT: &str =
	"no default context set for call with defined default parameter value";

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
	let mut out = HashMap::with_capacity(params.len());
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
			resolved_lazy_val!(evaluate(ctx, expr)?)
		} else {
			lazy_val!(closure!(clone ctx, clone expr, ||evaluate(ctx.clone(), &expr)))
		};
		out.insert(p.0.clone(), val);
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

pub fn parse_function_call_map(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &HashMap<Rc<str>, Val>,
	tailstrict: bool,
) -> Result<Context> {
	let mut out = HashMap::with_capacity(params.len());
	let mut positioned_args = vec![None; params.0.len()];
	for (name, val) in args.iter() {
		let idx = params
			.iter()
			.position(|p| *p.0 == **name)
			.ok_or_else(|| UnknownFunctionParameter((&name as &str).to_owned()))?;

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
			resolved_lazy_val!(arg)
		} else if let Some(default) = &p.1 {
			if tailstrict {
				resolved_lazy_val!(evaluate(
					body_ctx.clone().expect(NO_DEFAULT_CONTEXT),
					default
				)?)
			} else {
				let body_ctx = body_ctx.clone();
				let default = default.clone();
				lazy_val!(move || {
					evaluate(body_ctx.clone().expect(NO_DEFAULT_CONTEXT), &default)
				})
			}
		} else {
			throw!(FunctionParameterNotBoundInCall(p.0.clone()));
		};
		out.insert(p.0.clone(), val);
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

pub(crate) fn place_args(
	ctx: Context,
	body_ctx: Option<Context>,
	params: &ParamsDesc,
	args: &[Val],
) -> Result<Context> {
	let mut out = HashMap::with_capacity(params.len());
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
		out.insert(p.0.clone(), resolved_lazy_val!(val));
	}

	Ok(body_ctx.unwrap_or(ctx).extend(out, None, None, None))
}

#[macro_export]
macro_rules! parse_args {
	($ctx: expr, $fn_name: expr, $args: expr, $total_args: expr, [
		$($id: expr, $name: ident $(: [$($p: path)|+] $(!! $a: path)?)?, $nt: expr);+ $(;)?
	], $handler:block) => {{
		use crate::{throw, error::Error::*};
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
					throw!(IntristicArgumentReorderingIsNotSupportedYet);
				}
			}
			let $name = evaluate($ctx.clone(), &$name.1)?;
			$(
				match $name {
					$($p(_))|+ => {},
					_ => throw!(TypeMismatch(
						concat!($fn_name, " ", stringify!($id), "nd (", stringify!($name), ") argument"),
						$nt, $name.value_type()?
					)),
				};
				$(
					let $name = match $name {
						$a(v) => v,
						_ =>throw!(TypeMismatch(concat!($fn_name, " ", stringify!($id), "nd (", stringify!($name), ") argument"), $nt, $name.value_type()?)),
					};
				)*
			)*
		)+
		($handler as crate::Result<_>)
	}};
}

#[test]
fn test() -> Result<()> {
	use crate::val::ValType;
	use jrsonnet_parser::*;
	let state = crate::EvaluationState::default();
	let evaluator = state.with_stdlib();
	let ctx = evaluator.create_default_context()?;
	evaluator.run_in_state(|| {
		parse_args!(ctx, "test", ArgsDesc(vec![
			Arg(None, el!(Expr::Num(2.0))),
			Arg(Some("b".into()), el!(Expr::Num(1.0))),
		]), 2, [
			0, a: [Val::Num]!!Val::Num, vec![ValType::Num];
			1, b: [Val::Num]!!Val::Num, vec![ValType::Num];
		], {
			assert!((a - 2.0).abs() <= f64::EPSILON);
			assert!((b - 1.0).abs() <= f64::EPSILON);
			Ok(())
		})
		.unwrap();
		Ok(())
	})
}
