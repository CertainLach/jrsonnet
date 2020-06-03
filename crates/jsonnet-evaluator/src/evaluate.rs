use crate::{
	binding, bool_val, context_creator, function_default, function_rhs, future_wrapper,
	lazy_binding, lazy_val, Context, ContextCreator, EvaluationState, FuncDesc, LazyBinding,
	ObjMember, ObjValue, Val,
};
use closure::closure;
use jsonnet_parser::{
	el, Arg, ArgsDesc, AssertStmt, BinaryOpType, BindSpec, CompSpec, Expr, FieldMember,
	ForSpecData, IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc, UnaryOpType,
	Visibility,
};
use std::{
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

pub fn evaluate_binding(
	eval_state: EvaluationState,
	b: &BindSpec,
	context_creator: ContextCreator,
) -> (String, LazyBinding) {
	let b = b.clone();
	if let Some(args) = &b.params {
		let args = args.clone();
		(
			b.name.clone(),
			lazy_binding!(move |this, super_obj| lazy_val!(
				closure!(clone b, clone args, clone context_creator, clone eval_state, || evaluate_method(
					context_creator.0(this.clone(), super_obj.clone()),
					eval_state.clone(),
					&b.value,
					args.clone()
				))
			)),
		)
	} else {
		(
			b.name.clone(),
			lazy_binding!(move |this, super_obj| {
				lazy_val!(
					closure!(clone context_creator, clone b, clone eval_state, || evaluate(
						context_creator.0(this.clone(), super_obj.clone()),
						eval_state.clone(),
						&b.value
					))
				)
			}),
		)
	}
}

pub fn evaluate_method(
	ctx: Context,
	eval_state: EvaluationState,
	expr: &LocExpr,
	arg_spec: ParamsDesc,
) -> Val {
	Val::Func(FuncDesc {
		ctx,
		params: arg_spec,
		eval_rhs: function_rhs!(
			closure!(clone expr, clone eval_state, |ctx| evaluate(ctx, eval_state.clone(), &expr))
		),
		eval_default: function_default!(
			closure!(clone eval_state, |ctx, default| evaluate(ctx, eval_state.clone(), &default))
		),
	})
}

pub fn evaluate_field_name(
	context: Context,
	eval_state: EvaluationState,
	field_name: &jsonnet_parser::FieldName,
) -> String {
	match field_name {
		jsonnet_parser::FieldName::Fixed(n) => n.clone(),
		jsonnet_parser::FieldName::Dyn(expr) => {
			let name = evaluate(context, eval_state, expr).unwrap_if_lazy();
			match name {
				Val::Str(n) => n,
				_ => panic!(
					"dynamic field name can be only evaluated to 'string', got: {:?}",
					name
				),
			}
		}
	}
}

pub fn evaluate_unary_op(op: UnaryOpType, b: &Val) -> Val {
	match (op, b) {
		(o, Val::Lazy(l)) => evaluate_unary_op(o, &l.evaluate()),
		(UnaryOpType::Not, Val::Bool(v)) => Val::Bool(!v),
		(op, o) => panic!("unary op not implemented: {:?} {:?}", op, o),
	}
}

pub fn evaluate_add_op(a: &Val, b: &Val) -> Val {
	match (a, b) {
		(Val::Str(v1), Val::Str(v2)) => Val::Str(v1.to_owned() + &v2),
		(Val::Str(v1), Val::Num(v2)) => Val::Str(format!("{}{}", v1, v2)),
		(Val::Num(v1), Val::Str(v2)) => Val::Str(format!("{}{}", v1, v2)),
		(Val::Obj(v1), Val::Obj(v2)) => Val::Obj(v2.with_super(v1.clone())),
		(Val::Arr(a), Val::Arr(b)) => Val::Arr([&a[..], &b[..]].concat()),
		(Val::Num(v1), Val::Num(v2)) => Val::Num(v1 + v2),
		_ => panic!("can't add: {:?} and {:?}", a, b),
	}
}

pub fn evaluate_binary_op(
	context: Context,
	eval_state: EvaluationState,
	a: &Val,
	op: BinaryOpType,
	b: &Val,
) -> Val {
	match (a, op, b) {
		(Val::Lazy(a), o, b) => evaluate_binary_op(context, eval_state, &a.evaluate(), o, b),
		(a, o, Val::Lazy(b)) => evaluate_binary_op(context, eval_state, a, o, &b.evaluate()),

		(a, BinaryOpType::Add, b) => evaluate_add_op(a, b),

		(Val::Str(v1), BinaryOpType::Ne, Val::Str(v2)) => bool_val(v1 != v2),

		(Val::Str(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Str(v1.repeat(*v2 as usize)),
		(Val::Str(format), BinaryOpType::Mod, args) => evaluate(
			context
				.with_var("__tmp__format__".to_owned(), Val::Str(format.to_owned()))
				.with_var(
					"__tmp__args__".to_owned(),
					match args {
						Val::Arr(v) => Val::Arr(v.clone()),
						v => Val::Arr(vec![v.clone()]),
					},
				),
			eval_state,
			&el!(Expr::Apply(
				el!(Expr::Index(
					el!(Expr::Var("std".to_owned())),
					el!(Expr::Str("format".to_owned()))
				)),
				ArgsDesc(vec![
					Arg(None, el!(Expr::Var("__tmp__format__".to_owned()))),
					Arg(None, el!(Expr::Var("__tmp__args__".to_owned())))
				])
			)),
		),

		(Val::Bool(a), BinaryOpType::And, Val::Bool(b)) => Val::Bool(*a && *b),
		(Val::Bool(a), BinaryOpType::Or, Val::Bool(b)) => Val::Bool(*a || *b),

		(Val::Num(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Num(v1 * v2),
		(Val::Num(v1), BinaryOpType::Div, Val::Num(v2)) => Val::Num(v1 / v2),
		(Val::Num(v1), BinaryOpType::Mod, Val::Num(v2)) => Val::Num(v1 % v2),

		(Val::Num(v1), BinaryOpType::Sub, Val::Num(v2)) => Val::Num(v1 - v2),

		(Val::Num(v1), BinaryOpType::Lhs, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) << (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::Rhs, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) >> (*v2 as i32)) as f64)
		}

		(Val::Num(v1), BinaryOpType::Lt, Val::Num(v2)) => bool_val(v1 < v2),
		(Val::Num(v1), BinaryOpType::Gt, Val::Num(v2)) => bool_val(v1 > v2),
		(Val::Num(v1), BinaryOpType::Lte, Val::Num(v2)) => bool_val(v1 <= v2),
		(Val::Num(v1), BinaryOpType::Gte, Val::Num(v2)) => bool_val(v1 >= v2),

		(Val::Num(v1), BinaryOpType::Eq, Val::Num(v2)) => bool_val((v1 - v2).abs() < f64::EPSILON),
		(Val::Num(v1), BinaryOpType::Ne, Val::Num(v2)) => bool_val((v1 - v2).abs() > f64::EPSILON),

		(Val::Num(v1), BinaryOpType::BitAnd, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) & (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::BitOr, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) | (*v2 as i32)) as f64)
		}
		(Val::Num(v1), BinaryOpType::BitXor, Val::Num(v2)) => {
			Val::Num(((*v1 as i32) ^ (*v2 as i32)) as f64)
		}
		(a, BinaryOpType::Eq, b) => bool_val(a == b),
		(a, BinaryOpType::Ne, b) => bool_val(a != b),
		_ => panic!("no rules for binary operation: {:?} {:?} {:?}", a, op, b),
	}
}

future_wrapper!(HashMap<String, LazyBinding>, FutureNewBindings);
future_wrapper!(ObjValue, FutureObjValue);

pub fn evaluate_comp(
	context: Context,
	eval_state: EvaluationState,
	value: &LocExpr,
	specs: &[CompSpec],
) -> Option<Vec<Val>> {
	match specs.get(0) {
		None => Some(vec![evaluate(context, eval_state, &value)]),
		Some(CompSpec::IfSpec(IfSpecData(cond))) => {
			match evaluate(context.clone(), eval_state.clone(), &cond).unwrap_if_lazy() {
				Val::Bool(false) => None,
				Val::Bool(true) => evaluate_comp(context, eval_state, value, &specs[1..]),
				_ => panic!("if expression evaluated to non-boolean value"),
			}
		}
		Some(CompSpec::ForSpec(ForSpecData(var, expr))) => {
			match evaluate(context.clone(), eval_state.clone(), &expr).unwrap_if_lazy() {
				Val::Arr(list) => {
					let mut out = Vec::new();
					for item in list {
						let item = item.clone();
						out.push(evaluate_comp(
							context.with_var(var.clone(), item),
							eval_state.clone(),
							value,
							&specs[1..],
						));
					}
					Some(out.iter().flatten().flatten().cloned().collect())
				}
				_ => panic!("for expression evaluated to non-iterable value"),
			}
		}
	}
}

// TODO: Asserts
pub fn evaluate_object(context: Context, eval_state: EvaluationState, object: ObjBody) -> ObjValue {
	match object {
		ObjBody::MemberList(members) => {
			let new_bindings = FutureNewBindings::new();
			let future_this = FutureObjValue::new();
			let context_creator = context_creator!(
				closure!(clone context, clone new_bindings, clone future_this, |this: Option<ObjValue>, super_obj: Option<ObjValue>| {
					context.clone().extend(
						new_bindings.clone().unwrap(),
						context.clone().dollar().clone().or_else(||this.clone()),
						Some(this.unwrap()),
						super_obj
					)
				})
			);
			{
				let mut bindings: HashMap<String, LazyBinding> = HashMap::new();
				for (n, b) in members
					.iter()
					.filter_map(|m| match m {
						Member::BindStmt(b) => Some(b.clone()),
						_ => None,
					})
					.map(|b| evaluate_binding(eval_state.clone(), &b, context_creator.clone()))
				{
					bindings.insert(n, b);
				}
				new_bindings.fill(bindings);
			}

			let mut new_members = BTreeMap::new();
			for member in members.into_iter() {
				match member {
					Member::Field(FieldMember {
						name,
						plus,
						params: None,
						visibility,
						value,
					}) => {
						let name = evaluate_field_name(context.clone(), eval_state.clone(), &name);
						new_members.insert(
							name,
							ObjMember {
								add: plus,
								visibility: visibility.clone(),
								invoke: binding!(
									closure!(clone value, clone context_creator, clone eval_state, |this, super_obj| {
										let context = context_creator.0(this, super_obj);
										// TODO: Assert
										evaluate(
											context,
											eval_state.clone(),
											&value,
										).unwrap_if_lazy()
									})
								),
							},
						);
					}
					Member::Field(FieldMember {
						name,
						params: Some(params),
						value,
						..
					}) => {
						let name = evaluate_field_name(context.clone(), eval_state.clone(), &name);
						new_members.insert(
							name,
							ObjMember {
								add: false,
								visibility: Visibility::Hidden,
								invoke: binding!(
									closure!(clone value, clone context_creator, clone eval_state, |this, super_obj| {
										// TODO: Assert
										evaluate_method(
											context_creator.0(this, super_obj),
											eval_state.clone(),
											&value.clone(),
											params.clone(),
										)
									})
								),
							},
						);
					}
					Member::BindStmt(_) => {}
					Member::AssertStmt(_) => {}
				}
			}
			future_this.fill(ObjValue::new(None, Rc::new(new_members)))
		}
		_ => todo!(),
	}
}

pub fn evaluate(context: Context, eval_state: EvaluationState, expr: &LocExpr) -> Val {
	use Expr::*;
	eval_state.clone().push(expr.clone(), "expr".to_owned(), || {
		let LocExpr(expr, loc) = expr;
		match &**expr {
			Literal(LiteralType::This) => Val::Obj(
				context
					.this()
					.clone()
					.unwrap_or_else(|| panic!("this not found")),
			),
			Literal(LiteralType::Super) => Val::Obj(
				context
					.super_obj()
					.clone()
					.unwrap_or_else(|| panic!("super not found")),
			),
			Literal(LiteralType::True) => Val::Bool(true),
			Literal(LiteralType::False) => Val::Bool(false),
			Literal(LiteralType::Null) => Val::Null,
			Parened(e) => evaluate(context, eval_state.clone(), e),
			Str(v) => Val::Str(v.clone()),
			Num(v) => Val::Num(*v),
			BinaryOp(v1, o, v2) => {
				let a = evaluate(context.clone(), eval_state.clone(), v1).unwrap_if_lazy();
				let op = *o;
				let b = evaluate(context.clone(), eval_state.clone(), v2).unwrap_if_lazy();
				evaluate_binary_op(
					context,
					eval_state,
					&a,
					op,
					&b,
				)
			},
			UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(context, eval_state, v)),
			Var(name) => Val::Lazy(context.binding(&name)).unwrap_if_lazy(),
			Index(value, index) => {
				match (
					evaluate(context.clone(), eval_state.clone(), value).unwrap_if_lazy(),
					evaluate(context.clone(), eval_state.clone(), index),
				) {
					(Val::Obj(v), Val::Str(s)) => v
						.get(&s)
						.unwrap_or_else(closure!(clone context, clone eval_state, || {
							if let Some(n) = v.get("__intristic_namespace__") {
								if let Val::Str(n) = n.unwrap_if_lazy() {
									Val::Intristic(n, s)
								} else {
									panic!("__intristic_namespace__ should be string");
								}
							} else {
								panic!("{} not found in {:?}", s, v)
							}
						}))
						.unwrap_if_lazy(),
					(Val::Arr(v), Val::Num(n)) => v
						.get(n as usize)
						.unwrap_or_else(|| panic!("out of bounds"))
						.clone(),
					(Val::Str(s), Val::Num(n)) => {
						Val::Str(s.chars().skip(n as usize).take(1).collect())
					}
					(v, i) => todo!("not implemented: {:?}[{:?}]", v, i.unwrap_if_lazy()),
				}
			}
			LocalExpr(bindings, returned) => {
				let mut new_bindings: HashMap<String, LazyBinding> = HashMap::new();
				let future_context = Context::new_future();

				let context_creator = context_creator!(
					closure!(clone future_context, |_, _| future_context.clone().unwrap())
				);

				for (k, v) in bindings
					.iter()
					.map(|b| evaluate_binding(eval_state.clone(), b, context_creator.clone()))
				{
					new_bindings.insert(k, v);
				}

				let context = context
					.extend(new_bindings, None, None, None)
					.into_future(future_context);
				evaluate(context, eval_state.clone(), &returned.clone())
			}
			Arr(items) => {
				let mut out = Vec::with_capacity(items.len());
				for item in items {
					out.push(evaluate(context.clone(), eval_state.clone(), item));
				}
				Val::Arr(out)
			}
			ArrComp(expr, compspecs) => {
				Val::Arr(evaluate_comp(context, eval_state, expr, compspecs).unwrap())
			}
			Obj(body) => Val::Obj(evaluate_object(context, eval_state, body.clone())),
			Apply(value, ArgsDesc(args)) => {
				let value = evaluate(context.clone(), eval_state.clone(), value).unwrap_if_lazy();
				match value {
					// TODO: Capture context of application
					Val::Intristic(ns, name) => match (&ns as &str, &name as &str) {
						// arr/string/function
						("std", "length") => {
							assert_eq!(args.len(), 1);
							let expr = &args.get(0).unwrap().1;
							match evaluate(context, eval_state.clone(), expr) {
								Val::Str(n) => Val::Num(n.chars().count() as f64),
								Val::Arr(i) => Val::Num(i.len() as f64),
								v => panic!("can't get length of {:?}", v),
							}
						}
						// any
						("std", "type") => {
							assert_eq!(args.len(), 1);
							let expr = &args.get(0).unwrap().1;
							Val::Str(evaluate(context, eval_state, expr).type_of().to_owned())
						}
						// length, idx=>any
						("std", "makeArray") => {
							assert_eq!(args.len(), 2);
							if let (Val::Num(v), Val::Func(d)) = (
								evaluate(context.clone(), eval_state.clone(), &args[0].1),
								evaluate(context, eval_state, &args[1].1),
							) {
								assert!(v > 0.0);
								let mut out = Vec::with_capacity(v as usize);
								for i in 0..v as usize {
									out.push(d.evaluate(vec![(None, Val::Num(i as f64))]))
								}
								Val::Arr(out)
							} else {
								panic!("bad makeArray call");
							}
						}
						// string
						("std", "codepoint") => {
							assert_eq!(args.len(), 1);
							if let Val::Str(s) = evaluate(context, eval_state, &args[0].1) {
								assert!(
									s.chars().count() == 1,
									"std.codepoint should receive single char string"
								);
								Val::Num(s.chars().take(1).next().unwrap() as u32 as f64)
							} else {
								panic!("bad codepoint call");
							}
						}
						// object, includeHidden
						("std", "objectFieldsEx") => {
							assert_eq!(args.len(), 2);
							if let (Val::Obj(body), Val::Bool(_include_hidden)) = (
								evaluate(context.clone(), eval_state.clone(), &args[0].1),
								evaluate(context, eval_state, &args[1].1),
							) {
								// TODO: handle visibility (_include_hidden)
								Val::Arr(body.fields().into_iter().map(Val::Str).collect())
							} else {
								panic!("bad objectFieldsEx call");
							}
						}
						(ns, name) => panic!("Intristic not found: {}.{}", ns, name),
					},
					Val::Func(f) => f.evaluate(
						args.clone()
							.into_iter()
							.map(move |a| {
								(
									a.clone().0,
									Val::Lazy(lazy_val!(
										closure!(clone context, clone a, clone eval_state, || evaluate(context.clone(), eval_state.clone(), &a.clone().1))
									)),
								)
							})
							.collect(),
					),
					_ => panic!("{:?} is not a function", value),
				}
			}
			Function(params, body) => evaluate_method(context, eval_state, body, params.clone()),
			AssertExpr(AssertStmt(value, msg), returned) => {
				if evaluate(context.clone(), eval_state.clone(), &value).try_cast_bool() {
					evaluate(context, eval_state, returned)
				}else {
					if let Some(msg) = msg {
						panic!("assertion failed ({:?}): {}", value, evaluate(context, eval_state, msg).try_cast_str());
					} else {
						panic!("assertion failed ({:?}): no message", value);
					}
				}
			},
			Error(e) => panic!("error: {}", evaluate(context, eval_state, e)),
			IfElse {
				cond,
				cond_then,
				cond_else,
			} => match evaluate(context.clone(), eval_state.clone(), &cond.0).unwrap_if_lazy() {
				Val::Bool(true) => evaluate(context, eval_state.clone(), cond_then),
				Val::Bool(false) => match cond_else {
					Some(v) => evaluate(context, eval_state, v),
					None => Val::Bool(false),
				},
				v => panic!("if condition evaluated to {:?} (boolean needed instead)", v),
			},
			_ => panic!(
				"evaluation not implemented: {:?}",
				LocExpr(expr.clone(), loc.clone())
			),
		}
	})
}
