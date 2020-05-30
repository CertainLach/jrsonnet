use crate::{
	binding, bool_val, context_creator, function_default, function_rhs, future_wrapper, lazy_val,
	Binding, Context, ContextCreator, FuncDesc, ObjMember, ObjValue, Val,
};
use closure::closure;
use jsonnet_parser::{
	ArgsDesc, BinaryOpType, BindSpec, Expr, FieldMember, LiteralType, Member, ObjBody, ParamsDesc,
	UnaryOpType, Visibility,
};
use std::{
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

pub fn evaluate_binding(b: &BindSpec, context_creator: ContextCreator) -> (String, Binding) {
	let b = b.clone();
	if let Some(args) = &b.params {
		let args = args.clone();
		(
			b.name.clone(),
			binding!(move |this, super_obj| Val::Lazy(lazy_val!(
				closure!(clone b, clone args, clone context_creator, || evaluate_method(
					context_creator.0(this.clone(), super_obj.clone()),
					&b.value,
					args.clone()
				))
			))),
		)
	} else {
		(
			b.name.clone(),
				binding!(move |this, super_obj| {
					println!("Evaluating binding");
					Val::Lazy(lazy_val!(
					closure!(clone context_creator, clone b, || evaluate(
						context_creator.0(this.clone(), super_obj.clone()),
						&b.value
					))
				))
			}),
		)
	}
}

pub fn evaluate_method(ctx: Context, expr: &Expr, arg_spec: ParamsDesc) -> Val {
	Val::Func(FuncDesc {
		ctx,
		params: arg_spec,
		eval_rhs: function_rhs!(closure!(clone expr, |ctx| evaluate(ctx, &expr))),
		eval_default: function_default!(|ctx, default| evaluate(ctx, &default)),
	})
}

pub fn evaluate_field_name(context: Context, field_name: &jsonnet_parser::FieldName) -> String {
	match field_name {
		jsonnet_parser::FieldName::Fixed(n) => n.clone(),
		jsonnet_parser::FieldName::Dyn(expr) => {
			let name = evaluate(context, expr).unwrap_if_lazy();
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
		(o, Val::Lazy(l)) => evaluate_unary_op(o, &l.0()),
		(UnaryOpType::Not, Val::Literal(LiteralType::True)) => Val::Literal(LiteralType::False),
		(UnaryOpType::Not, Val::Literal(LiteralType::False)) => Val::Literal(LiteralType::True),
		(op, o) => panic!("unary op not implemented: {:?} {:?}", op, o),
	}
}

pub fn evaluate_binary_op(a: &Val, op: BinaryOpType, b: &Val) -> Val {
	match (a, op, b) {
		(Val::Lazy(a), o, b) => evaluate_binary_op(&a.0(), o, b),
		(a, o, Val::Lazy(b)) => evaluate_binary_op(a, o, &b.0()),

		(Val::Str(v1), BinaryOpType::Add, Val::Str(v2)) => Val::Str(v1.to_owned() + &v2),
		(Val::Str(v1), BinaryOpType::Eq, Val::Str(v2)) => bool_val(v1 == v2),
		(Val::Str(v1), BinaryOpType::Ne, Val::Str(v2)) => bool_val(v1 != v2),

		(Val::Str(v1), BinaryOpType::Add, Val::Num(v2)) => Val::Str(format!("{}{}", v1, v2)),
		(Val::Str(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Str(v1.repeat(*v2 as usize)),

		(Val::Obj(v1), BinaryOpType::Add, Val::Obj(v2)) => Val::Obj(v2.with_super(v1.clone())),

		(Val::Arr(a), BinaryOpType::Add, Val::Arr(b)) => Val::Arr([&a[..], &b[..]].concat()),

		(Val::Num(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Num(v1 * v2),
		(Val::Num(v1), BinaryOpType::Div, Val::Num(v2)) => Val::Num(v1 / v2),
		(Val::Num(v1), BinaryOpType::Mod, Val::Num(v2)) => Val::Num(v1 % v2),

		(Val::Num(v1), BinaryOpType::Add, Val::Num(v2)) => Val::Num(v1 + v2),
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
		_ => panic!("no rules for binary operation: {:?} {:?} {:?}", a, op, b),
	}
}

future_wrapper!(HashMap<String, Binding>, FutureNewBindings);
future_wrapper!(ObjValue, FutureObjValue);

// TODO: Asserts
pub fn evaluate_object(context: Context, object: ObjBody) -> ObjValue {
	match object {
		ObjBody::MemberList(members) => {
			let future_bindings = FutureNewBindings::new();
			let future_this = FutureObjValue::new();
			let context_creator = context_creator!(
				closure!(clone context, clone future_bindings, |this: Option<ObjValue>, super_obj: Option<ObjValue>| {
					println!("Context created");
					context.clone().extend(
						future_bindings.clone().unwrap(),
						context.clone().dollar().clone().or_else(||this.clone()),
						this,
						super_obj
					)
				})
			);
			let mut bindings: HashMap<String, Binding> = HashMap::new();
			for (n, b) in members
				.iter()
				.filter_map(|m| match m {
					Member::BindStmt(b) => Some(b.clone()),
					_ => None,
				})
				.map(|b| {
					evaluate_binding(&b, context_creator.clone())
				})
			{
				bindings.insert(n, b);
			}
			future_bindings.fill(bindings);

			println!("Bindings filled");
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
						let name = evaluate_field_name(context.clone(), &name);
						new_members.insert(
							name,
							ObjMember {
								add: plus,
								visibility: visibility.clone(),
								invoke: binding!(
									closure!(clone value, clone context_creator, clone future_this, |this, super_obj| {
										// FIXME: I should take "this" instead of "future_this" there?
										// TODO: Assert
										evaluate(
											context_creator.0(this, super_obj),
											&value,
										)
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
						let name = evaluate_field_name(context.clone(), &name);
						new_members.insert(
							name,
							ObjMember {
								add: false,
								visibility: Visibility::Hidden,
								invoke: binding!(
									closure!(clone value, clone context_creator, clone future_this, |this, super_obj| {
										// FIXME: I should take "this" instead of "future_this" there?
										// TODO: Assert
										evaluate_method(
											context_creator.0(this, super_obj),
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

pub fn evaluate(context: Context, expr: &Expr) -> Val {
	use Expr::*;
	match &*expr {
		Literal(LiteralType::This) => {
			println!("{:?}", context.this());
			Val::Obj(
				context
					.this()
					.clone()
					.unwrap_or_else(|| panic!("this not found")),
			)
		}
		Literal(LiteralType::Super) => Val::Obj(
			context
				.super_obj()
				.clone()
				.unwrap_or_else(|| panic!("super not found")),
		),
		Literal(t) => Val::Literal(t.clone()),
		Parened(e) => evaluate(context, e),
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::Num(*v),
		BinaryOp(v1, o, v2) => {
			evaluate_binary_op(&evaluate(context.clone(), v1), *o, &evaluate(context, v2))
		}
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(context, v)),
		Var(name) => {
			let variable = context.binding(&name);
			variable.0(None, None).unwrap_if_lazy()
		}
		Index(box value, box index) => {
			match (
				evaluate(context.clone(), value).unwrap_if_lazy(),
				evaluate(context, index),
			) {
				(Val::Obj(v), Val::Str(s)) => v
					.get(&s)
					.unwrap_or_else(|| panic!("{} not found in {:?}", s, v)),
				(Val::Arr(v), Val::Num(n)) => v
					.get(n as usize)
					.unwrap_or_else(|| panic!("out of bounds"))
					.clone(),
				(v, i) => todo!("not implemented: {:?}[{:?}]", v, i.unwrap_if_lazy()),
			}
		}
		LocalExpr(bindings, returned) => {
			let mut new_bindings: HashMap<String, Binding> = HashMap::new();
			let future_context = Context::new_future();

			let context_creator = context_creator!(
				closure!(clone future_context, |_, _| future_context.clone().unwrap())
			);

			for (k, v) in bindings
				.iter()
				.map(move |b| evaluate_binding(b, context_creator.clone()))
			{
				new_bindings.insert(k, v);
			}

			let context = context
				.extend(new_bindings, None, None, None)
				.into_future(future_context);
			evaluate(context, &*returned.clone())
		}
		Obj(body) => Val::Obj(evaluate_object(context, body.clone())),
		Apply(box value, ArgsDesc(args)) => {
			let value = evaluate(context.clone(), value).unwrap_if_lazy();
			match value {
				Val::Func(f) => f.evaluate(
					args.clone()
						.into_iter()
						.map(|a| {
							(
								a.clone().0,
								Val::Lazy(lazy_val!(
									closure!(clone context, clone a, || evaluate(context.clone(), &a.clone().1))
								)),
							)
						})
						.collect(),
				),
				_ => panic!("{:?} is not a function", value),
			}
		}
		Function(params, body) => evaluate_method(context, body, params.clone()),
		Error(e) => panic!("error: {}", evaluate(context, e)),
		IfElse {
			cond,
			cond_then,
			cond_else,
		} => match evaluate(context.clone(), &cond.0).unwrap_if_lazy() {
			Val::Literal(LiteralType::True) => evaluate(context, cond_then),
			Val::Literal(LiteralType::False) => match cond_else {
				Some(v) => evaluate(context, v),
				None => Val::Literal(LiteralType::False),
			},
			v => panic!("if condition evaluated to {:?} (boolean needed instead)", v),
		},
		_ => panic!("evaluation not implemented: {:?}", expr),
	}
}
