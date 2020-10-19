use crate::{
	context_creator, error::Error::*, future_wrapper, lazy_val, push, throw, with_state, Context,
	ContextCreator, FuncDesc, FuncVal, LazyBinding, LazyVal, ObjMember, ObjValue, Result, Val,
	ValType,
};
use closure::closure;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BinaryOpType, BindSpec, CompSpec, Expr, ExprLocation, FieldMember,
	ForSpecData, IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc, UnaryOpType,
	Visibility,
};
use rustc_hash::FxHashMap;
use std::{collections::HashMap, rc::Rc};

pub fn evaluate_binding(b: &BindSpec, context_creator: ContextCreator) -> (Rc<str>, LazyBinding) {
	let b = b.clone();
	if let Some(params) = &b.params {
		let params = params.clone();
		(
			b.name.clone(),
			LazyBinding::Bindable(Rc::new(move |this, super_obj| {
				Ok(lazy_val!(
					closure!(clone b, clone params, clone context_creator, || Ok(evaluate_method(
						context_creator.0(this.clone(), super_obj.clone())?,
						b.name.clone(),
						params.clone(),
						b.value.clone(),
					)))
				))
			})),
		)
	} else {
		(
			b.name.clone(),
			LazyBinding::Bindable(Rc::new(move |this, super_obj| {
				Ok(lazy_val!(closure!(clone context_creator, clone b, ||
						evaluate_named(
							context_creator.0(this.clone(), super_obj.clone())?,
							&b.value,
							b.name.clone()
						)
				)))
			})),
		)
	}
}

pub fn evaluate_method(ctx: Context, name: Rc<str>, params: ParamsDesc, body: LocExpr) -> Val {
	Val::Func(Rc::new(FuncVal::Normal(FuncDesc {
		name,
		ctx,
		params,
		body,
	})))
}

pub fn evaluate_field_name(
	context: Context,
	field_name: &jrsonnet_parser::FieldName,
) -> Result<Option<Rc<str>>> {
	Ok(match field_name {
		jrsonnet_parser::FieldName::Fixed(n) => Some(n.clone()),
		jrsonnet_parser::FieldName::Dyn(expr) => {
			let lazy = evaluate(context, expr)?;
			let value = lazy.unwrap_if_lazy()?;
			if matches!(value, Val::Null) {
				None
			} else {
				Some(value.try_cast_str("dynamic field name")?)
			}
		}
	})
}

pub fn evaluate_unary_op(op: UnaryOpType, b: &Val) -> Result<Val> {
	Ok(match (op, b) {
		(o, Val::Lazy(l)) => evaluate_unary_op(o, &l.evaluate()?)?,
		(UnaryOpType::Not, Val::Bool(v)) => Val::Bool(!v),
		(UnaryOpType::Minus, Val::Num(n)) => Val::Num(-*n),
		(UnaryOpType::BitNot, Val::Num(n)) => Val::Num(!(*n as i32) as f64),
		(op, o) => throw!(UnaryOperatorDoesNotOperateOnType(op, o.value_type()?)),
	})
}

pub fn evaluate_add_op(a: &Val, b: &Val) -> Result<Val> {
	Ok(match (a, b) {
		(Val::Str(v1), Val::Str(v2)) => Val::Str(((**v1).to_owned() + v2).into()),

		// Can't use generic json serialization way, because it depends on number to string concatenation (std.jsonnet:890)
		(Val::Num(n), Val::Str(o)) => Val::Str(format!("{}{}", n, o).into()),
		(Val::Str(o), Val::Num(n)) => Val::Str(format!("{}{}", o, n).into()),

		(Val::Str(s), o) => Val::Str(format!("{}{}", s, o.clone().to_string()?).into()),
		(o, Val::Str(s)) => Val::Str(format!("{}{}", o.clone().to_string()?, s).into()),

		(Val::Obj(v1), Val::Obj(v2)) => Val::Obj(v2.with_super(v1.clone())),
		(Val::Arr(a), Val::Arr(b)) => Val::Arr(Rc::new([&a[..], &b[..]].concat())),
		(Val::Num(v1), Val::Num(v2)) => Val::new_checked_num(v1 + v2)?,
		_ => throw!(BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Add,
			a.value_type()?,
			b.value_type()?,
		)),
	})
}

pub fn evaluate_binary_op_special(
	context: Context,
	a: &LocExpr,
	op: BinaryOpType,
	b: &LocExpr,
) -> Result<Val> {
	Ok(
		match (evaluate(context.clone(), a)?.unwrap_if_lazy()?, op, b) {
			(Val::Bool(true), BinaryOpType::Or, _o) => Val::Bool(true),
			(Val::Bool(false), BinaryOpType::And, _o) => Val::Bool(false),
			(a, op, eb) => {
				evaluate_binary_op_normal(&a, op, &evaluate(context, eb)?.unwrap_if_lazy()?)?
			}
		},
	)
}

pub fn evaluate_binary_op_normal(a: &Val, op: BinaryOpType, b: &Val) -> Result<Val> {
	Ok(match (a, op, b) {
		(a, BinaryOpType::Add, b) => evaluate_add_op(a, b)?,

		(Val::Str(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Str(v1.repeat(*v2 as usize).into()),

		// Bool X Bool
		(Val::Bool(a), BinaryOpType::And, Val::Bool(b)) => Val::Bool(*a && *b),
		(Val::Bool(a), BinaryOpType::Or, Val::Bool(b)) => Val::Bool(*a || *b),

		// Str X Str
		(Val::Str(v1), BinaryOpType::Lt, Val::Str(v2)) => Val::Bool(v1 < v2),
		(Val::Str(v1), BinaryOpType::Gt, Val::Str(v2)) => Val::Bool(v1 > v2),
		(Val::Str(v1), BinaryOpType::Lte, Val::Str(v2)) => Val::Bool(v1 <= v2),
		(Val::Str(v1), BinaryOpType::Gte, Val::Str(v2)) => Val::Bool(v1 >= v2),

		// Num X Num
		(Val::Num(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::new_checked_num(v1 * v2)?,
		(Val::Num(v1), BinaryOpType::Div, Val::Num(v2)) => {
			if *v2 <= f64::EPSILON {
				throw!(DivisionByZero)
			}
			Val::new_checked_num(v1 / v2)?
		}

		(Val::Num(v1), BinaryOpType::Sub, Val::Num(v2)) => Val::new_checked_num(v1 - v2)?,

		(Val::Num(v1), BinaryOpType::Lt, Val::Num(v2)) => Val::Bool(v1 < v2),
		(Val::Num(v1), BinaryOpType::Gt, Val::Num(v2)) => Val::Bool(v1 > v2),
		(Val::Num(v1), BinaryOpType::Lte, Val::Num(v2)) => Val::Bool(v1 <= v2),
		(Val::Num(v1), BinaryOpType::Gte, Val::Num(v2)) => Val::Bool(v1 >= v2),

		(Val::Num(v1), BinaryOpType::BitAnd, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) & (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::BitOr, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) | (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::BitXor, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) ^ (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::Lhs, Val::Num(v2)) => {
			if *v2 < 0.0 {
				throw!(RuntimeError("shift by negative exponent".into()))
			}
			Val::Num(((*v1 as i32) << (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::Rhs, Val::Num(v2)) => {
			if *v2 < 0.0 {
				throw!(RuntimeError("shift by negative exponent".into()))
			}
			Val::Num(((*v1 as i32) >> (*v2 as i32)) as f64)
		}

		_ => throw!(BinaryOperatorDoesNotOperateOnValues(
			op,
			a.value_type()?,
			b.value_type()?,
		)),
	})
}

future_wrapper!(HashMap<Rc<str>, LazyBinding>, FutureNewBindings);
future_wrapper!(ObjValue, FutureObjValue);

pub fn evaluate_comp<T>(
	context: Context,
	value: &impl Fn(Context) -> Result<T>,
	specs: &[CompSpec],
) -> Result<Option<Vec<T>>> {
	Ok(match specs.get(0) {
		None => Some(vec![value(context)?]),
		Some(CompSpec::IfSpec(IfSpecData(cond))) => {
			if evaluate(context.clone(), cond)?.try_cast_bool("if spec")? {
				evaluate_comp(context, value, &specs[1..])?
			} else {
				None
			}
		}
		Some(CompSpec::ForSpec(ForSpecData(var, expr))) => {
			match evaluate(context.clone(), expr)?.unwrap_if_lazy()? {
				Val::Arr(list) => {
					let mut out = Vec::new();
					for item in list.iter() {
						let item = item.unwrap_if_lazy()?;
						out.push(evaluate_comp(
							context.clone().with_var(var.clone(), item.clone()),
							value,
							&specs[1..],
						)?);
					}
					Some(out.into_iter().flatten().flatten().collect())
				}
				_ => throw!(InComprehensionCanOnlyIterateOverArray),
			}
		}
	})
}

pub fn evaluate_member_list_object(context: Context, members: &[Member]) -> Result<ObjValue> {
	let new_bindings = FutureNewBindings::new();
	let future_this = FutureObjValue::new();
	let context_creator = context_creator!(
		closure!(clone context, clone new_bindings, |this: Option<ObjValue>, super_obj: Option<ObjValue>| {
			Ok(context.clone().extend_unbound(
				new_bindings.clone().unwrap(),
				context.dollar().clone().or_else(||this.clone()),
				Some(this.unwrap()),
				super_obj
			)?)
		})
	);
	{
		let mut bindings: HashMap<Rc<str>, LazyBinding> = HashMap::new();
		for (n, b) in members
			.iter()
			.filter_map(|m| match m {
				Member::BindStmt(b) => Some(b.clone()),
				_ => None,
			})
			.map(|b| evaluate_binding(&b, context_creator.clone()))
		{
			bindings.insert(n, b);
		}
		new_bindings.fill(bindings);
	}

	let mut new_members = HashMap::new();
	for member in members.iter() {
		match member {
			Member::Field(FieldMember {
				name,
				plus,
				params: None,
				visibility,
				value,
			}) => {
				let name = evaluate_field_name(context.clone(), name)?;
				if name.is_none() {
					continue;
				}
				let name = name.unwrap();
				new_members.insert(
					name.clone(),
					ObjMember {
						add: *plus,
						visibility: *visibility,
						invoke: LazyBinding::Bindable(Rc::new(
							closure!(clone name, clone value, clone context_creator, |this, super_obj| {
								Ok(LazyVal::new_resolved(evaluate(
									context_creator.0(this, super_obj)?,
									&value,
								)?))
							}),
						)),
						location: value.1.clone(),
					},
				);
			}
			Member::Field(FieldMember {
				name,
				params: Some(params),
				value,
				..
			}) => {
				let name = evaluate_field_name(context.clone(), name)?;
				if name.is_none() {
					continue;
				}
				let name = name.unwrap();
				new_members.insert(
					name.clone(),
					ObjMember {
						add: false,
						visibility: Visibility::Hidden,
						invoke: LazyBinding::Bindable(Rc::new(
							closure!(clone value, clone context_creator, clone params, clone name, |this, super_obj| {
								// TODO: Assert
								Ok(LazyVal::new_resolved(evaluate_method(
									context_creator.0(this, super_obj)?,
									name.clone(),
									params.clone(),
									value.clone(),
								)))
							}),
						)),
						location: value.1.clone(),
					},
				);
			}
			Member::BindStmt(_) => {}
			Member::AssertStmt(_) => {}
		}
	}
	Ok(future_this.fill(ObjValue::new(None, Rc::new(new_members))))
}

pub fn evaluate_object(context: Context, object: &ObjBody) -> Result<ObjValue> {
	Ok(match object {
		ObjBody::MemberList(members) => evaluate_member_list_object(context, members)?,
		ObjBody::ObjComp(obj) => {
			let future_this = FutureObjValue::new();
			let mut new_members = HashMap::new();
			for (k, v) in evaluate_comp(
				context.clone(),
				&|ctx| {
					let new_bindings = FutureNewBindings::new();
					let context_creator = context_creator!(
						closure!(clone context, clone new_bindings, |this: Option<ObjValue>, super_obj: Option<ObjValue>| {
							Ok(context.clone().extend_unbound(
								new_bindings.clone().unwrap(),
								context.dollar().clone().or_else(||this.clone()),
								None,
								super_obj
							)?)
						})
					);
					let mut bindings: HashMap<Rc<str>, LazyBinding> = HashMap::new();
					for (n, b) in obj
						.pre_locals
						.iter()
						.chain(obj.post_locals.iter())
						.map(|b| evaluate_binding(b, context_creator.clone()))
					{
						bindings.insert(n, b);
					}
					let bindings = new_bindings.fill(bindings);
					let ctx = ctx.extend_unbound(bindings, None, None, None)?;
					let key = evaluate(ctx.clone(), &obj.key)?;
					let value = LazyBinding::Bindable(Rc::new(
						closure!(clone ctx, clone obj.value, |this, _super_obj| {
							Ok(LazyVal::new_resolved(evaluate(ctx.clone().extend(FxHashMap::default(), None, this, None), &value)?))
						}),
					));

					Ok((key, value))
				},
				&obj.compspecs,
			)?
			.unwrap()
			{
				match k {
					Val::Null => {}
					Val::Str(n) => {
						new_members.insert(
							n,
							ObjMember {
								add: false,
								visibility: Visibility::Normal,
								invoke: v,
								location: obj.value.1.clone(),
							},
						);
					}
					v => throw!(FieldMustBeStringGot(v.value_type()?)),
				}
			}

			future_this.fill(ObjValue::new(None, Rc::new(new_members)))
		}
	})
}

pub fn evaluate_apply(
	context: Context,
	value: &LocExpr,
	args: &ArgsDesc,
	loc: &Option<ExprLocation>,
	tailstrict: bool,
) -> Result<Val> {
	let lazy = evaluate(context.clone(), value)?;
	let value = lazy.unwrap_if_lazy()?;
	Ok(match value {
		Val::Func(f) => {
			let body = || f.evaluate(context, loc, args, tailstrict);
			if tailstrict {
				body()?
			} else {
				push(loc, || format!("function <{}> call", f.name()), body)?
			}
		}
		v => throw!(OnlyFunctionsCanBeCalledGot(v.value_type()?)),
	})
}

pub fn evaluate_named(context: Context, lexpr: &LocExpr, name: Rc<str>) -> Result<Val> {
	use Expr::*;
	let LocExpr(expr, _loc) = lexpr;
	Ok(match &**expr {
		Function(params, body) => evaluate_method(context, name, params.clone(), body.clone()),
		_ => evaluate(context, lexpr)?,
	})
}

pub fn evaluate(context: Context, expr: &LocExpr) -> Result<Val> {
	use Expr::*;
	let LocExpr(expr, loc) = expr;
	Ok(match &**expr {
		Literal(LiteralType::This) => Val::Obj(
			context
				.this()
				.clone()
				.ok_or_else(|| CantUseSelfOutsideOfObject)?,
		),
		Literal(LiteralType::Dollar) => Val::Obj(
			context
				.dollar()
				.clone()
				.ok_or_else(|| NoTopLevelObjectFound)?,
		),
		Literal(LiteralType::True) => Val::Bool(true),
		Literal(LiteralType::False) => Val::Bool(false),
		Literal(LiteralType::Null) => Val::Null,
		Parened(e) => evaluate(context, e)?,
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::new_checked_num(*v)?,
		BinaryOp(v1, o, v2) => evaluate_binary_op_special(context, v1, *o, v2)?,
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(context, v)?)?,
		Var(name) => push(
			loc,
			|| format!("variable <{}>", name),
			|| Ok(Val::Lazy(context.binding(name.clone())?).unwrap_if_lazy()?),
		)?,
		Index(LocExpr(v, _), index) if matches!(&**v, Expr::Literal(LiteralType::Super)) => {
			let name = evaluate(context.clone(), index)?.try_cast_str("object index")?;
			context
				.super_obj()
				.clone()
				.expect("no super found")
				.get_raw(name, &context.this().clone().expect("no this found"))?
				.expect("value not found")
		}
		Index(value, index) => {
			match (
				evaluate(context.clone(), value)?.unwrap_if_lazy()?,
				evaluate(context, index)?,
			) {
				(Val::Obj(v), Val::Str(s)) => {
					let sn = s.clone();
					push(
						loc,
						|| format!("field <{}> access", sn),
						|| {
							if let Some(v) = v.get(s.clone())? {
								Ok(v.unwrap_if_lazy()?)
							} else if v.get("__intrinsic_namespace__".into())?.is_some() {
								Ok(Val::Func(Rc::new(FuncVal::Intrinsic(s))))
							} else {
								throw!(NoSuchField(s))
							}
						},
					)?
				}
				(Val::Obj(_), n) => throw!(ValueIndexMustBeTypeGot(
					ValType::Obj,
					ValType::Str,
					n.value_type()?,
				)),

				(Val::Arr(v), Val::Num(n)) => {
					if n.fract() > f64::EPSILON {
						throw!(FractionalIndex)
					}
					v.get(n as usize)
						.ok_or_else(|| ArrayBoundsError(n as usize, v.len()))?
						.clone()
						.unwrap_if_lazy()?
				}
				(Val::Arr(_), Val::Str(n)) => throw!(AttemptedIndexAnArrayWithString(n)),
				(Val::Arr(_), n) => throw!(ValueIndexMustBeTypeGot(
					ValType::Arr,
					ValType::Num,
					n.value_type()?,
				)),

				(Val::Str(s), Val::Num(n)) => Val::Str(
					s.chars()
						.skip(n as usize)
						.take(1)
						.collect::<String>()
						.into(),
				),
				(Val::Str(_), n) => throw!(ValueIndexMustBeTypeGot(
					ValType::Str,
					ValType::Num,
					n.value_type()?,
				)),

				(v, _) => throw!(CantIndexInto(v.value_type()?)),
			}
		}
		LocalExpr(bindings, returned) => {
			let mut new_bindings: HashMap<Rc<str>, LazyBinding> = HashMap::new();
			let future_context = Context::new_future();

			let context_creator = context_creator!(
				closure!(clone future_context, |_, _| Ok(future_context.clone().unwrap()))
			);

			for (k, v) in bindings
				.iter()
				.map(|b| evaluate_binding(b, context_creator.clone()))
			{
				new_bindings.insert(k, v);
			}

			let context = context
				.extend_unbound(new_bindings, None, None, None)?
				.into_future(future_context);
			evaluate(context, &returned.clone())?
		}
		Arr(items) => {
			let mut out = Vec::with_capacity(items.len());
			for item in items {
				out.push(Val::Lazy(lazy_val!(
					closure!(clone context, clone item, || {
						evaluate(context.clone(), &item)
					})
				)));
			}
			Val::Arr(Rc::new(out))
		}
		ArrComp(expr, comp_specs) => Val::Arr(
			// First comp_spec should be for_spec, so no "None" possible here
			Rc::new(evaluate_comp(context, &|ctx| evaluate(ctx, expr), comp_specs)?.unwrap()),
		),
		Obj(body) => Val::Obj(evaluate_object(context, body)?),
		ObjExtend(s, t) => evaluate_add_op(
			&evaluate(context.clone(), s)?,
			&Val::Obj(evaluate_object(context, t)?),
		)?,
		Apply(value, args, tailstrict) => evaluate_apply(context, value, args, loc, *tailstrict)?,
		Function(params, body) => {
			evaluate_method(context, "anonymous".into(), params.clone(), body.clone())
		}
		Intrinsic(name) => Val::Func(Rc::new(FuncVal::Intrinsic(name.clone()))),
		AssertExpr(AssertStmt(value, msg), returned) => {
			let assertion_result = push(
				&value.1,
				|| "assertion condition".to_owned(),
				|| {
					evaluate(context.clone(), value)?
						.try_cast_bool("assertion condition should be of type `boolean`")
				},
			)?;
			if assertion_result {
				evaluate(context, returned)?
			} else if let Some(msg) = msg {
				throw!(AssertionFailed(evaluate(context, msg)?.to_string()?));
			} else {
				throw!(AssertionFailed(Val::Null.to_string()?));
			}
		}
		ErrorStmt(e) => push(
			loc,
			|| "error statement".to_owned(),
			|| {
				throw!(RuntimeError(
					evaluate(context, e)?.try_cast_str("error text should be of type `string`")?,
				))
			},
		)?,
		IfElse {
			cond,
			cond_then,
			cond_else,
		} => {
			if push(
				loc,
				|| "if condition".to_owned(),
				|| evaluate(context.clone(), &cond.0)?.try_cast_bool("in if condition"),
			)? {
				evaluate(context, cond_then)?
			} else {
				match cond_else {
					Some(v) => evaluate(context, v)?,
					None => Val::Null,
				}
			}
		}
		Import(path) => {
			let mut tmp = loc
				.clone()
				.expect("imports cannot be used without loc_data")
				.0;
			let import_location = Rc::make_mut(&mut tmp);
			import_location.pop();
			push(
				loc,
				|| format!("import {:?}", path),
				|| with_state(|s| s.import_file(import_location, path)),
			)?
		}
		ImportStr(path) => {
			let mut tmp = loc
				.clone()
				.expect("imports cannot be used without loc_data")
				.0;
			let import_location = Rc::make_mut(&mut tmp);
			import_location.pop();
			Val::Str(with_state(|s| s.import_file_str(import_location, path))?)
		}
		Literal(LiteralType::Super) => throw!(StandaloneSuper),
	})
}
