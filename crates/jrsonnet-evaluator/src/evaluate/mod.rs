use std::rc::Rc;

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BinaryOpType, BindSpec, CompSpec, Expr, FieldMember, FieldName,
	ForSpecData, IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc,
};
use jrsonnet_types::ValType;

use self::destructure::destruct;
use crate::{
	arr::ArrValue,
	bail,
	destructure::evaluate_dest,
	error::{suggest_object_fields, ErrorKind::*},
	evaluate::operator::{evaluate_add_op, evaluate_binary_op_special, evaluate_unary_op},
	function::{CallLocation, FuncDesc, FuncVal},
	in_frame,
	typed::Typed,
	val::{CachedUnbound, IndexableVal, NumValue, StrValue, Thunk},
	with_state, Context, Error, GcHashMap, ObjValue, ObjValueBuilder, ObjectAssertion, Pending,
	Result, ResultExt, SupThis, Unbound, Val,
};
pub mod destructure;
pub mod operator;

// This is the amount of bytes that need to be left on the stack before increasing the size.
// It must be at least as large as the stack required by any code that does not call
// `ensure_sufficient_stack`.
const RED_ZONE: usize = 100 * 1024; // 100k

// Only the first stack that is pushed, grows exponentially (2^n * STACK_PER_RECURSION) from then
// on. This flag has performance relevant characteristics. Don't set it too high.
const STACK_PER_RECURSION: usize = 1024 * 1024; // 1MB

/// Grows the stack on demand to prevent stack overflow. Call this in strategic locations
/// to "break up" recursive calls. E.g. almost any call to `visit_expr` or equivalent can benefit
/// from this.
///
/// Should not be sprinkled around carelessly, as it causes a little bit of overhead.
#[inline]
#[cfg(not(miri))]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
	// This is the amount of bytes that need to be left on the stack before increasing the size.
	// It must be at least as large as the stack required by any code that does not call
	// `ensure_sufficient_stack`.
	const RED_ZONE: usize = 100 * 1024; // 100k

	// Only the first stack that is pushed, grows exponentially (2^n * STACK_PER_RECURSION) from then
	// on. This flag has performance relevant characteristics. Don't set it too high.
	const STACK_PER_RECURSION: usize = 1024 * 1024; // 1MB

	stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}
#[inline]
#[cfg(miri)]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
	f()
}

pub fn evaluate_trivial(expr: &LocExpr) -> Option<Val> {
	fn is_trivial(expr: &LocExpr) -> bool {
		match expr.expr() {
			Expr::Str(_)
			| Expr::Num(_)
			| Expr::Literal(LiteralType::False | LiteralType::True | LiteralType::Null) => true,
			Expr::Arr(a) => a.iter().all(is_trivial),
			Expr::Parened(e) => is_trivial(e),
			_ => false,
		}
	}
	Some(match expr.expr() {
		Expr::Str(s) => Val::string(s.clone()),
		Expr::Num(n) => {
			Val::Num(NumValue::new(*n).expect("parser will not allow non-finite values"))
		}
		Expr::Literal(LiteralType::False) => Val::Bool(false),
		Expr::Literal(LiteralType::True) => Val::Bool(true),
		Expr::Literal(LiteralType::Null) => Val::Null,
		Expr::Arr(n) => {
			if n.iter().any(|e| !is_trivial(e)) {
				return None;
			}
			Val::array(
				n.iter()
					.map(evaluate_trivial)
					.map(|e| e.expect("checked trivial"))
					.collect::<Vec<_>>(),
			)
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

pub fn evaluate_field_name(ctx: &Context, field_name: &FieldName) -> Result<Option<IStr>> {
	Ok(match field_name {
		FieldName::Fixed(n) => Some(n.clone()),
		FieldName::Dyn(expr) => in_frame(
			CallLocation::new(&expr.span()),
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

pub fn evaluate_comp_strict(
	ctx: &mut Context,
	specs: &[CompSpec],
	callback: &mut impl FnMut(&Context) -> Result<()>,
) -> Result<()> {
	match specs.first() {
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
					let ctx = ctx.clone().extend_bindings(new_bindings).into_future(fctx);

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
					let fctx = Pending::new();
					let mut new_bindings = GcHashMap::with_capacity(var.capacity_hint());
					let obj = obj.clone();
					let value = Thunk::evaluated(Val::Arr(ArrValue::lazy(vec![
						Thunk::evaluated(Val::string(field.clone())),
						Thunk!(move || obj.get(field).transpose().expect(
							"field exists, as field name was obtained from object.fields()",
						)),
					])));
					destruct(var, value, fctx.clone(), &mut new_bindings)?;
					let ctx = ctx
						.clone()
						.extend(new_bindings, None, None, None)
						.into_future(fctx);

					evaluate_comp(ctx, &specs[1..], callback)?;
				}
			}
			_ => bail!(InComprehensionCanOnlyIterateOverArray),
		},
	}
	Ok(())
}

trait CloneableUnbound<T>: Unbound<Bound = T> + Clone {}
impl<V, T> CloneableUnbound<T> for V where V: Unbound<Bound = T> + Clone {}

fn evaluate_object_locals(
	fctx: Context,
	locals: Rc<Vec<BindSpec>>,
) -> impl CloneableUnbound<Context> {
	#[derive(Trace, Clone)]
	struct UnboundLocals {
		fctx: Context,
		locals: Rc<Vec<BindSpec>>,
	}
	impl Unbound for UnboundLocals {
		type Bound = Context;

		fn bind(&self, sup_this: SupThis) -> Result<Context> {
			let fctx = Context::new_future();
			let mut new_bindings =
				GcHashMap::with_capacity(self.locals.iter().map(BindSpec::capacity_hint).sum());
			for b in self.locals.iter() {
				evaluate_dest(b, fctx.clone(), &mut new_bindings)?;
			}

			let ctx = self.fctx.clone();

			let ctx = ctx
				.extend_bindings_sup_this(new_bindings, sup_this)
				.into_future(fctx);

			Ok(ctx)
		}
	}

	UnboundLocals { fctx, locals }
}

pub fn evaluate_field_member<B: Unbound<Bound = Context> + Clone>(
	builder: &mut ObjValueBuilder,
	ctx: &Context,
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
				fn bind(&self, sup_this: SupThis) -> Result<Val> {
					evaluate_named(self.uctx.bind(sup_this)?, &self.value, self.name.clone())
				}
			}

			builder
				.field(name.clone())
				.with_add(*plus)
				.with_visibility(*visibility)
				.with_location(value.span())
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
				fn bind(&self, sup_this: SupThis) -> Result<Val> {
					Ok(evaluate_method(
						self.uctx.bind(sup_this)?,
						self.name.clone(),
						self.params.clone(),
						self.value.clone(),
					))
				}
			}

			builder
				.field(name.clone())
				.with_visibility(*visibility)
				.with_location(value.span())
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
pub fn evaluate_member_list_object(ctx: &Context, members: &[Member]) -> Result<ObjValue> {
	let mut builder = ObjValueBuilder::with_capacity(count_members);
	let locals = Rc::new(
		members
			.iter()
			.filter_map(|m| match m {
				Member::BindStmt(bind) => Some(bind.clone()),
				_ => None,
			})
			.collect::<Vec<_>>(),
	);

	// We have single context for all fields, so we can cache binds
	let uctx = CachedUnbound::new(evaluate_object_locals(ctx.clone(), locals));

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
					fn run(&self, sup_this: SupThis) -> Result<()> {
						let ctx = self.uctx.bind(sup_this)?;
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
	Ok(builder.build())
}

pub fn evaluate_object(ctx: &Context, object: &ObjBody) -> Result<ObjValue> {
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
			evaluate_comp(ctx, &obj.compspecs, &mut |ctx| {
				let uctx = evaluate_object_locals(ctx.clone(), locals.clone());

				evaluate_field_member(&mut builder, ctx, uctx, &obj.field)
			})?;

			builder.build()
		}
	})
}

pub fn evaluate_apply(
	ctx: &Context,
	value: &LocExpr,
	args: &ArgsDesc,
	loc: CallLocation<'_>,
	tailstrict: bool,
) -> Result<Val> {
	let value = evaluate(ctx, value)?;
	Ok(match value {
		Val::Func(f) => {
			let body = || f.evaluate(ctx, loc, args, tailstrict);
			if tailstrict {
				body()?
			} else {
				in_frame(loc, || format!("function <{}> call", f.name()), body)?
			}
		}
		v => bail!(OnlyFunctionsCanBeCalledGot(v.value_type())),
	})
}

pub fn evaluate_assert(ctx: &Context, assertion: &AssertStmt) -> Result<()> {
	let value = &assertion.0;
	let msg = &assertion.1;
	let assertion_result = in_frame(
		CallLocation::new(&value.span()),
		|| "assertion condition".to_owned(),
		|| bool::from_untyped(evaluate(ctx, value)?),
	)?;
	if !assertion_result {
		in_frame(
			CallLocation::new(&value.span()),
			|| "assertion failure".to_owned(),
			|| {
				if let Some(msg) = msg {
					bail!(AssertionFailed(evaluate(ctx, msg)?.to_string()?));
				}
				bail!(AssertionFailed(Val::Null.to_string()?));
			},
		)?;
	}
	Ok(())
}

pub fn evaluate_named(ctx: &Context, expr: &LocExpr, name: IStr) -> Result<Val> {
	use Expr::*;
	Ok(match expr.expr() {
		Function(params, body) => evaluate_method(ctx.clone(), name, params.clone(), body.clone()),
		_ => evaluate(ctx, expr)?,
	})
}

#[allow(clippy::too_many_lines)]
pub fn evaluate(ctx: &Context, expr: &LocExpr) -> Result<Val> {
	use Expr::*;

	if let Some(trivial) = evaluate_trivial(expr) {
		return Ok(trivial);
	}
	let loc = expr.span();
	Ok(match expr.expr() {
		Literal(LiteralType::This) => Val::Obj(ctx.try_this()?),
		Literal(LiteralType::Super) => Val::Obj(ctx.try_sup_this()?.standalone_super()?),
		Literal(LiteralType::Dollar) => Val::Obj(ctx.try_dollar()?),
		Literal(LiteralType::True) => Val::Bool(true),
		Literal(LiteralType::False) => Val::Bool(false),
		Literal(LiteralType::Null) => Val::Null,
		Parened(e) => evaluate(ctx, e)?,
		Str(v) => Val::string(v.clone()),
		Num(v) => Val::try_num(*v)?,
		// I have tried to remove special behavior from super by implementing standalone-super
		// expresion, but looks like this case still needs special treatment.
		//
		// Note that other jsonnet implementations will fail on `if value in (super)` expression,
		// because the standalone super literal is not supported, that is because in other
		// implementations `in super` treated differently from `in smth_else`.
		BinaryOp(field, BinaryOpType::In, e)
			if matches!(e.expr(), Expr::Literal(LiteralType::Super)) =>
		{
			let sup_this = ctx.try_sup_this()?;
			// In jsonnet, "field" in e is eager, LHS expression is always executed regardless of super existence.
			// In jrsonnet, however, this wasn't true, this was kept here for compatibility.
			if !sup_this.has_super() {
				return Ok(Val::Bool(false));
			}
			let field = evaluate(ctx, field)?;
			Val::Bool(sup_this.field_in_super(field.to_string()?))
		}
		BinaryOp(v1, o, v2) => evaluate_binary_op_special(ctx, v1, *o, v2)?,
		UnaryOp(o, v) => evaluate_unary_op(*o, &evaluate(ctx, v)?)?,
		Var(name) => in_frame(
			CallLocation::new(&loc),
			|| format!("local <{name}> access"),
			|| ctx.binding(name.clone()),
		)?,
		Index { indexable, parts } => ensure_sufficient_stack(|| {
			let mut parts = parts.iter();
			let mut indexable = if matches!(indexable.expr(), Expr::Literal(LiteralType::Super)) {
				let part = parts.next().expect("at least part should exist");
				// sup_this existence check might also be skipped here for null-coalesce...
				// But I believe this might cause errors.
				let sup_this = ctx.try_sup_this()?;
				if !sup_this.has_super() {
					#[cfg(feature = "exp-null-coaelse")]
					if part.null_coaelse {
						return Ok(Val::Null);
					}
					bail!(NoSuperFound)
				}
				let name = evaluate(ctx.clone(), &part.value)?;

				let Val::Str(name) = name else {
					bail!(ValueIndexMustBeTypeGot(
						ValType::Obj,
						ValType::Str,
						name.value_type(),
					))
				};

				let name = name.into_flat();
				match sup_this
					.get_super(name.clone())
					.with_description_src(&part.value, || format!("field <{name}> access"))?
				{
					Some(v) => v,
					#[cfg(feature = "exp-null-coaelse")]
					None if part.null_coaelse => return Ok(Val::Null),
					None => {
						let suggestions = suggest_object_fields(
							&sup_this.standalone_super().expect("super exists"),
							name.clone(),
						);

						bail!(NoSuchField(name, suggestions))
					}
				}
			} else {
				evaluate(ctx.clone(), indexable)?
			};

			for part in parts {
				indexable = match (indexable, evaluate(ctx.clone(), &part.value)?) {
					(Val::Obj(v), Val::Str(key)) => match v
						.get(key.clone().into_flat())
						.with_description_src(&part.value, || format!("field <{key}> access"))?
					{
						Some(v) => v,
						#[cfg(feature = "exp-null-coaelse")]
						None if part.null_coaelse => return Ok(Val::Null),
						None => {
							let suggestions = suggest_object_fields(&v, key.clone().into_flat());

							return Err(Error::from(NoSuchField(
								key.clone().into_flat(),
								suggestions,
							)))
							.with_description_src(&part.value, || format!("field <{key}> access"));
						}
					},
					(Val::Obj(_), n) => bail!(ValueIndexMustBeTypeGot(
						ValType::Obj,
						ValType::Str,
						n.value_type(),
					)),
					(Val::Arr(v), Val::Num(n)) => {
						let n = n.get();
						if n.fract() > f64::EPSILON {
							bail!(FractionalIndex)
						}
						if n < 0.0 {
							bail!(ArrayBoundsError(n as isize, v.len()));
						}
						v.get(n as usize)?
							.ok_or_else(|| ArrayBoundsError(n as isize, v.len()))?
					}
					(Val::Arr(_), Val::Str(n)) => {
						bail!(AttemptedIndexAnArrayWithString(n.into_flat()))
					}
					(Val::Arr(_), n) => bail!(ValueIndexMustBeTypeGot(
						ValType::Arr,
						ValType::Num,
						n.value_type(),
					)),

					(Val::Str(s), Val::Num(n)) => Val::Str({
						let v: IStr = s
							.clone()
							.into_flat()
							.chars()
							.skip(n.get() as usize)
							.take(1)
							.collect::<String>()
							.into();
						if v.is_empty() {
							let size = s.into_flat().chars().count();
							bail!(StringBoundsError(n.get() as usize, size))
						}
						StrValue::Flat(v)
					}),
					(Val::Str(_), n) => bail!(ValueIndexMustBeTypeGot(
						ValType::Str,
						ValType::Num,
						n.value_type(),
					)),
					#[cfg(feature = "exp-null-coaelse")]
					(Val::Null, _) if part.null_coaelse => return Ok(Val::Null),
					(v, _) => bail!(CantIndexInto(v.value_type())),
				};
			}
			Ok(indexable)
		})?,
		LocalExpr(bindings, returned) => {
			let mut new_bindings: GcHashMap<IStr, Thunk<Val>> =
				GcHashMap::with_capacity(bindings.iter().map(BindSpec::capacity_hint).sum());
			let fctx = Context::new_future();
			for b in bindings {
				evaluate_dest(b, fctx.clone(), &mut new_bindings)?;
			}
			let ctx = ctx.extend_bindings(new_bindings).into_future(fctx);
			evaluate(ctx, &returned.clone())?
		}
		Arr(items) => {
			if items.is_empty() {
				Val::Arr(ArrValue::empty())
			} else if items.len() == 1 {
				let item = items[0].clone();
				Val::Arr(ArrValue::lazy(vec![Thunk!(move || evaluate(ctx, &item))]))
			} else {
				Val::Arr(ArrValue::expr(ctx, items.iter().cloned()))
			}
		}
		ArrComp(expr, comp_specs) => {
			let mut out = Vec::new();
			evaluate_comp(ctx, comp_specs, &mut |ctx| {
				let expr = expr.clone();
				out.push(Thunk!(move || evaluate(ctx, &expr)));
				Ok(())
			})?;
			Val::Arr(ArrValue::lazy(out))
		}
		Obj(body) => Val::Obj(evaluate_object(ctx, body)?),
		ObjExtend(a, b) => evaluate_add_op(
			&evaluate(ctx.clone(), a)?,
			&Val::Obj(evaluate_object(ctx, b)?),
		)?,
		Apply(value, args, tailstrict) => ensure_sufficient_stack(|| {
			evaluate_apply(ctx, value, args, CallLocation::new(&loc), *tailstrict)
		})?,
		Function(params, body) => {
			evaluate_method(ctx, "anonymous".into(), params.clone(), body.clone())
		}
		AssertExpr(assert, returned) => {
			evaluate_assert(ctx.clone(), assert)?;
			evaluate(ctx, returned)?
		}
		ErrorStmt(e) => in_frame(
			CallLocation::new(&loc),
			|| "error statement".to_owned(),
			|| bail!(RuntimeError(evaluate(ctx, e)?.to_string()?,)),
		)?,
		IfElse {
			cond,
			cond_then,
			cond_else,
		} => {
			if in_frame(
				CallLocation::new(&loc),
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
				ctx: Context,
				expr: Option<&LocExpr>,
				desc: &'static str,
			) -> Result<Option<T>> {
				if let Some(value) = expr {
					Ok(in_frame(
						loc,
						|| format!("slice {desc}"),
						|| <Option<T>>::from_untyped(evaluate(ctx, value)?),
					)?)
				} else {
					Ok(None)
				}
			}

			let indexable = evaluate(ctx.clone(), value)?;
			let loc = CallLocation::new(&loc);

			let start = parse_idx(loc, ctx.clone(), desc.start.as_ref(), "start")?;
			let end = parse_idx(loc, ctx.clone(), desc.end.as_ref(), "end")?;
			let step = parse_idx(loc, ctx, desc.step.as_ref(), "step")?;

			IndexableVal::into_untyped(indexable.into_indexable()?.slice(start, end, step)?)?
		}
		i @ (Import(path) | ImportStr(path) | ImportBin(path)) => {
			let Expr::Str(path) = &path.expr() else {
				bail!("computed imports are not supported")
			};
			let tmp = loc.clone().0;
			with_state(|s| {
				let resolved_path = s.resolve_from(tmp.source_path(), path)?;
				Ok(match i {
					Import(_) => in_frame(
						CallLocation::new(&loc),
						|| format!("import {:?}", path.clone()),
						|| s.import_resolved(resolved_path),
					)?,
					ImportStr(_) => Val::string(s.import_resolved_str(resolved_path)?),
					ImportBin(_) => {
						Val::Arr(ArrValue::bytes(s.import_resolved_bin(resolved_path)?))
					}
					_ => unreachable!(),
				}) as Result<Val>
			})?
		}
	})
}
