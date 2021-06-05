use crate::{
	equals, error::Error::*, push, throw, with_state, ArrValue, Bindable, Context, ContextCreator,
	FuncDesc, FuncVal, FutureWrapper, LazyBinding, LazyVal, LazyValValue, ObjMember, ObjValue,
	ObjectAssertion, Result, Val,
};
use gc::{custom_trace, Finalize, Gc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BinaryOpType, BindSpec, CompSpec, Expr, ExprLocation, FieldMember,
	ForSpecData, IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc, UnaryOpType,
	Visibility,
};
use jrsonnet_types::ValType;
use rustc_hash::{FxHashMap, FxHasher};
use std::{collections::HashMap, hash::BuildHasherDefault, rc::Rc};

pub fn evaluate_binding_in_future(
	b: &BindSpec,
	context_creator: FutureWrapper<Context>,
) -> LazyVal {
	let b = b.clone();
	if let Some(params) = &b.params {
		let params = params.clone();

		struct LazyMethodBinding {
			context_creator: FutureWrapper<Context>,
			name: IStr,
			params: ParamsDesc,
			value: LocExpr,
		}
		impl Finalize for LazyMethodBinding {}
		unsafe impl Trace for LazyMethodBinding {
			custom_trace!(this, {
				mark(&this.context_creator);
				mark(&this.name);
				mark(&this.params);
				mark(&this.value);
			});
		}
		impl LazyValValue for LazyMethodBinding {
			fn get(self: Box<Self>) -> Result<Val> {
				Ok(evaluate_method(
					self.context_creator.unwrap(),
					self.name,
					self.params,
					self.value,
				))
			}
		}

		LazyVal::new(Box::new(LazyMethodBinding {
			context_creator,
			name: b.name.clone(),
			params,
			value: b.value.clone(),
		}))
	} else {
		struct LazyNamedBinding {
			context_creator: FutureWrapper<Context>,
			name: IStr,
			value: LocExpr,
		}
		impl Finalize for LazyNamedBinding {}
		unsafe impl Trace for LazyNamedBinding {
			custom_trace!(this, {
				mark(&this.context_creator);
				mark(&this.name);
				mark(&this.value);
			});
		}
		impl LazyValValue for LazyNamedBinding {
			fn get(self: Box<Self>) -> Result<Val> {
				evaluate_named(self.context_creator.unwrap(), &self.value, self.name)
			}
		}
		LazyVal::new(Box::new(LazyNamedBinding {
			context_creator,
			name: b.name.clone(),
			value: b.value,
		}))
	}
}

pub fn evaluate_binding(b: &BindSpec, context_creator: ContextCreator) -> (IStr, LazyBinding) {
	let b = b.clone();
	if let Some(params) = &b.params {
		let params = params.clone();

		struct BindableMethodLazyVal {
			this: Option<ObjValue>,
			super_obj: Option<ObjValue>,

			context_creator: ContextCreator,
			name: IStr,
			params: ParamsDesc,
			value: LocExpr,
		}
		impl Finalize for BindableMethodLazyVal {}
		unsafe impl Trace for BindableMethodLazyVal {
			custom_trace!(this, {
				mark(&this.this);
				mark(&this.super_obj);
				mark(&this.context_creator);
				mark(&this.name);
				mark(&this.params);
				mark(&this.value);
			});
		}
		impl LazyValValue for BindableMethodLazyVal {
			fn get(self: Box<Self>) -> Result<Val> {
				Ok(evaluate_method(
					self.context_creator.create(self.this, self.super_obj)?,
					self.name,
					self.params,
					self.value,
				))
			}
		}

		#[derive(Trace, Finalize)]
		struct BindableMethod {
			context_creator: ContextCreator,
			name: IStr,
			params: ParamsDesc,
			value: LocExpr,
		}
		impl Bindable for BindableMethod {
			fn bind(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<LazyVal> {
				Ok(LazyVal::new(Box::new(BindableMethodLazyVal {
					this: this.clone(),
					super_obj: super_obj.clone(),

					context_creator: self.context_creator.clone(),
					name: self.name.clone(),
					params: self.params.clone(),
					value: self.value.clone(),
				})))
			}
		}

		(
			b.name.clone(),
			LazyBinding::Bindable(Gc::new(Box::new(BindableMethod {
				context_creator,
				name: b.name.clone(),
				params,
				value: b.value.clone(),
			}))),
		)
	} else {
		struct BindableNamedLazyVal {
			this: Option<ObjValue>,
			super_obj: Option<ObjValue>,

			context_creator: ContextCreator,
			name: IStr,
			value: LocExpr,
		}
		impl Finalize for BindableNamedLazyVal {}
		unsafe impl Trace for BindableNamedLazyVal {
			custom_trace!(this, {
				mark(&this.this);
				mark(&this.super_obj);
				mark(&this.context_creator);
				mark(&this.name);
				mark(&this.value);
			});
		}
		impl LazyValValue for BindableNamedLazyVal {
			fn get(self: Box<Self>) -> Result<Val> {
				evaluate_named(
					self.context_creator.create(self.this, self.super_obj)?,
					&self.value,
					self.name,
				)
			}
		}

		#[derive(Trace, Finalize)]
		struct BindableNamed {
			context_creator: ContextCreator,
			name: IStr,
			value: LocExpr,
		}
		impl Bindable for BindableNamed {
			fn bind(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<LazyVal> {
				Ok(LazyVal::new(Box::new(BindableNamedLazyVal {
					this,
					super_obj,

					context_creator: self.context_creator.clone(),
					name: self.name.clone(),
					value: self.value.clone(),
				})))
			}
		}

		(
			b.name.clone(),
			LazyBinding::Bindable(Gc::new(Box::new(BindableNamed {
				context_creator,
				name: b.name.clone(),
				value: b.value.clone(),
			}))),
		)
	}
}

pub fn evaluate_method(ctx: Context, name: IStr, params: ParamsDesc, body: LocExpr) -> Val {
	Val::Func(Gc::new(FuncVal::Normal(FuncDesc {
		name,
		ctx,
		params,
		body,
	})))
}

pub fn evaluate_field_name(
	context: Context,
	field_name: &jrsonnet_parser::FieldName,
) -> Result<Option<IStr>> {
	Ok(match field_name {
		jrsonnet_parser::FieldName::Fixed(n) => Some(n.clone()),
		jrsonnet_parser::FieldName::Dyn(expr) => {
			let value = evaluate(context, expr)?;
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
		(UnaryOpType::Not, Val::Bool(v)) => Val::Bool(!v),
		(UnaryOpType::Minus, Val::Num(n)) => Val::Num(-*n),
		(UnaryOpType::BitNot, Val::Num(n)) => Val::Num(!(*n as i32) as f64),
		(op, o) => throw!(UnaryOperatorDoesNotOperateOnType(op, o.value_type())),
	})
}

pub fn evaluate_add_op(a: &Val, b: &Val) -> Result<Val> {
	Ok(match (a, b) {
		(Val::DebugGcTraceValue(v1), Val::DebugGcTraceValue(v2)) => {
			evaluate_add_op(&v1.value, &v2.value)?
		}
		(Val::Str(v1), Val::Str(v2)) => Val::Str(((**v1).to_owned() + v2).into()),

		// Can't use generic json serialization way, because it depends on number to string concatenation (std.jsonnet:890)
		(Val::Num(n), Val::Str(o)) => Val::Str(format!("{}{}", n, o).into()),
		(Val::Str(o), Val::Num(n)) => Val::Str(format!("{}{}", o, n).into()),

		(Val::Str(s), o) => Val::Str(format!("{}{}", s, o.clone().to_string()?).into()),
		(o, Val::Str(s)) => Val::Str(format!("{}{}", o.clone().to_string()?, s).into()),

		(Val::Obj(v1), Val::Obj(v2)) => Val::Obj(v2.extend_from(v1.clone())),
		(Val::Arr(a), Val::Arr(b)) => {
			let mut out = Vec::with_capacity(a.len() + b.len());
			out.extend(a.iter_lazy());
			out.extend(b.iter_lazy());
			Val::Arr(out.into())
		}
		(Val::Num(v1), Val::Num(v2)) => Val::new_checked_num(v1 + v2)?,
		_ => throw!(BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Add,
			a.value_type(),
			b.value_type(),
		)),
	})
}

pub fn evaluate_binary_op_special(
	context: Context,
	a: &LocExpr,
	op: BinaryOpType,
	b: &LocExpr,
) -> Result<Val> {
	Ok(match (evaluate(context.clone(), a)?, op, b) {
		(Val::Bool(true), BinaryOpType::Or, _o) => Val::Bool(true),
		(Val::Bool(false), BinaryOpType::And, _o) => Val::Bool(false),
		(a, op, eb) => evaluate_binary_op_normal(&a, op, &evaluate(context, eb)?)?,
	})
}

pub fn evaluate_binary_op_normal(a: &Val, op: BinaryOpType, b: &Val) -> Result<Val> {
	Ok(match (a, op, b) {
		(a, BinaryOpType::Add, b) => evaluate_add_op(a, b)?,

		(a, BinaryOpType::Eq, b) => Val::Bool(equals(a, b)?),
		(a, BinaryOpType::Neq, b) => Val::Bool(!equals(a, b)?),

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
			a.value_type(),
			b.value_type(),
		)),
	})
}

pub fn evaluate_comp(
	context: Context,
	specs: &[CompSpec],
	callback: &mut impl FnMut(Context) -> Result<()>,
) -> Result<()> {
	match specs.get(0) {
		None => callback(context)?,
		Some(CompSpec::IfSpec(IfSpecData(cond))) => {
			if evaluate(context.clone(), cond)?.try_cast_bool("if spec")? {
				evaluate_comp(context, &specs[1..], callback)?
			}
		}
		Some(CompSpec::ForSpec(ForSpecData(var, expr))) => match evaluate(context.clone(), expr)? {
			Val::Arr(list) => {
				for item in list.iter() {
					evaluate_comp(
						context.clone().with_var(var.clone(), item?.clone()),
						&specs[1..],
						callback,
					)?
				}
			}
			_ => throw!(InComprehensionCanOnlyIterateOverArray),
		},
	}
	Ok(())
}

pub fn evaluate_member_list_object(context: Context, members: &[Member]) -> Result<ObjValue> {
	let new_bindings = FutureWrapper::new();
	let future_this = FutureWrapper::new();
	let context_creator = ContextCreator(context.clone(), new_bindings.clone());
	{
		let mut bindings: FxHashMap<IStr, LazyBinding> =
			FxHashMap::with_capacity_and_hasher(members.len(), BuildHasherDefault::default());
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

	let mut new_members = FxHashMap::default();
	let mut assertions: Vec<Box<dyn ObjectAssertion>> = Vec::new();
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

				#[derive(Trace, Finalize)]
				struct ObjMemberBinding {
					context_creator: ContextCreator,
					value: LocExpr,
					name: IStr,
				}
				impl Bindable for ObjMemberBinding {
					fn bind(
						&self,
						this: Option<ObjValue>,
						super_obj: Option<ObjValue>,
					) -> Result<LazyVal> {
						Ok(LazyVal::new_resolved(evaluate_named(
							self.context_creator.create(this, super_obj)?,
							&self.value,
							self.name.clone(),
						)?))
					}
				}
				new_members.insert(
					name.clone(),
					ObjMember {
						add: *plus,
						visibility: *visibility,
						invoke: LazyBinding::Bindable(Gc::new(Box::new(ObjMemberBinding {
							context_creator: context_creator.clone(),
							value: value.clone(),
							name,
						}))),
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
				#[derive(Trace, Finalize)]
				struct ObjMemberBinding {
					context_creator: ContextCreator,
					value: LocExpr,
					params: ParamsDesc,
					name: IStr,
				}
				impl Bindable for ObjMemberBinding {
					fn bind(
						&self,
						this: Option<ObjValue>,
						super_obj: Option<ObjValue>,
					) -> Result<LazyVal> {
						Ok(LazyVal::new_resolved(evaluate_method(
							self.context_creator.create(this, super_obj)?,
							self.name.clone(),
							self.params.clone(),
							self.value.clone(),
						)))
					}
				}
				new_members.insert(
					name.clone(),
					ObjMember {
						add: false,
						visibility: Visibility::Hidden,
						invoke: LazyBinding::Bindable(Gc::new(Box::new(ObjMemberBinding {
							context_creator: context_creator.clone(),
							value: value.clone(),
							params: params.clone(),
							name,
						}))),
						location: value.1.clone(),
					},
				);
			}
			Member::BindStmt(_) => {}
			Member::AssertStmt(stmt) => {
				struct ObjectAssert {
					context_creator: ContextCreator,
					assert: AssertStmt,
				}
				impl Finalize for ObjectAssert {}
				unsafe impl Trace for ObjectAssert {
					custom_trace!(this, {
						mark(&this.context_creator);
						mark(&this.assert);
					});
				}
				impl ObjectAssertion for ObjectAssert {
					fn run(
						&self,
						this: Option<ObjValue>,
						super_obj: Option<ObjValue>,
					) -> Result<()> {
						let ctx = self.context_creator.create(this, super_obj)?;
						evaluate_assert(ctx, &self.assert)
					}
				}
				assertions.push(Box::new(ObjectAssert {
					context_creator: context_creator.clone(),
					assert: stmt.clone(),
				}));
			}
		}
	}
	let this = ObjValue::new(None, Gc::new(new_members), Gc::new(assertions));
	future_this.fill(this.clone());
	Ok(this)
}

pub fn evaluate_object(context: Context, object: &ObjBody) -> Result<ObjValue> {
	Ok(match object {
		ObjBody::MemberList(members) => evaluate_member_list_object(context, members)?,
		ObjBody::ObjComp(obj) => {
			let future_this = FutureWrapper::new();
			let mut new_members = FxHashMap::default();
			evaluate_comp(context.clone(), &obj.compspecs, &mut |ctx| {
				let new_bindings = FutureWrapper::new();
				let context_creator = ContextCreator(context.clone(), new_bindings.clone());
				let mut bindings: FxHashMap<IStr, LazyBinding> =
					FxHashMap::with_capacity_and_hasher(
						obj.pre_locals.len() + obj.post_locals.len(),
						BuildHasherDefault::default(),
					);
				for (n, b) in obj
					.pre_locals
					.iter()
					.chain(obj.post_locals.iter())
					.map(|b| evaluate_binding(b, context_creator.clone()))
				{
					bindings.insert(n, b);
				}
				new_bindings.fill(bindings.clone());
				let ctx = ctx.extend_unbound(bindings, None, None, None)?;
				let key = evaluate(ctx.clone(), &obj.key)?;

				match key {
					Val::Null => {}
					Val::Str(n) => {
						#[derive(Trace, Finalize)]
						struct ObjCompBinding {
							context: Context,
							value: LocExpr,
						}
						impl Bindable for ObjCompBinding {
							fn bind(
								&self,
								this: Option<ObjValue>,
								_super_obj: Option<ObjValue>,
							) -> Result<LazyVal> {
								Ok(LazyVal::new_resolved(evaluate(
									self.context.clone().extend(
										FxHashMap::default(),
										None,
										this,
										None,
									),
									&self.value,
								)?))
							}
						}
						new_members.insert(
							n,
							ObjMember {
								add: false,
								visibility: Visibility::Normal,
								invoke: LazyBinding::Bindable(Gc::new(Box::new(ObjCompBinding {
									context: ctx.clone(),
									value: obj.value.clone(),
								}))),
								location: obj.value.1.clone(),
							},
						);
					}
					v => throw!(FieldMustBeStringGot(v.value_type())),
				}

				Ok(())
			})?;

			let this = ObjValue::new(None, Gc::new(new_members), Gc::new(Vec::new()));
			future_this.fill(this.clone());
			this
		}
	})
}

pub fn evaluate_apply(
	context: Context,
	value: &LocExpr,
	args: &ArgsDesc,
	loc: Option<&ExprLocation>,
	tailstrict: bool,
) -> Result<Val> {
	let value = evaluate(context.clone(), value)?;
	Ok(match value {
		Val::Func(f) => {
			let body = || f.evaluate(context, loc, args, tailstrict);
			if tailstrict {
				body()?
			} else {
				push(loc, || format!("function <{}> call", f.name()), body)?
			}
		}
		v => throw!(OnlyFunctionsCanBeCalledGot(v.value_type())),
	})
}

pub fn evaluate_assert(context: Context, assertion: &AssertStmt) -> Result<()> {
	let value = &assertion.0;
	let msg = &assertion.1;
	let assertion_result = push(
		value.1.as_ref(),
		|| "assertion condition".to_owned(),
		|| {
			evaluate(context.clone(), value)?
				.try_cast_bool("assertion condition should be of type `boolean`")
		},
	)?;
	if !assertion_result {
		push(
			value.1.as_ref(),
			|| "assertion failure".to_owned(),
			|| {
				if let Some(msg) = msg {
					throw!(AssertionFailed(evaluate(context, msg)?.to_string()?));
				} else {
					throw!(AssertionFailed(Val::Null.to_string()?));
				}
			},
		)?
	}
	Ok(())
}

pub fn evaluate_named(context: Context, lexpr: &LocExpr, name: IStr) -> Result<Val> {
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
		Literal(LiteralType::This) => {
			Val::Obj(context.this().clone().ok_or(CantUseSelfOutsideOfObject)?)
		}
		Literal(LiteralType::Super) => Val::Obj(
			context
				.super_obj()
				.clone()
				.ok_or(NoSuperFound)?
				.with_this(context.this().clone().unwrap()),
		),
		Literal(LiteralType::Dollar) => {
			Val::Obj(context.dollar().clone().ok_or(NoTopLevelObjectFound)?)
		}
		Literal(LiteralType::True) => Val::Bool(true),
		Literal(LiteralType::False) => Val::Bool(false),
		Literal(LiteralType::Null) => Val::Null,
		Parened(e) => evaluate(context, e)?,
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::new_checked_num(*v)?,
		BinaryOp(v1, o, v2) => evaluate_binary_op_special(context, v1, *o, v2)?,
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(context, v)?)?,
		Var(name) => push(
			loc.as_ref(),
			|| format!("variable <{}>", name),
			|| context.binding(name.clone())?.evaluate(),
		)?,
		Index(value, index) => {
			match (evaluate(context.clone(), value)?, evaluate(context, index)?) {
				(Val::Obj(v), Val::Str(s)) => {
					let sn = s.clone();
					push(
						loc.as_ref(),
						|| format!("field <{}> access", sn),
						|| {
							if let Some(v) = v.get(s.clone())? {
								Ok(v)
							} else if v.get("__intrinsic_namespace__".into())?.is_some() {
								Ok(Val::Func(Gc::new(FuncVal::Intrinsic(s))))
							} else {
								throw!(NoSuchField(s))
							}
						},
					)?
				}
				(Val::Obj(_), n) => throw!(ValueIndexMustBeTypeGot(
					ValType::Obj,
					ValType::Str,
					n.value_type(),
				)),

				(Val::Arr(v), Val::Num(n)) => {
					if n.fract() > f64::EPSILON {
						throw!(FractionalIndex)
					}
					v.get(n as usize)?
						.ok_or_else(|| ArrayBoundsError(n as usize, v.len()))?
				}
				(Val::Arr(_), Val::Str(n)) => throw!(AttemptedIndexAnArrayWithString(n)),
				(Val::Arr(_), n) => throw!(ValueIndexMustBeTypeGot(
					ValType::Arr,
					ValType::Num,
					n.value_type(),
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
					n.value_type(),
				)),

				(v, _) => throw!(CantIndexInto(v.value_type())),
			}
		}
		LocalExpr(bindings, returned) => {
			let mut new_bindings: FxHashMap<IStr, LazyVal> = HashMap::with_capacity_and_hasher(
				bindings.len(),
				BuildHasherDefault::<FxHasher>::default(),
			);
			let future_context = Context::new_future();
			for b in bindings {
				new_bindings.insert(
					b.name.clone(),
					evaluate_binding_in_future(b, future_context.clone()),
				);
			}
			let context = context
				.extend_bound(new_bindings)
				.into_future(future_context);
			evaluate(context, &returned.clone())?
		}
		Arr(items) => {
			let mut out = Vec::with_capacity(items.len());
			for item in items {
				// TODO: Implement ArrValue::Lazy with same context for every element?
				struct ArrayElement {
					context: Context,
					item: LocExpr,
				}
				impl Finalize for ArrayElement {}
				unsafe impl Trace for ArrayElement {
					custom_trace!(this, {
						mark(&this.context);
						mark(&this.item);
					});
				}
				impl LazyValValue for ArrayElement {
					fn get(self: Box<Self>) -> Result<Val> {
						evaluate(self.context, &self.item)
					}
				}
				out.push(LazyVal::new(Box::new(ArrayElement {
					context: context.clone(),
					item: item.clone(),
				})));
			}
			Val::Arr(out.into())
		}
		ArrComp(expr, comp_specs) => {
			let mut out = Vec::new();
			evaluate_comp(context, comp_specs, &mut |ctx| {
				out.push(evaluate(ctx, expr)?);
				Ok(())
			})?;
			Val::Arr(ArrValue::Eager(Gc::new(out)))
		}
		Obj(body) => Val::Obj(evaluate_object(context, body)?),
		ObjExtend(s, t) => evaluate_add_op(
			&evaluate(context.clone(), s)?,
			&Val::Obj(evaluate_object(context, t)?),
		)?,
		Apply(value, args, tailstrict) => {
			evaluate_apply(context, value, args, loc.as_ref(), *tailstrict)?
		}
		Function(params, body) => {
			evaluate_method(context, "anonymous".into(), params.clone(), body.clone())
		}
		Intrinsic(name) => Val::Func(Gc::new(FuncVal::Intrinsic(name.clone()))),
		AssertExpr(assert, returned) => {
			evaluate_assert(context.clone(), assert)?;
			evaluate(context, returned)?
		}
		ErrorStmt(e) => push(
			loc.as_ref(),
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
				loc.as_ref(),
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
				loc.as_ref(),
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
	})
}
