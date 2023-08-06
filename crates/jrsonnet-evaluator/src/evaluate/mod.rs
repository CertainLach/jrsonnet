use std::rc::Rc;

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BindSpec, CompSpec, Expr, FieldMember, FieldName, ForSpecData,
	IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc,
};
use jrsonnet_types::ValType;

use self::destructure::destruct;
use crate::{
	arr::ArrValue,
	destructure::evaluate_dest,
	error::{suggest_object_fields, ErrorKind::*},
	evaluate::operator::{evaluate_add_op, evaluate_binary_op_special, evaluate_unary_op},
	function::{CallLocation, FuncDesc, FuncVal},
	throw,
	typed::Typed,
	val::{CachedUnbound, IndexableVal, StrValue, Thunk, ThunkValue},
	Context, GcHashMap, ObjValue, ObjValueBuilder, ObjectAssertion, Pending, Result, State,
	Unbound, Val,
};
pub mod destructure;
pub mod operator;

pub fn evaluate_trivial(expr: &LocExpr) -> Option<Val> {
	fn is_trivial(expr: &LocExpr) -> bool {
		match &*expr.0 {
			Expr::Str(_)
			| Expr::Num(_)
			| Expr::Literal(LiteralType::False | LiteralType::True | LiteralType::Null) => true,
			Expr::Arr(a) => a.iter().all(is_trivial),
			Expr::Parened(e) => is_trivial(e),
			_ => false,
		}
	}
	Some(match &*expr.0 {
		Expr::Str(s) => Val::Str(StrValue::Flat(s.clone())),
		Expr::Num(n) => Val::Num(*n),
		Expr::Literal(LiteralType::False) => Val::Bool(false),
		Expr::Literal(LiteralType::True) => Val::Bool(true),
		Expr::Literal(LiteralType::Null) => Val::Null,
		Expr::Arr(n) => {
			if n.iter().any(|e| !is_trivial(e)) {
				return None;
			}
			Val::Arr(ArrValue::eager(
				n.iter()
					.map(evaluate_trivial)
					.map(|e| e.expect("checked trivial"))
					.collect(),
			))
		}
		Expr::Parened(e) => evaluate_trivial(e)?,
		_ => return None,
	})
}

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
				for item in list.iter_lazy() {
					let fctx = Pending::new();
					let mut new_bindings = GcHashMap::with_capacity(var.capacity_hint());
					destruct(var, item, fctx.clone(), &mut new_bindings)?;
					let ctx = ctx
						.clone()
						.extend(new_bindings, None, None, None)
						.into_future(fctx);

					evaluate_comp(ctx, &specs[1..], callback)?;
				}
			}
			#[cfg(feature = "exp-object-iteration")]
			Val::Obj(obj) => {
				for field in obj.fields(
					// TODO: Should there be ability to preserve iteration order?
					#[cfg(feature = "exp-preserve-order")]
					false,
				) {
					#[derive(Trace)]
					struct ObjectFieldThunk {
						obj: ObjValue,
						field: IStr,
					}
					impl ThunkValue for ObjectFieldThunk {
						type Output = Val;

						fn get(self: Box<Self>) -> Result<Self::Output> {
							self.obj.get(self.field).transpose().expect(
								"field exists, as field name was obtained from object.fields()",
							)
						}
					}

					let fctx = Pending::new();
					let mut new_bindings = GcHashMap::with_capacity(var.capacity_hint());
					let value = Thunk::evaluated(Val::Arr(ArrValue::lazy(Cc::new(vec![
						Thunk::evaluated(Val::Str(StrValue::Flat(field.clone()))),
						Thunk::new(ObjectFieldThunk {
							field: field.clone(),
							obj: obj.clone(),
						}),
					]))));
					destruct(var, value, fctx.clone(), &mut new_bindings)?;
					let ctx = ctx
						.clone()
						.extend(new_bindings, None, None, None)
						.into_future(fctx);

					evaluate_comp(ctx, &specs[1..], callback)?;
				}
			}
			_ => throw!(InComprehensionCanOnlyIterateOverArray),
		},
	}
	Ok(())
}

trait CloneableUnbound<T>: Unbound<Bound = T> + Clone {}
impl<V, T> CloneableUnbound<T> for V where V: Unbound<Bound = T> + Clone {}

fn evaluate_object_locals(
	fctx: Pending<Context>,
	locals: Rc<Vec<BindSpec>>,
) -> impl CloneableUnbound<Context> {
	#[derive(Trace, Clone)]
	struct UnboundLocals {
		fctx: Pending<Context>,
		locals: Rc<Vec<BindSpec>>,
	}
	impl Unbound for UnboundLocals {
		type Bound = Context;

		fn bind(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Context> {
			let fctx = Context::new_future();
			let mut new_bindings =
				GcHashMap::with_capacity(self.locals.iter().map(BindSpec::capacity_hint).sum());
			for b in self.locals.iter() {
				evaluate_dest(b, fctx.clone(), &mut new_bindings)?;
			}

			let ctx = self.fctx.unwrap();
			let new_dollar = ctx.dollar().cloned().or_else(|| this.clone());

			let ctx = ctx
				.extend(new_bindings, new_dollar, sup, this)
				.into_future(fctx);

			Ok(ctx)
		}
	}

	UnboundLocals { fctx, locals }
}

pub fn evaluate_field_member<B: Unbound<Bound = Context> + Clone>(
	builder: &mut ObjValueBuilder,
	ctx: Context,
	uctx: B,
	field: &FieldMember,
) -> Result<()> {
	let name = evaluate_field_name(ctx, &field.name)?;
	let Some(name) = name else {
		return Ok(());
	};

	match field {
		FieldMember {
			plus,
			params: None,
			visibility,
			value,
			..
		} => {
			#[derive(Trace)]
			struct UnboundValue<B: Trace> {
				uctx: B,
				value: LocExpr,
				name: IStr,
			}
			impl<B: Unbound<Bound = Context>> Unbound for UnboundValue<B> {
				type Bound = Val;
				fn bind(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Val> {
					evaluate_named(self.uctx.bind(sup, this)?, &self.value, self.name.clone())
				}
			}

			builder
				.member(name.clone())
				.with_add(*plus)
				.with_visibility(*visibility)
				.with_location(value.1.clone())
				.bindable(UnboundValue {
					uctx,
					value: value.clone(),
					name,
				})?;
		}
		FieldMember {
			params: Some(params),
			visibility,
			value,
			..
		} => {
			#[derive(Trace)]
			struct UnboundMethod<B: Trace> {
				uctx: B,
				value: LocExpr,
				params: ParamsDesc,
				name: IStr,
			}
			impl<B: Unbound<Bound = Context>> Unbound for UnboundMethod<B> {
				type Bound = Val;
				fn bind(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Val> {
					Ok(evaluate_method(
						self.uctx.bind(sup, this)?,
						self.name.clone(),
						self.params.clone(),
						self.value.clone(),
					))
				}
			}

			builder
				.member(name.clone())
				.with_visibility(*visibility)
				.with_location(value.1.clone())
				.bindable(UnboundMethod {
					uctx,
					value: value.clone(),
					params: params.clone(),
					name,
				})?;
		}
	}
	Ok(())
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

	for member in members {
		match member {
			Member::Field(field) => {
				evaluate_field_member(&mut builder, ctx.clone(), uctx.clone(), field)?;
			}
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
				builder.assert(ObjectAssert {
					uctx: uctx.clone(),
					assert: stmt.clone(),
				});
			}
			Member::BindStmt(_) => {
				// Already handled
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
				let fctx = Context::new_future();
				ctxs.push((ctx.clone(), fctx.clone()));
				let uctx = evaluate_object_locals(fctx, locals.clone());

				evaluate_field_member(&mut builder, ctx, uctx, &obj.field)
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

	if let Some(trivial) = evaluate_trivial(expr) {
		return Ok(trivial);
	}
	let LocExpr(expr, loc) = expr;
	Ok(match &**expr {
		Literal(LiteralType::This) => {
			Val::Obj(ctx.this().ok_or(CantUseSelfOutsideOfObject)?.clone())
		}
		Literal(LiteralType::Super) => Val::Obj(
			ctx.super_obj().ok_or(NoSuperFound)?.with_this(
				ctx.this()
					.expect("if super exists - then this should too")
					.clone(),
			),
		),
		Literal(LiteralType::Dollar) => {
			Val::Obj(ctx.dollar().ok_or(NoTopLevelObjectFound)?.clone())
		}
		Literal(LiteralType::True) => Val::Bool(true),
		Literal(LiteralType::False) => Val::Bool(false),
		Literal(LiteralType::Null) => Val::Null,
		Parened(e) => evaluate(ctx, e)?,
		Str(v) => Val::Str(StrValue::Flat(v.clone())),
		Num(v) => Val::new_checked_num(*v)?,
		BinaryOp(v1, o, v2) => evaluate_binary_op_special(ctx, v1, *o, v2)?,
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(ctx, v)?)?,
		Var(name) => State::push(
			CallLocation::new(loc),
			|| format!("variable <{name}> access"),
			|| ctx.binding(name.clone())?.evaluate(),
		)?,
		Index {
			indexable: LocExpr(v, _),
			index,
			#[cfg(feature = "exp-null-coaelse")]
			null_coaelse,
		} if matches!(&**v, Expr::Literal(LiteralType::Super)) => {
			let name = evaluate(ctx.clone(), index)?;
			let Val::Str(name) = name else {
				throw!(ValueIndexMustBeTypeGot(
					ValType::Obj,
					ValType::Str,
					name.value_type(),
				))
			};
			let Some(super_obj) = ctx.super_obj() else {
				#[cfg(feature = "exp-null-coaelse")]
				if *null_coaelse {
					return Ok(Val::Null);
				}
				throw!(NoSuperFound)
			};
			let this = ctx
				.this()
				.expect("no this found, while super present, should not happen");
			let key = name.into_flat();
			match super_obj.get_for(key.clone(), this.clone())? {
				Some(v) => v,
				#[cfg(feature = "exp-null-coaelse")]
				None if *null_coaelse => Val::Null,
				None => {
					let suggestions = suggest_object_fields(super_obj, key.clone());

					throw!(NoSuchField(key, suggestions))
				}
			}
		}
		Index {
			indexable,
			index,
			#[cfg(feature = "exp-null-coaelse")]
			null_coaelse,
		} => match (evaluate(ctx.clone(), indexable)?, evaluate(ctx, index)?) {
			(Val::Obj(v), Val::Str(key)) => State::push(
				CallLocation::new(loc),
				|| format!("field <{key}> access"),
				|| match v.get(key.clone().into_flat()) {
					Ok(Some(v)) => Ok(v),
					#[cfg(feature = "exp-null-coaelse")]
					Ok(None) if *null_coaelse => Ok(Val::Null),
					Ok(None) => {
						let suggestions = suggest_object_fields(&v, key.clone().into_flat());

						throw!(NoSuchField(key.clone().into_flat(), suggestions))
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
			(Val::Arr(_), Val::Str(n)) => throw!(AttemptedIndexAnArrayWithString(n.into_flat())),
			(Val::Arr(_), n) => throw!(ValueIndexMustBeTypeGot(
				ValType::Arr,
				ValType::Num,
				n.value_type(),
			)),

			(Val::Str(s), Val::Num(n)) => Val::Str({
				let v: IStr = s
					.clone()
					.into_flat()
					.chars()
					.skip(n as usize)
					.take(1)
					.collect::<String>()
					.into();
				if v.is_empty() {
					let size = s.into_flat().chars().count();
					throw!(StringBoundsError(n as usize, size))
				}
				StrValue::Flat(v)
			}),
			(Val::Str(_), n) => throw!(ValueIndexMustBeTypeGot(
				ValType::Str,
				ValType::Num,
				n.value_type(),
			)),
			#[cfg(feature = "exp-null-coaelse")]
			(Val::Null, _) if *null_coaelse => Val::Null,

			(v, _) => throw!(CantIndexInto(v.value_type())),
		},
		LocalExpr(bindings, returned) => {
			let mut new_bindings: GcHashMap<IStr, Thunk<Val>> =
				GcHashMap::with_capacity(bindings.iter().map(BindSpec::capacity_hint).sum());
			let fctx = Context::new_future();
			for b in bindings {
				evaluate_dest(b, fctx.clone(), &mut new_bindings)?;
			}
			let ctx = ctx.extend(new_bindings, None, None, None).into_future(fctx);
			evaluate(ctx, &returned.clone())?
		}
		Arr(items) => {
			if items.is_empty() {
				Val::Arr(ArrValue::empty())
			} else if items.len() == 1 {
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
				Val::Arr(ArrValue::lazy(vec![Thunk::new(ArrayElement {
					ctx,
					item: items[0].clone(),
				})]))
			} else {
				Val::Arr(ArrValue::expr(ctx, items.iter().cloned()))
			}
		}
		ArrComp(expr, comp_specs) => {
			let mut out = Vec::new();
			evaluate_comp(ctx, comp_specs, &mut |ctx| {
				out.push(evaluate(ctx, expr)?);
				Ok(())
			})?;
			Val::Arr(ArrValue::eager(out))
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
				expr: Option<&LocExpr>,
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

			let start = parse_idx(loc, &ctx, desc.start.as_ref(), "start")?;
			let end = parse_idx(loc, &ctx, desc.end.as_ref(), "end")?;
			let step = parse_idx(loc, &ctx, desc.step.as_ref(), "step")?;

			IndexableVal::into_untyped(indexable.into_indexable()?.slice(start, end, step)?)?
		}
		i @ (Import(path) | ImportStr(path) | ImportBin(path)) => {
			let Expr::Str(path) = &*path.0 else {
				throw!("computed imports are not supported")
			};
			let tmp = loc.clone().0;
			let s = ctx.state();
			let resolved_path = s.resolve_from(tmp.source_path(), path as &str)?;
			match i {
				Import(_) => State::push(
					CallLocation::new(loc),
					|| format!("import {:?}", path.clone()),
					|| s.import_resolved(resolved_path),
				)?,
				ImportStr(_) => Val::Str(StrValue::Flat(s.import_resolved_str(resolved_path)?)),
				ImportBin(_) => Val::Arr(ArrValue::bytes(s.import_resolved_bin(resolved_path)?)),
				_ => unreachable!(),
			}
		}
	})
}
