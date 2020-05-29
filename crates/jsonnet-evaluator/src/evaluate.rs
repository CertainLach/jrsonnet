use crate::BoxedLazyVal;
use crate::{
	bool_val, ArgsBinding, BoxedBinding, BoxedContextCreator, ConstantContextCreator, Context,
	FuncDesc, FunctionDefault, FunctionRhs, NoArgsBinding, Val,
};
use crate::{
	future_wrapper, BoxedFunctionDefault, BoxedFunctionRhs, ContextCreator, ObjMember, ObjValue,
	PlainLazyVal,
};
use jsonnet_parser::{
	ArgsDesc, BinaryOpType, BindSpec, Expr, FieldMember, LiteralType, Member, ObjBody, ParamsDesc,
	Visibility,
};
use std::{
	cell::RefCell,
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

pub fn evaluate_binding<'t>(
	b: &BindSpec,
	context_creator: BoxedContextCreator,
) -> (String, BoxedBinding) {
	if let Some(args) = &b.params {
		(
			b.name.clone(),
			Rc::new(ArgsBinding {
				expr: *b.value.clone(),
				args: args.clone(),
				context_creator: context_creator.clone(),
			}),
		)
	} else {
		(
			b.name.clone(),
			Rc::new(NoArgsBinding {
				expr: *b.value.clone(),
				context_creator: context_creator.clone(),
			}) as BoxedBinding,
		)
	}
}

#[derive(Debug)]
struct MethodRhs {
	rhs: Expr,
}
impl FunctionRhs for MethodRhs {
	fn evaluate(&self, ctx: Context) -> Val {
		evaluate(ctx, &self.rhs)
	}
}

#[derive(Debug)]
struct MethodDefault {}
impl FunctionDefault for MethodDefault {
	fn default(&self, ctx: Context, expr: Expr) -> Val {
		evaluate(ctx, &expr)
	}
}

pub fn evaluate_method(ctx: Context, expr: &Expr, arg_spec: ParamsDesc) -> Val {
	Val::Func(FuncDesc {
		ctx,
		params: arg_spec,
		eval_rhs: BoxedFunctionRhs(Rc::new(MethodRhs { rhs: expr.clone() })),
		eval_default: BoxedFunctionDefault(Rc::new(MethodDefault {})),
	})
}

pub fn evaluate_field_name(context: Context, field_name: &jsonnet_parser::FieldName) -> String {
	match field_name {
		jsonnet_parser::FieldName::Fixed(n) => n.clone(),
		jsonnet_parser::FieldName::Dyn(expr) => {
			let name = evaluate(context, expr).unwrap_if_lazy();
			match name {
				Val::Str(n) => n.clone(),
				_ => panic!(
					"dynamic field name can be only evaluated to 'string', got: {:?}",
					name
				),
			}
		}
	}
}

pub fn evaluate_binary_op(a: &Val, op: BinaryOpType, b: &Val) -> Val {
	match (a, op, b) {
		(Val::Lazy(l), o, r) => evaluate_binary_op(&l.evaluate(), o, r),
		(l, o, Val::Lazy(r)) => evaluate_binary_op(l, o, &r.evaluate()),

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

		(Val::Num(v1), BinaryOpType::Eq, Val::Num(v2)) => bool_val(v1 == v2),
		(Val::Num(v1), BinaryOpType::Ne, Val::Num(v2)) => bool_val(v1 != v2),

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

future_wrapper!(HashMap<String, BoxedBinding>, FutureNewBindings);

#[derive(Debug)]
pub struct ObjectContextCreator {
	original: Context,
	future_bindings: FutureNewBindings,
}

impl ContextCreator for ObjectContextCreator {
	fn create_context(&self, this: &Option<ObjValue>, super_obj: &Option<ObjValue>) -> Context {
		self.original.extend(
			self.future_bindings.clone().unwrap(),
			self.original.dollar().clone().or_else(|| this.clone()),
			this.clone(),
			super_obj.clone(),
		)
	}
}

// TODO: Asserts
pub fn evaluate_object(context: Context, object: ObjBody) -> ObjValue {
	match object {
		ObjBody::MemberList(members) => {
			let future_bindings = FutureNewBindings::new();
			let binding_context_creator = Rc::new(ObjectContextCreator {
				future_bindings: future_bindings.clone(),
				original: context.clone(),
			});
			let mut bindings: HashMap<String, BoxedBinding> = HashMap::new();
			for (n, b) in members
				.iter()
				.filter_map(|m| match m {
					Member::BindStmt(b) => Some(b.clone()),
					_ => None,
				})
				.map(|b| evaluate_binding(&b, binding_context_creator.clone()))
			{
				bindings.insert(n, b);
			}
			let bindings = future_bindings.fill(bindings);
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
						let name = evaluate_field_name(context.clone(), name);
						new_members.insert(
							name,
							ObjMember {
								add: *plus,
								visibility: visibility.clone(),
								invoke: Rc::new(NoArgsBinding {
									context_creator: binding_context_creator.clone(),
									expr: value.clone(),
								}),
							},
						);
					}
					Member::Field(FieldMember {
						name,
						params: Some(params),
						value,
						..
					}) => {
						let name = evaluate_field_name(context.clone(), name);
						new_members.insert(
							name,
							ObjMember {
								add: false,
								visibility: Visibility::Hidden,
								invoke: Rc::new(ArgsBinding {
									expr: value.clone(),
									args: params.clone(),
									context_creator: binding_context_creator.clone(),
								}),
							},
						);
					}
					Member::BindStmt(_) => {}
					Member::AssertStmt(_) => {}
					_ => todo!(),
				}
			}
			ObjValue::new(None, Rc::new(new_members))
		}
		_ => todo!(),
	}
}

pub fn evaluate(context: Context, expr: &Expr) -> Val {
	use Expr::*;
	match &*expr {
		Literal(t) => Val::Literal(t.clone()),
		Parened(e) => evaluate(context, e),
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::Num(*v),
		BinaryOp(v1, o, v2) => {
			evaluate_binary_op(&evaluate(context.clone(), v1), *o, &evaluate(context, v2))
		}
		Var(name) => {
			let variable = context.binding(&name);
			let val = variable.evaluate(None, None);
			val
		}
		Index(box value, box index) => {
			match (
				evaluate(context.clone(), value).unwrap_if_lazy(),
				evaluate(context.clone(), index),
			) {
				(Val::Literal(LiteralType::Super), _idx) => todo!(),
				(Val::Literal(LiteralType::This), idx) => match &idx.unwrap_if_lazy() {
					Val::Str(str) => context
						.this()
						.clone()
						.unwrap_or_else(|| panic!("'this' is not defined in current context"))
						.get_raw(str, None)
						.unwrap_or_else(|| {
							panic!(
								"key {} not found in current context 'this' ({:?})",
								str,
								context.this()
							)
						}),
					_ => panic!("bad index"),
				},
				(Val::Obj(v), Val::Str(s)) => v
					.get_raw(&s, None)
					.unwrap_or_else(|| panic!("{} not found in {:?}", s, v)),
				(v, i) => todo!("not implemented: {:?}[{:?}]", v, i.unwrap_if_lazy()),
			}
		}
		LocalExpr(bindings, returned) => {
			let mut new_bindings: HashMap<String, BoxedBinding> = HashMap::new();
			let future_context = Context::new_future();

			let context_creator = Rc::new(ConstantContextCreator {
				context: future_context.clone(),
			});
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
								a.0,
								Val::Lazy(BoxedLazyVal(Rc::new(PlainLazyVal {
									context: context.clone(),
									expr: *a.1,
								}))),
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
			Val::Literal(LiteralType::True) => evaluate(context.clone(), cond_then),
			Val::Literal(LiteralType::False) => match cond_else {
				Some(v) => evaluate(context.clone(), v),
				None => Val::Literal(LiteralType::False),
			},
			v => panic!("if condition evaluated to {:?} (boolean needed instead)", v),
		},
		_ => panic!("evaluation not implemented: {:?}", expr),
	}
}
