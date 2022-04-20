use std::convert::TryFrom;

use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BindSpec, CompSpec, Expr, FieldMember, ForSpecData, IfSpecData,
	LiteralType, LocExpr, Member, ObjBody, ParamsDesc,
};
use jrsonnet_types::ValType;

use crate::{
	builtin::{std_slice, BUILTINS},
	error::Error::*,
	evaluate::operator::{evaluate_add_op, evaluate_binary_op_special, evaluate_unary_op},
	function::CallLocation,
	gc::TraceBox,
	push_frame, throw,
	typed::BoundedUsize,
	val::{ArrValue, FuncDesc, FuncVal, LazyValValue},
	with_state, Bindable, Context, ContextCreator, FutureWrapper, GcHashMap, LazyBinding, LazyVal,
	ObjValue, ObjValueBuilder, ObjectAssertion, Result, Val,
};
pub mod operator;

pub fn evaluate_binding_in_future(
	b: &BindSpec,
	context_creator: FutureWrapper<Context>,
) -> LazyVal {
	let b = b.clone();
	if let Some(params) = &b.params {
		let params = params.clone();

		#[derive(Trace)]
		struct LazyMethodBinding {
			context_creator: FutureWrapper<Context>,
			name: IStr,
			params: ParamsDesc,
			value: LocExpr,
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

		LazyVal::new(TraceBox(Box::new(LazyMethodBinding {
			context_creator,
			name: b.name.clone(),
			params,
			value: b.value.clone(),
		})))
	} else {
		#[derive(Trace)]
		struct LazyNamedBinding {
			context_creator: FutureWrapper<Context>,
			name: IStr,
			value: LocExpr,
		}
		impl LazyValValue for LazyNamedBinding {
			fn get(self: Box<Self>) -> Result<Val> {
				evaluate_named(self.context_creator.unwrap(), &self.value, self.name)
			}
		}
		LazyVal::new(TraceBox(Box::new(LazyNamedBinding {
			context_creator,
			name: b.name.clone(),
			value: b.value,
		})))
	}
}

pub fn evaluate_binding(b: &BindSpec, context_creator: ContextCreator) -> (IStr, LazyBinding) {
	let b = b.clone();
	if let Some(params) = &b.params {
		let params = params.clone();

		#[derive(Trace)]
		struct BindableMethodLazyVal {
			this: Option<ObjValue>,
			super_obj: Option<ObjValue>,

			context_creator: ContextCreator,
			name: IStr,
			params: ParamsDesc,
			value: LocExpr,
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

		#[derive(Trace)]
		struct BindableMethod {
			context_creator: ContextCreator,
			name: IStr,
			params: ParamsDesc,
			value: LocExpr,
		}
		impl Bindable for BindableMethod {
			fn bind(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<LazyVal> {
				Ok(LazyVal::new(TraceBox(Box::new(BindableMethodLazyVal {
					this,
					super_obj,

					context_creator: self.context_creator.clone(),
					name: self.name.clone(),
					params: self.params.clone(),
					value: self.value.clone(),
				}))))
			}
		}

		(
			b.name.clone(),
			LazyBinding::Bindable(Cc::new(TraceBox(Box::new(BindableMethod {
				context_creator,
				name: b.name.clone(),
				params,
				value: b.value.clone(),
			})))),
		)
	} else {
		#[derive(Trace)]
		struct BindableNamedLazyVal {
			this: Option<ObjValue>,
			super_obj: Option<ObjValue>,

			context_creator: ContextCreator,
			name: IStr,
			value: LocExpr,
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

		#[derive(Trace)]
		struct BindableNamed {
			context_creator: ContextCreator,
			name: IStr,
			value: LocExpr,
		}
		impl Bindable for BindableNamed {
			fn bind(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<LazyVal> {
				Ok(LazyVal::new(TraceBox(Box::new(BindableNamedLazyVal {
					this,
					super_obj,

					context_creator: self.context_creator.clone(),
					name: self.name.clone(),
					value: self.value.clone(),
				}))))
			}
		}

		(
			b.name.clone(),
			LazyBinding::Bindable(Cc::new(TraceBox(Box::new(BindableNamed {
				context_creator,
				name: b.name.clone(),
				value: b.value.clone(),
			})))),
		)
	}
}

pub fn evaluate_method(ctx: Context, name: IStr, params: ParamsDesc, body: LocExpr) -> Val {
	Val::Func(FuncVal::Normal(Cc::new(FuncDesc {
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
		jrsonnet_parser::FieldName::Dyn(expr) => push_frame(
			CallLocation::new(&expr.1),
			|| "evaluating field name".to_string(),
			|| {
				let value = evaluate(context, expr)?;
				if matches!(value, Val::Null) {
					Ok(None)
				} else {
					Ok(Some(IStr::try_from(value)?))
				}
			},
		)?,
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
			if bool::try_from(evaluate(context.clone(), cond)?)? {
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
		let mut bindings: GcHashMap<IStr, LazyBinding> = GcHashMap::with_capacity(members.len());
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

	let mut builder = ObjValueBuilder::new();
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

				#[derive(Trace)]
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
				builder
					.member(name.clone())
					.with_add(*plus)
					.with_visibility(*visibility)
					.with_location(value.1.clone())
					.bindable(TraceBox(Box::new(ObjMemberBinding {
						context_creator: context_creator.clone(),
						value: value.clone(),
						name,
					})))?;
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
				#[derive(Trace)]
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
				builder
					.member(name.clone())
					.hide()
					.with_location(value.1.clone())
					.bindable(TraceBox(Box::new(ObjMemberBinding {
						context_creator: context_creator.clone(),
						value: value.clone(),
						params: params.clone(),
						name,
					})))?;
			}
			Member::BindStmt(_) => {}
			Member::AssertStmt(stmt) => {
				#[derive(Trace)]
				struct ObjectAssert {
					context_creator: ContextCreator,
					assert: AssertStmt,
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
				builder.assert(TraceBox(Box::new(ObjectAssert {
					context_creator: context_creator.clone(),
					assert: stmt.clone(),
				})));
			}
		}
	}
	let this = builder.build();
	future_this.fill(this.clone());
	Ok(this)
}

pub fn evaluate_object(context: Context, object: &ObjBody) -> Result<ObjValue> {
	Ok(match object {
		ObjBody::MemberList(members) => evaluate_member_list_object(context, members)?,
		ObjBody::ObjComp(obj) => {
			let future_this = FutureWrapper::new();
			let mut builder = ObjValueBuilder::new();
			evaluate_comp(context.clone(), &obj.compspecs, &mut |ctx| {
				let new_bindings = FutureWrapper::new();
				let context_creator = ContextCreator(context.clone(), new_bindings.clone());
				let mut bindings: GcHashMap<IStr, LazyBinding> =
					GcHashMap::with_capacity(obj.pre_locals.len() + obj.post_locals.len());
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
						#[derive(Trace)]
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
									self.context
										.clone()
										.extend(GcHashMap::new(), None, this, None),
									&self.value,
								)?))
							}
						}
						builder
							.member(n)
							.with_location(obj.value.1.clone())
							.with_add(obj.plus)
							.bindable(TraceBox(Box::new(ObjCompBinding {
								context: ctx,
								value: obj.value.clone(),
							})))?;
					}
					v => throw!(FieldMustBeStringGot(v.value_type())),
				}

				Ok(())
			})?;

			let this = builder.build();
			future_this.fill(this.clone());
			this
		}
	})
}

pub fn evaluate_apply(
	context: Context,
	value: &LocExpr,
	args: &ArgsDesc,
	loc: CallLocation,
	tailstrict: bool,
) -> Result<Val> {
	let value = evaluate(context.clone(), value)?;
	Ok(match value {
		Val::Func(f) => {
			let body = || f.evaluate(context, loc, args, tailstrict);
			if tailstrict {
				body()?
			} else {
				push_frame(loc, || format!("function <{}> call", f.name()), body)?
			}
		}
		v => throw!(OnlyFunctionsCanBeCalledGot(v.value_type())),
	})
}

pub fn evaluate_assert(context: Context, assertion: &AssertStmt) -> Result<()> {
	let value = &assertion.0;
	let msg = &assertion.1;
	let assertion_result = push_frame(
		CallLocation::new(&value.1),
		|| "assertion condition".to_owned(),
		|| bool::try_from(evaluate(context.clone(), value)?),
	)?;
	if !assertion_result {
		push_frame(
			CallLocation::new(&value.1),
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
	// let bp = with_state(|s| s.0.stop_at.borrow().clone());
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
		Var(name) => push_frame(
			CallLocation::new(loc),
			|| format!("variable <{}> access", name),
			|| context.binding(name.clone())?.evaluate(),
		)?,
		Index(value, index) => {
			match (evaluate(context.clone(), value)?, evaluate(context, index)?) {
				(Val::Obj(v), Val::Str(s)) => {
					let sn = s.clone();
					push_frame(
						CallLocation::new(loc),
						|| format!("field <{}> access", sn),
						|| {
							if let Some(v) = v.get(s.clone())? {
								Ok(v)
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
			let mut new_bindings: GcHashMap<IStr, LazyVal> =
				GcHashMap::with_capacity(bindings.len());
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
				#[derive(Trace)]
				struct ArrayElement {
					context: Context,
					item: LocExpr,
				}
				impl LazyValValue for ArrayElement {
					fn get(self: Box<Self>) -> Result<Val> {
						evaluate(self.context, &self.item)
					}
				}
				out.push(LazyVal::new(TraceBox(Box::new(ArrayElement {
					context: context.clone(),
					item: item.clone(),
				}))));
			}
			Val::Arr(out.into())
		}
		ArrComp(expr, comp_specs) => {
			let mut out = Vec::new();
			evaluate_comp(context, comp_specs, &mut |ctx| {
				out.push(evaluate(ctx, expr)?);
				Ok(())
			})?;
			Val::Arr(ArrValue::Eager(Cc::new(out)))
		}
		Obj(body) => Val::Obj(evaluate_object(context, body)?),
		ObjExtend(s, t) => evaluate_add_op(
			&evaluate(context.clone(), s)?,
			&Val::Obj(evaluate_object(context, t)?),
		)?,
		Apply(value, args, tailstrict) => {
			evaluate_apply(context, value, args, CallLocation::new(loc), *tailstrict)?
		}
		Function(params, body) => {
			evaluate_method(context, "anonymous".into(), params.clone(), body.clone())
		}
		Intrinsic(name) => Val::Func(FuncVal::StaticBuiltin(
			BUILTINS
				.with(|b| b.get(name).copied())
				.ok_or_else(|| IntrinsicNotFound(name.clone()))?,
		)),
		AssertExpr(assert, returned) => {
			evaluate_assert(context.clone(), assert)?;
			evaluate(context, returned)?
		}
		ErrorStmt(e) => push_frame(
			CallLocation::new(loc),
			|| "error statement".to_owned(),
			|| throw!(RuntimeError(evaluate(context, e)?.to_string()?,)),
		)?,
		IfElse {
			cond,
			cond_then,
			cond_else,
		} => {
			if push_frame(
				CallLocation::new(loc),
				|| "if condition".to_owned(),
				|| bool::try_from(evaluate(context.clone(), &cond.0)?),
			)? {
				evaluate(context, cond_then)?
			} else {
				match cond_else {
					Some(v) => evaluate(context, v)?,
					None => Val::Null,
				}
			}
		}
		Slice(value, desc) => {
			let indexable = evaluate(context.clone(), value)?;
			let loc = CallLocation::new(loc);

			fn parse_idx<const MIN: usize>(
				loc: CallLocation,
				context: &Context,
				expr: &Option<LocExpr>,
				desc: &'static str,
			) -> Result<Option<BoundedUsize<MIN, { i32::MAX as usize }>>> {
				if let Some(value) = expr {
					Ok(Some(push_frame(
						loc,
						|| format!("slice {}", desc),
						|| evaluate(context.clone(), value)?.try_into(),
					)?))
				} else {
					Ok(None)
				}
			}

			let start = parse_idx(loc, &context, &desc.start, "start")?;
			let end = parse_idx(loc, &context, &desc.end, "end")?;
			let step = parse_idx(loc, &context, &desc.step, "step")?;

			std_slice(indexable.into_indexable()?, start, end, step)?
		}
		Import(path) => {
			let tmp = loc.clone().0;
			let mut import_location = tmp.to_path_buf();
			import_location.pop();
			push_frame(
				CallLocation::new(loc),
				|| format!("import {:?}", path),
				|| with_state(|s| s.import_file(&import_location, path)),
			)?
		}
		ImportStr(path) => {
			let tmp = loc.clone().0;
			let mut import_location = tmp.to_path_buf();
			import_location.pop();
			Val::Str(with_state(|s| s.import_file_str(&import_location, path))?)
		}
		ImportBin(path) => {
			let tmp = loc.clone().0;
			let mut import_location = tmp.to_path_buf();
			import_location.pop();
			let bytes = with_state(|s| s.import_file_bin(&import_location, path))?;
			Val::Arr(ArrValue::Bytes(bytes))
		}
	})
}
