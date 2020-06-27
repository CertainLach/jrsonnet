use crate::{
	context_creator, create_error, create_error_result, escape_string_json, future_wrapper,
	lazy_val, manifest_json_ex, parse_args, push, with_state, Context, ContextCreator, Error,
	FuncDesc, LazyBinding, LazyVal, ObjMember, ObjValue, Result, Val, ValType,
};
use closure::closure;
use jsonnet_parser::{
	AssertStmt, BinaryOpType, BindSpec, CompSpec, Expr, FieldMember, ForSpecData, IfSpecData,
	LiteralType, LocExpr, Member, ObjBody, ParamsDesc, UnaryOpType, Visibility,
};
use std::{
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

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
					push(&b.value.1, "thunk", ||{
						evaluate(
							context_creator.0(this.clone(), super_obj.clone())?,
							&b.value
						)
					})
				)))
			})),
		)
	}
}

pub fn evaluate_method(ctx: Context, params: ParamsDesc, body: LocExpr) -> Val {
	Val::Func(FuncDesc { ctx, params, body })
}

pub fn evaluate_field_name(
	context: Context,
	field_name: &jsonnet_parser::FieldName,
) -> Result<Option<Rc<str>>> {
	Ok(match field_name {
		jsonnet_parser::FieldName::Fixed(n) => Some(n.clone()),
		jsonnet_parser::FieldName::Dyn(expr) => {
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
		(op, o) => create_error_result(Error::UnaryOperatorDoesNotOperateOnType(
			op,
			o.value_type()?,
		))?,
	})
}

pub(crate) fn evaluate_add_op(a: &Val, b: &Val) -> Result<Val> {
	Ok(match (a, b) {
		(Val::Str(v1), Val::Str(v2)) => Val::Str(((**v1).to_owned() + &v2).into()),

		// Can't use generic json serialization way, because it depends on number to string concatenation (std.jsonnet:890)
		(Val::Num(n), Val::Str(o)) => Val::Str(format!("{}{}", n, o).into()),
		(Val::Str(o), Val::Num(n)) => Val::Str(format!("{}{}", o, n).into()),

		(Val::Str(s), o) => Val::Str(format!("{}{}", s, o.clone().into_json(0)?).into()),
		(o, Val::Str(s)) => Val::Str(format!("{}{}", o.clone().into_json(0)?, s).into()),

		(Val::Obj(v1), Val::Obj(v2)) => Val::Obj(v2.with_super(v1.clone())),
		(Val::Arr(a), Val::Arr(b)) => Val::Arr(Rc::new([&a[..], &b[..]].concat())),
		(Val::Num(v1), Val::Num(v2)) => Val::Num(v1 + v2),
		_ => create_error_result(Error::BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Add,
			a.value_type()?,
			b.value_type()?,
		))?,
	})
}

pub fn evaluate_binary_op_special(
	context: Context,
	a: &LocExpr,
	op: BinaryOpType,
	b: &LocExpr,
) -> Result<Val> {
	Ok(
		match (evaluate(context.clone(), &a)?.unwrap_if_lazy()?, op, b) {
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
		(Val::Num(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Num(v1 * v2),
		(Val::Num(v1), BinaryOpType::Div, Val::Num(v2)) => {
			if *v2 <= f64::EPSILON {
				create_error_result(crate::Error::DivisionByZero)?
			}
			Val::Num(v1 / v2)
		}

		(Val::Num(v1), BinaryOpType::Sub, Val::Num(v2)) => Val::Num(v1 - v2),

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
			Val::Num(((*v1 as i32) << (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::Rhs, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) >> (*v2 as i32)) as f64)
		}

		_ => create_error_result(Error::BinaryOperatorDoesNotOperateOnValues(
			op,
			a.value_type()?,
			b.value_type()?,
		))?,
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
			if evaluate(context.clone(), &cond)?.try_cast_bool("if spec")? {
				evaluate_comp(context, value, &specs[1..])?
			} else {
				None
			}
		}
		Some(CompSpec::ForSpec(ForSpecData(var, expr))) => {
			match evaluate(context.clone(), &expr)?.unwrap_if_lazy()? {
				Val::Arr(list) => {
					let mut out = Vec::new();
					for item in list.iter() {
						let item = item.unwrap_if_lazy()?;
						out.push(evaluate_comp(
							context.with_var(var.clone(), item.clone())?,
							value,
							&specs[1..],
						)?);
					}
					Some(out.into_iter().flatten().flatten().collect())
				}
				_ => panic!("for expression evaluated to non-iterable value"),
			}
		}
	})
}

pub fn evaluate_member_list_object(context: Context, members: &[Member]) -> Result<ObjValue> {
	let new_bindings = FutureNewBindings::new();
	let future_this = FutureObjValue::new();
	let context_creator = context_creator!(
		closure!(clone context, clone new_bindings, |this: Option<ObjValue>, super_obj: Option<ObjValue>| {
			Ok(context.extend_unbound(
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

	let mut new_members = BTreeMap::new();
	for member in members.iter() {
		match member {
			Member::Field(FieldMember {
				name,
				plus,
				params: None,
				visibility,
				value,
			}) => {
				let name = evaluate_field_name(context.clone(), &name)?;
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
								Ok(LazyVal::new_resolved(push(&value.1, "object field", ||{
									let context = context_creator.0(this, super_obj)?;
									evaluate(
										context,
										&value,
									)
								})?))
							}),
						)),
					},
				);
			}
			Member::Field(FieldMember {
				name,
				params: Some(params),
				value,
				..
			}) => {
				let name = evaluate_field_name(context.clone(), &name)?;
				if name.is_none() {
					continue;
				}
				let name = name.unwrap();
				new_members.insert(
					name,
					ObjMember {
						add: false,
						visibility: Visibility::Hidden,
						invoke: LazyBinding::Bindable(Rc::new(
							closure!(clone value, clone context_creator, clone params, |this, super_obj| {
								// TODO: Assert
								Ok(LazyVal::new_resolved(evaluate_method(
									context_creator.0(this, super_obj)?,
									params.clone(),
									value.clone(),
								)))
							}),
						)),
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
		ObjBody::MemberList(members) => evaluate_member_list_object(context, &members)?,
		ObjBody::ObjComp(obj) => {
			let future_this = FutureObjValue::new();
			let mut new_members = BTreeMap::new();
			for (k, v) in evaluate_comp(
				context.clone(),
				&|ctx| {
					let new_bindings = FutureNewBindings::new();
					let context_creator = context_creator!(
						closure!(clone context, clone new_bindings, |this: Option<ObjValue>, super_obj: Option<ObjValue>| {
							Ok(context.extend_unbound(
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
							Ok(LazyVal::new_resolved(evaluate(ctx.extend(HashMap::new(), None, this, None)?, &value)?))
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
							},
						);
					}
					v => create_error_result(Error::FieldMustBeStringGot(v.value_type()?))?,
				}
			}

			future_this.fill(ObjValue::new(None, Rc::new(new_members)))
		}
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
				.unwrap_or_else(|| panic!("this not found")),
		),
		Literal(LiteralType::Dollar) => Val::Obj(
			context
				.dollar()
				.clone()
				.unwrap_or_else(|| panic!("dollar not found")),
		),
		Literal(LiteralType::True) => Val::Bool(true),
		Literal(LiteralType::False) => Val::Bool(false),
		Literal(LiteralType::Null) => Val::Null,
		Parened(e) => evaluate(context, e)?,
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::Num(*v),
		BinaryOp(v1, o, v2) => evaluate_binary_op_special(context, &v1, *o, &v2)?,
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(context, v)?)?,
		Var(name) => push(loc, "var", || {
			Ok(Val::Lazy(context.binding(name.clone())?).unwrap_if_lazy()?)
		})?,
		Index(LocExpr(v, _), index) if matches!(&**v, Expr::Literal(LiteralType::Super)) => {
			let name = evaluate(context.clone(), index)?.try_cast_str("object index")?;
			context
				.super_obj()
				.clone()
				.expect("no super found")
				.get_raw(&name, &context.this().clone().expect("no this found"))?
				.expect("value not found")
		}
		Index(value, index) => {
			match (
				evaluate(context.clone(), value)?.unwrap_if_lazy()?,
				evaluate(context, index)?,
			) {
				(Val::Obj(v), Val::Str(s)) => {
					if let Some(v) = v.get(s.clone())? {
						v.unwrap_if_lazy()?
					} else if let Some(Val::Str(n)) = v.get("__intristic_namespace__".into())? {
						Val::Intristic(n, s)
					} else {
						create_error_result(crate::Error::NoSuchField(s))?
					}
				}
				(Val::Obj(_), n) => create_error_result(crate::Error::ValueIndexMustBeTypeGot(
					ValType::Obj,
					ValType::Str,
					n.value_type()?,
				))?,

				(Val::Arr(v), Val::Num(n)) => {
					if n.fract() > f64::EPSILON {
						create_error_result(crate::Error::FractionalIndex)?
					}
					v.get(n as usize)
						.ok_or_else(|| {
							create_error(crate::Error::ArrayBoundsError(n as usize, v.len()))
						})?
						.clone()
						.unwrap_if_lazy()?
				}
				(Val::Arr(_), Val::Str(n)) => {
					create_error_result(crate::Error::AttemptedIndexAnArrayWithString(n))?
				}
				(Val::Arr(_), n) => create_error_result(crate::Error::ValueIndexMustBeTypeGot(
					ValType::Arr,
					ValType::Num,
					n.value_type()?,
				))?,

				(Val::Str(s), Val::Num(n)) => Val::Str(
					s.chars()
						.skip(n as usize)
						.take(1)
						.collect::<String>()
						.into(),
				),
				(Val::Str(_), n) => create_error_result(crate::Error::ValueIndexMustBeTypeGot(
					ValType::Str,
					ValType::Num,
					n.value_type()?,
				))?,

				(v, _) => create_error_result(crate::Error::CantIndexInto(v.value_type()?))?,
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
		ArrComp(expr, compspecs) => Val::Arr(
			// First compspec should be forspec, so no "None" possible here
			Rc::new(evaluate_comp(context, &|ctx| evaluate(ctx, expr), compspecs)?.unwrap()),
		),
		Obj(body) => Val::Obj(evaluate_object(context, body)?),
		ObjExtend(s, t) => evaluate_add_op(
			&evaluate(context.clone(), s)?,
			&Val::Obj(evaluate_object(context, t)?),
		)?,
		Apply(value, args, tailstrict) => {
			let lazy = evaluate(context.clone(), value)?;
			let value = lazy.unwrap_if_lazy()?;
			match value {
				Val::Intristic(ns, name) => match (&ns as &str, &name as &str) {
					// arr/string/function
					("std", "length") => parse_args!(context, "std.length", args, 1, [
						0, x: [Val::Str|Val::Arr|Val::Obj], vec![ValType::Str, ValType::Arr, ValType::Obj];
					], {
						match x {
							Val::Str(n) => Val::Num(n.chars().count() as f64),
							Val::Arr(i) => Val::Num(i.len() as f64),
							Val::Obj(o) => Val::Num(
								o.fields_visibility()
									.into_iter()
									.filter(|(_k, v)| *v)
									.count() as f64,
							),
							_ => unreachable!(),
						}
					}),
					// any
					("std", "type") => parse_args!(context, "std.type", args, 1, [
						0, x, vec![];
					], {
						Val::Str(x.value_type()?.name().into())
					}),
					// length, idx=>any
					("std", "makeArray") => parse_args!(context, "std.makeArray", args, 2, [
						0, sz: [Val::Num]!!Val::Num, vec![ValType::Num];
						1, func: [Val::Func]!!Val::Func, vec![ValType::Func];
					], {
						assert!(sz >= 0.0);
						let mut out = Vec::with_capacity(sz as usize);
						for i in 0..sz as usize {
							out.push(func.evaluate_values(
								Context::new(),
								&[Val::Num(i as f64)]
							)?)
						}
						Val::Arr(Rc::new(out))
					}),
					// string
					("std", "codepoint") => parse_args!(context, "std.codepoint", args, 1, [
						0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
					], {
						assert!(
							str.chars().count() == 1,
							"std.codepoint should receive single char string"
						);
						Val::Num(str.chars().take(1).next().unwrap() as u32 as f64)
					}),
					// object, includeHidden
					("std", "objectFieldsEx") => {
						parse_args!(context, "std.objectFieldsEx",args, 2, [
							0, obj: [Val::Obj]!!Val::Obj, vec![ValType::Obj];
							1, inc_hidden: [Val::Bool]!!Val::Bool, vec![ValType::Bool];
						], {
							Val::Arr(Rc::new(
								obj.fields_visibility()
									.into_iter()
									.filter(|(_k, v)| *v || inc_hidden)
									.map(|(k, _v)| Val::Str(k))
									.collect(),
							))
						})
					}
					// object, field, includeHidden
					("std", "objectHasEx") => parse_args!(context, "std.objectHasEx", args, 3, [
						0, obj: [Val::Obj]!!Val::Obj, vec![ValType::Obj];
						1, f: [Val::Str]!!Val::Str, vec![ValType::Str];
						2, inc_hidden: [Val::Bool]!!Val::Bool, vec![ValType::Bool];
					], {
						Val::Bool(
							obj.fields_visibility()
								.into_iter()
								.filter(|(_k, v)| *v || inc_hidden)
								.any(|(k, _v)| *k == *f),
						)
					}),
					("std", "primitiveEquals") => {
						parse_args!(context, "std.primitiveEquals", args, 2, [
							0, a, vec![];
							1, b, vec![];
						], {
							Val::Bool(a == b)
						})
					}
					("std", "modulo") => parse_args!(context, "std.modulo", args, 2, [
						0, a: [Val::Num]!!Val::Num, vec![ValType::Num];
						1, b: [Val::Num]!!Val::Num, vec![ValType::Num];
					], {
						Val::Num(a % b)
					}),
					("std", "floor") => parse_args!(context, "std.floor", args, 1, [
						0, x: [Val::Num]!!Val::Num, vec![ValType::Num];
					], {
						Val::Num(x.floor())
					}),
					("std", "trace") => parse_args!(context, "std.trace", args, 2, [
						0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
						1, rest, vec![];
					], {
						// TODO: Line numbers as in original jsonnet
						println!("TRACE: {}", str);
						rest
					}),
					("std", "pow") => parse_args!(context, "std.modulo", args, 2, [
						0, x: [Val::Num]!!Val::Num, vec![ValType::Num];
						1, n: [Val::Num]!!Val::Num, vec![ValType::Num];
					], {
						Val::Num(x.powf(n))
					}),
					("std", "extVar") => parse_args!(context, "std.extVar", args, 2, [
						0, x: [Val::Str]!!Val::Str, vec![ValType::Str];
					], {
						with_state(|s| s.0.ext_vars.borrow().get(&x).cloned()).ok_or_else(
							|| create_error(crate::Error::UndefinedExternalVariable(x)),
						)?
					}),
					("std", "filter") => parse_args!(context, "std.filter", args, 2, [
						0, func: [Val::Func]!!Val::Func, vec![ValType::Func];
						1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
					], {
						Val::Arr(Rc::new(
							arr.iter()
								.cloned()
								.filter(|e| {
									func
										.evaluate_values(context.clone(), &[e.clone()])
										.unwrap()
										.try_cast_bool("filter predicate")
										.unwrap()
								})
								.collect(),
						))
					}),
					("std", "char") => parse_args!(context, "std.char", args, 1, [
						0, n: [Val::Num]!!Val::Num, vec![ValType::Num];
					], {
						let mut out = String::new();
						out.push(std::char::from_u32(n as u32).unwrap());
						Val::Str(out.into())
					}),
					("std", "encodeUTF8") => parse_args!(context, "std.encodeUtf8", args, 1, [
						0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
					], {
						Val::Arr(Rc::new(str.bytes().map(|b| Val::Num(b as f64)).collect()))
					}),
					("std", "md5") => parse_args!(context, "std.md5", args, 1, [
						0, str: [Val::Str]!!Val::Str, vec![ValType::Str];
					], {
						Val::Str(format!("{:x}", md5::compute(str.as_bytes())).into())
					}),
					// faster
					("std", "join") => parse_args!(context, "std.join", args, 2, [
						0, sep: [Val::Str|Val::Arr], vec![ValType::Str, ValType::Arr];
						1, arr: [Val::Arr]!!Val::Arr, vec![ValType::Arr];
					], {
						match sep {
							Val::Arr(joiner_items) => {
								let mut out = Vec::new();

								let mut first = true;
								for item in arr.iter().cloned() {
									if let Val::Arr(items) = item.unwrap_if_lazy()? {
										if !first {
											out.reserve(joiner_items.len());
											out.extend(joiner_items.iter().cloned());
										}
										first = false;
										out.reserve(items.len());
										out.extend(items.iter().cloned());
									} else {
										panic!("all array items should be arrays")
									}
								}

								Val::Arr(Rc::new(out))
							},
							Val::Str(sep) => {
								let mut out = String::new();

								let mut first = true;
								for item in arr.iter().cloned() {
									if let Val::Str(item) = item.unwrap_if_lazy()? {
										if !first {
											out += &sep;
										}
										first = false;
										out += &item;
									} else {
										panic!("all array items should be strings")
									}
								}

								Val::Str(out.into())
							},
							_ => unreachable!()
						}
					}),
					// Faster
					("std", "escapeStringJson") => {
						parse_args!(context, "std.escapeStringJson", args, 1, [
							0, str_: [Val::Str]!!Val::Str, vec![ValType::Str];
						], {
							Val::Str(escape_string_json(&str_).into())
						})
					}
					// Faster
					("std", "manifestJsonEx") => {
						parse_args!(context, "std.manifestJsonEx", args, 2, [
							0, value, vec![];
							1, indent: [Val::Str]!!Val::Str, vec![ValType::Str];
						], {
							Val::Str(manifest_json_ex(&value, &indent)?.into())
						})
					}
					(ns, name) => create_error_result(crate::Error::IntristicNotFound(
						ns.into(),
						name.into(),
					))?,
				},
				Val::Func(f) => {
					let body = || f.evaluate(context, args, *tailstrict);
					if *tailstrict {
						body()?
					} else {
						push(loc, "function call", body)?
					}
				}
				v => create_error_result(crate::Error::OnlyFunctionsCanBeCalledGot(
					v.value_type()?,
				))?,
			}
		}
		Function(params, body) => evaluate_method(context, params.clone(), body.clone()),
		AssertExpr(AssertStmt(value, msg), returned) => {
			let assertion_result = push(&value.1, "assertion condition", || {
				evaluate(context.clone(), &value)?
					.try_cast_bool("assertion condition should be boolean")
			})?;
			if assertion_result {
				evaluate(context, returned)?
			} else if let Some(msg) = msg {
				panic!(
					"assertion failed ({:?}): {}",
					value,
					evaluate(context, msg)?.try_cast_str("assertion message should be string")?
				);
			} else {
				panic!("assertion failed ({:?}): no message", value);
			}
		}
		Error(e) => create_error_result(crate::Error::RuntimeError(
			evaluate(context, e)?.try_cast_str("error text should be string")?,
		))?,
		IfElse {
			cond,
			cond_then,
			cond_else,
		} => {
			if evaluate(context.clone(), &cond.0)?
				.try_cast_bool("if condition should be boolean")?
			{
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
				.expect("imports can't be used without loc_data")
				.0;
			let import_location = Rc::make_mut(&mut tmp);
			import_location.pop();
			with_state(|s| s.import_file(&import_location, path))?
		}
		ImportStr(path) => {
			let mut tmp = loc
				.clone()
				.expect("imports can't be used without loc_data")
				.0;
			let import_location = Rc::make_mut(&mut tmp);
			import_location.pop();
			Val::Str(with_state(|s| s.import_file_str(&import_location, path))?)
		}
		Literal(LiteralType::Super) => {
			return create_error_result(crate::Error::StandaloneSuper)
		}
	})
}
