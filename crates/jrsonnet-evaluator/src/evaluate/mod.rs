use std::{cmp::Ordering, rc::Rc};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BindSpec, CompSpec, Expr, FieldMember, FieldName, ForSpecData,
	IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc,
};
use jrsonnet_types::ValType;

use crate::{
	destructure::evaluate_dest,
	error::Error::*,
	evaluate::operator::{evaluate_add_op, evaluate_binary_op_special, evaluate_unary_op},
	function::{CallLocation, FuncDesc, FuncVal},
	tb, throw,
	typed::Typed,
	val::{ArrValue, CachedUnbound, IndexableVal, Thunk, ThunkValue},
	Context, GcHashMap, ObjValue, ObjValueBuilder, ObjectAssertion, Pending, Result, State,
	Unbound, Val,
};
pub mod destructure;
pub mod operator;

pub fn evaluate_method(ctx: Context, name: IStr, params: ParamsDesc, body: LocExpr) -> Val {
	Val::Func(FuncVal::Normal(Cc::new(FuncDesc {
		name,
		ctx,
		params,
		body,
	})))
}

pub fn evaluate_field_name(ctx: Context, field_name: &FieldName) -> Result<Option<IStr>> {
	Ok(match field_name {
		FieldName::Fixed(n) => Some(n.clone()),
		FieldName::Dyn(expr) => State::push(
			CallLocation::new(&expr.1),
			|| "evaluating field name".to_string(),
			|| {
				let value = evaluate(ctx, expr)?;
				if matches!(value, Val::Null) {
					Ok(None)
				} else {
					Ok(Some(IStr::from_untyped(value)?))
				}
			},
		)?,
	})
}

pub fn evaluate_comp(
	ctx: Context,
	specs: &[CompSpec],
	callback: &mut impl FnMut(Context) -> Result<()>,
) -> Result<()> {
	match specs.get(0) {
		None => callback(ctx)?,
		Some(CompSpec::IfSpec(IfSpecData(cond))) => {
			if bool::from_untyped(evaluate(ctx.clone(), cond)?)? {
				evaluate_comp(ctx, &specs[1..], callback)?;
			}
		}
		Some(CompSpec::ForSpec(ForSpecData(var, expr))) => match evaluate(ctx.clone(), expr)? {
			Val::Arr(list) => {
				for item in list.iter() {
					evaluate_comp(
						ctx.clone().with_var(var.clone(), item?.clone()),
						&specs[1..],
						callback,
					)?;
				}
			}
			_ => throw!(InComprehensionCanOnlyIterateOverArray),
		},
	}
	Ok(())
}

trait CloneableUnbound<T>: Unbound<Bound = T> + Clone {}

fn evaluate_object_locals(
	fctx: Pending<Context>,
	locals: Rc<Vec<BindSpec>>,
) -> impl CloneableUnbound<Context> {
	#[derive(Trace, Clone)]
	struct UnboundLocals {
		fctx: Pending<Context>,
		locals: Rc<Vec<BindSpec>>,
	}
	impl CloneableUnbound<Context> for UnboundLocals {}
	impl Unbound for UnboundLocals {
		type Bound = Context;

		fn bind(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Context> {
			let fctx = Context::new_future();
			let mut new_bindings = GcHashMap::new();
			for b in self.locals.iter() {
				evaluate_dest(b, fctx.clone(), &mut new_bindings)?;
			}

			let ctx = self.fctx.unwrap();
			let new_dollar = ctx.dollar().clone().or_else(|| this.clone());

			let ctx = ctx
				.extend(new_bindings, new_dollar, sup, this)
				.into_future(fctx);

			Ok(ctx)
		}
	}

	UnboundLocals { fctx, locals }
}

#[allow(clippy::too_many_lines)]
pub fn evaluate_member_list_object(ctx: Context, members: &[Member]) -> Result<ObjValue> {
	let mut builder = ObjValueBuilder::new();
	let locals = Rc::new(
		members
			.iter()
			.filter_map(|m| match m {
				Member::BindStmt(bind) => Some(bind.clone()),
				_ => None,
			})
			.collect::<Vec<_>>(),
	);

	let fctx = Context::new_future();

	// We have single context for all fields, so we can cache binds
	let uctx = CachedUnbound::new(evaluate_object_locals(fctx.clone(), locals));

	for member in members.iter() {
		match member {
			Member::Field(FieldMember {
				name,
				plus,
				params: None,
				visibility,
				value,
			}) => {
				#[derive(Trace)]
				struct UnboundValue<B: Trace> {
					uctx: B,
					value: LocExpr,
					name: IStr,
				}
				impl<B: Unbound<Bound = Context>> Unbound for UnboundValue<B> {
					type Bound = Thunk<Val>;
					fn bind(
						&self,
						sup: Option<ObjValue>,
						this: Option<ObjValue>,
					) -> Result<Thunk<Val>> {
						Ok(Thunk::evaluated(evaluate_named(
							self.uctx.bind(sup, this)?,
							&self.value,
							self.name.clone(),
						)?))
					}
				}

				let name = evaluate_field_name(ctx.clone(), name)?;
				let Some(name) = name else {
					continue;
				};

				builder
					.member(name.clone())
					.with_add(*plus)
					.with_visibility(*visibility)
					.with_location(value.1.clone())
					.bindable(tb!(UnboundValue {
						uctx: uctx.clone(),
						value: value.clone(),
						name: name.clone()
					}))?;
			}
			Member::Field(FieldMember {
				name,
				params: Some(params),
				value,
				..
			}) => {
				#[derive(Trace)]
				struct UnboundMethod<B: Trace> {
					uctx: B,
					value: LocExpr,
					params: ParamsDesc,
					name: IStr,
				}
				impl<B: Unbound<Bound = Context>> Unbound for UnboundMethod<B> {
					type Bound = Thunk<Val>;
					fn bind(
						&self,
						sup: Option<ObjValue>,
						this: Option<ObjValue>,
					) -> Result<Thunk<Val>> {
						Ok(Thunk::evaluated(evaluate_method(
							self.uctx.bind(sup, this)?,
							self.name.clone(),
							self.params.clone(),
							self.value.clone(),
						)))
					}
				}

				let name = if let Some(name) = evaluate_field_name(ctx.clone(), name)? {
					name
				} else {
					continue;
				};

				builder
					.member(name.clone())
					.hide()
					.with_location(value.1.clone())
					.bindable(tb!(UnboundMethod {
						uctx: uctx.clone(),
						value: value.clone(),
						params: params.clone(),
						name: name.clone()
					}))?;
			}
			Member::BindStmt(_) => {}
			Member::AssertStmt(stmt) => {
				#[derive(Trace)]
				struct ObjectAssert<B: Trace> {
					uctx: B,
					assert: AssertStmt,
				}
				impl<B: Unbound<Bound = Context>> ObjectAssertion for ObjectAssert<B> {
					fn run(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<()> {
						let ctx = self.uctx.bind(sup, this)?;
						evaluate_assert(ctx, &self.assert)
					}
				}
				builder.assert(tb!(ObjectAssert {
					uctx: uctx.clone(),
					assert: stmt.clone(),
				}));
			}
		}
	}
	let this = builder.build();
	fctx.fill(ctx.extend(GcHashMap::new(), None, None, Some(this.clone())));
	Ok(this)
}

pub fn evaluate_object(ctx: Context, object: &ObjBody) -> Result<ObjValue> {
	Ok(match object {
		ObjBody::MemberList(members) => evaluate_member_list_object(ctx, members)?,
		ObjBody::ObjComp(obj) => {
			let mut builder = ObjValueBuilder::new();
			let locals = Rc::new(
				obj.pre_locals
					.iter()
					.chain(obj.post_locals.iter())
					.cloned()
					.collect::<Vec<_>>(),
			);
			let mut ctxs = vec![];
			evaluate_comp(ctx, &obj.compspecs, &mut |ctx| {
				let key = evaluate(ctx.clone(), &obj.key)?;
				let fctx = Context::new_future();
				ctxs.push((ctx, fctx.clone()));
				let uctx = evaluate_object_locals(fctx, locals.clone());

				match key {
					Val::Null => {}
					Val::Str(n) => {
						#[derive(Trace)]
						struct UnboundValue<B: Trace> {
							uctx: B,
							value: LocExpr,
						}
						impl<B: Unbound<Bound = Context>> Unbound for UnboundValue<B> {
							type Bound = Thunk<Val>;
							fn bind(
								&self,
								sup: Option<ObjValue>,
								this: Option<ObjValue>,
							) -> Result<Thunk<Val>> {
								Ok(Thunk::evaluated(evaluate(
									self.uctx.bind(sup, this.clone())?.extend(
										GcHashMap::new(),
										None,
										None,
										this,
									),
									&self.value,
								)?))
							}
						}
						builder
							.member(n)
							.with_location(obj.value.1.clone())
							.with_add(obj.plus)
							.bindable(tb!(UnboundValue {
								uctx,
								value: obj.value.clone(),
							}))?;
					}
					v => throw!(FieldMustBeStringGot(v.value_type())),
				}

				Ok(())
			})?;

			let this = builder.build();
			for (ctx, fctx) in ctxs {
				let _ctx = ctx
					.extend(GcHashMap::new(), None, None, Some(this.clone()))
					.into_future(fctx);
			}
			this
		}
	})
}

pub fn evaluate_apply(
	ctx: Context,
	value: &LocExpr,
	args: &ArgsDesc,
	loc: CallLocation<'_>,
	tailstrict: bool,
) -> Result<Val> {
	let value = evaluate(ctx.clone(), value)?;
	Ok(match value {
		Val::Func(f) => {
			let body = || f.evaluate(ctx, loc, args, tailstrict);
			if tailstrict {
				body()?
			} else {
				State::push(loc, || format!("function <{}> call", f.name()), body)?
			}
		}
		v => throw!(OnlyFunctionsCanBeCalledGot(v.value_type())),
	})
}

pub fn evaluate_assert(ctx: Context, assertion: &AssertStmt) -> Result<()> {
	let value = &assertion.0;
	let msg = &assertion.1;
	let assertion_result = State::push(
		CallLocation::new(&value.1),
		|| "assertion condition".to_owned(),
		|| bool::from_untyped(evaluate(ctx.clone(), value)?),
	)?;
	if !assertion_result {
		State::push(
			CallLocation::new(&value.1),
			|| "assertion failure".to_owned(),
			|| {
				if let Some(msg) = msg {
					throw!(AssertionFailed(evaluate(ctx, msg)?.to_string()?));
				}
				throw!(AssertionFailed(Val::Null.to_string()?));
			},
		)?;
	}
	Ok(())
}

pub fn evaluate_named(ctx: Context, expr: &LocExpr, name: IStr) -> Result<Val> {
	use Expr::*;
	let LocExpr(raw_expr, _loc) = expr;
	Ok(match &**raw_expr {
		Function(params, body) => evaluate_method(ctx, name, params.clone(), body.clone()),
		_ => evaluate(ctx, expr)?,
	})
}

#[allow(clippy::too_many_lines)]
pub fn evaluate(ctx: Context, expr: &LocExpr) -> Result<Val> {
	use Expr::*;
	let LocExpr(expr, loc) = expr;
	// let bp = with_state(|s| s.0.stop_at.borrow().clone());
	Ok(match &**expr {
		Literal(LiteralType::This) => {
			Val::Obj(ctx.this().clone().ok_or(CantUseSelfOutsideOfObject)?)
		}
		Literal(LiteralType::Super) => Val::Obj(
			ctx.super_obj().clone().ok_or(NoSuperFound)?.with_this(
				ctx.this()
					.clone()
					.expect("if super exists - then this should to"),
			),
		),
		Literal(LiteralType::Dollar) => {
			Val::Obj(ctx.dollar().clone().ok_or(NoTopLevelObjectFound)?)
		}
		Literal(LiteralType::True) => Val::Bool(true),
		Literal(LiteralType::False) => Val::Bool(false),
		Literal(LiteralType::Null) => Val::Null,
		Parened(e) => evaluate(ctx, e)?,
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::new_checked_num(*v)?,
		BinaryOp(v1, o, v2) => evaluate_binary_op_special(ctx, v1, *o, v2)?,
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(ctx, v)?)?,
		Var(name) => State::push(
			CallLocation::new(loc),
			|| format!("variable <{name}> access"),
			|| ctx.binding(name.clone())?.evaluate(),
		)?,
		Index(value, index) => match (evaluate(ctx.clone(), value)?, evaluate(ctx, index)?) {
			(Val::Obj(v), Val::Str(key)) => State::push(
				CallLocation::new(loc),
				|| format!("field <{key}> access"),
				|| match v.get(key.clone()) {
					Ok(Some(v)) => Ok(v),
					#[cfg(not(feature = "friendly-errors"))]
					Ok(None) => throw!(NoSuchField(key.clone(), vec![])),
					#[cfg(feature = "friendly-errors")]
					Ok(None) => {
						let mut heap = Vec::new();
						for field in v.fields_ex(
							true,
							#[cfg(feature = "exp-preserve-order")]
							false,
						) {
							let conf = strsim::jaro_winkler(&field as &str, &key as &str);
							if conf < 0.8 {
								continue;
							}
							heap.push((conf, field));
						}
						heap.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

						throw!(NoSuchField(
							key.clone(),
							heap.into_iter().map(|(_, v)| v).collect()
						))
					}
					Err(e) => Err(e),
				},
			)?,
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

			(Val::Str(s), Val::Num(n)) => Val::Str({
				let v: IStr = s
					.chars()
					.skip(n as usize)
					.take(1)
					.collect::<String>()
					.into();
				if v.is_empty() {
					let size = s.chars().count();
					throw!(StringBoundsError(n as usize, size))
				}
				v
			}),
			(Val::Str(_), n) => throw!(ValueIndexMustBeTypeGot(
				ValType::Str,
				ValType::Num,
				n.value_type(),
			)),

			(v, _) => throw!(CantIndexInto(v.value_type())),
		},
		LocalExpr(bindings, returned) => {
			let mut new_bindings: GcHashMap<IStr, Thunk<Val>> =
				GcHashMap::with_capacity(bindings.len());
			let fctx = Context::new_future();
			for b in bindings {
				evaluate_dest(b, fctx.clone(), &mut new_bindings)?;
			}
			let ctx = ctx.extend(new_bindings, None, None, None).into_future(fctx);
			evaluate(ctx, &returned.clone())?
		}
		Arr(items) => {
			let mut out = Vec::with_capacity(items.len());
			for item in items {
				// TODO: Implement ArrValue::Lazy with same context for every element?
				#[derive(Trace)]
				struct ArrayElement {
					ctx: Context,
					item: LocExpr,
				}
				impl ThunkValue for ArrayElement {
					type Output = Val;
					fn get(self: Box<Self>) -> Result<Val> {
						evaluate(self.ctx, &self.item)
					}
				}
				out.push(Thunk::new(tb!(ArrayElement {
					ctx: ctx.clone(),
					item: item.clone(),
				})));
			}
			Val::Arr(out.into())
		}
		ArrComp(expr, comp_specs) => {
			let mut out = Vec::new();
			evaluate_comp(ctx, comp_specs, &mut |ctx| {
				out.push(evaluate(ctx, expr)?);
				Ok(())
			})?;
			Val::Arr(ArrValue::Eager(Cc::new(out)))
		}
		Obj(body) => Val::Obj(evaluate_object(ctx, body)?),
		ObjExtend(a, b) => evaluate_add_op(
			&evaluate(ctx.clone(), a)?,
			&Val::Obj(evaluate_object(ctx, b)?),
		)?,
		Apply(value, args, tailstrict) => {
			evaluate_apply(ctx, value, args, CallLocation::new(loc), *tailstrict)?
		}
		Function(params, body) => {
			evaluate_method(ctx, "anonymous".into(), params.clone(), body.clone())
		}
		AssertExpr(assert, returned) => {
			evaluate_assert(ctx.clone(), assert)?;
			evaluate(ctx, returned)?
		}
		ErrorStmt(e) => State::push(
			CallLocation::new(loc),
			|| "error statement".to_owned(),
			|| throw!(RuntimeError(evaluate(ctx, e)?.to_string()?,)),
		)?,
		IfElse {
			cond,
			cond_then,
			cond_else,
		} => {
			if State::push(
				CallLocation::new(loc),
				|| "if condition".to_owned(),
				|| bool::from_untyped(evaluate(ctx.clone(), &cond.0)?),
			)? {
				evaluate(ctx, cond_then)?
			} else {
				match cond_else {
					Some(v) => evaluate(ctx, v)?,
					None => Val::Null,
				}
			}
		}
		Slice(value, desc) => {
			fn parse_idx<T: Typed>(
				loc: CallLocation<'_>,
				ctx: &Context,
				expr: &Option<LocExpr>,
				desc: &'static str,
			) -> Result<Option<T>> {
				if let Some(value) = expr {
					Ok(Some(State::push(
						loc,
						|| format!("slice {desc}"),
						|| T::from_untyped(evaluate(ctx.clone(), value)?),
					)?))
				} else {
					Ok(None)
				}
			}

			let indexable = evaluate(ctx.clone(), value)?;
			let loc = CallLocation::new(loc);

			let start = parse_idx(loc, &ctx, &desc.start, "start")?;
			let end = parse_idx(loc, &ctx, &desc.end, "end")?;
			let step = parse_idx(loc, &ctx, &desc.step, "step")?;

			IndexableVal::into_untyped(indexable.into_indexable()?.slice(start, end, step)?)?
		}
		i @ (Import(path) | ImportStr(path) | ImportBin(path)) => {
			let tmp = loc.clone().0;
			let s = ctx.state();
			let resolved_path = s.resolve_from(tmp.source_path(), path as &str)?;
			match i {
				Import(_) => State::push(
					CallLocation::new(loc),
					|| format!("import {:?}", path.clone()),
					|| s.import_resolved(resolved_path),
				)?,
				ImportStr(_) => Val::Str(s.import_resolved_str(resolved_path)?),
				ImportBin(_) => Val::Arr(ArrValue::Bytes(s.import_resolved_bin(resolved_path)?)),
				_ => unreachable!(),
			}
		}
	})
}
