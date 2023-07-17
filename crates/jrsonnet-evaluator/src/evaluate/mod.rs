use std::{fmt, marker::PhantomData, ops::Deref, rc::Rc};

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_macros::{tco, tcok, tcr};
use jrsonnet_parser::{
	ArgsDesc, AssertStmt, BinaryOpType, BindSpec, CompSpec, Expr, FieldMember, FieldName,
	ForSpecData, IfSpecData, LiteralType, LocExpr, Member, ObjBody, ParamsDesc, UnaryOpType,
};
use jrsonnet_types::ValType;

use self::destructure::destruct;
use crate::{
	arr::ArrValue,
	destructure::evaluate_dest,
	error::{suggest_object_fields, ErrorKind::*},
	evaluate::operator::evaluate_unary_op,
	function::{parse::parse_function_call, CallLocation, FuncDesc, FuncVal},
	operator::evaluate_binary_op_normal,
	throw,
	typed::{CheckType, Typed},
	val::{CachedUnbound, IndexableVal, StrValue, Thunk, ThunkValue},
	Context, GcHashMap, MaybeUnbound, ObjValue, ObjValueBuilder, ObjectAssertion, Pending, Result,
	State, Unbound, Val,
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

	for member in members.iter() {
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

pub(crate) struct Fifo<T> {
	data: Vec<(T, Tag<T>)>,
}
impl<T> Fifo<T> {
	fn with_capacity(cap: usize) -> Self {
		Self {
			data: Vec::with_capacity(cap),
		}
	}
	fn single(cap: usize, data: T, tag: Tag<T>) -> Self {
		// eprintln!(">>> {}", tag.0);
		let mut out = Self {
			data: Vec::with_capacity(cap),
		};
		out.push(data, tag);
		out
	}
	pub(crate) fn push(&mut self, data: T, tag: Tag<T>) {
		// eprintln!(">>> {}", tag.0);
		self.data.push((data, tag));
	}
	#[track_caller]
	pub(crate) fn pop(&mut self, tag: Tag<T>) -> T {
		// eprintln!("<<< {}", tag.0);
		let (data, stag) = self
			.data
			.pop()
			.unwrap_or_else(|| panic!("underflow querying for {tag:?}"));
		// debug_assert doesn't work here, as it always requires PartialEq
		#[cfg(debug_assertions)]
		assert_eq!(
			stag, tag,
			"mismatched expected {tag:?} and actual {stag:?} tags",
		);
		data
	}
	pub(crate) fn is_empty(&self) -> bool {
		self.data.is_empty()
	}
	pub(crate) fn len(&self) -> usize {
		self.data.len()
	}
	pub(crate) fn reserve(&mut self, size: usize) {
		self.data.reserve(size)
	}
}

pub(crate) struct Tag<T> {
	#[cfg(debug_assertions)]
	name: &'static str,
	#[cfg(debug_assertions)]
	id: u64,
	_marker: PhantomData<fn(T)>,
}

#[cfg(debug_assertions)]
impl<T> PartialEq for Tag<T> {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name && self.id == other.id
	}
}
impl<T> fmt::Debug for Tag<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		#[cfg(debug_assertions)]
		{
			write!(f, "Tag({})", self.name)
		}
		#[cfg(not(debug_assertions))]
		{
			write!(f, "UncheckedTag")
		}
	}
}
impl<T> Clone for Tag<T> {
	fn clone(&self) -> Self {
		Self {
			#[cfg(debug_assertions)]
			name: self.name,
			#[cfg(debug_assertions)]
			id: self.id.clone(),
			_marker: self._marker.clone(),
		}
	}
}
impl<T> Copy for Tag<T> {}

#[inline(always)]
pub(crate) fn val_tag(name: &'static str) -> Tag<Val> {
	#[cfg(debug_assertions)]
	{
		static ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
		Tag {
			name,
			id: ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst),
			_marker: PhantomData,
		}
	}
	#[cfg(not(debug_assertions))]
	{
		Tag {
			_marker: PhantomData,
		}
	}
}
#[inline(always)]
pub(crate) fn ctx_tag(name: &'static str) -> Tag<Context> {
	#[cfg(debug_assertions)]
	{
		static ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
		Tag {
			name,
			id: ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst),
			_marker: PhantomData,
		}
	}
	#[cfg(not(debug_assertions))]
	{
		Tag {
			_marker: PhantomData,
		}
	}
}

#[inline(always)]
pub(crate) fn apply_tag<'a>() -> Tag<TailCallApply> {
	#[cfg(debug_assertions)]
	{
		Tag {
			name: "APPLY",
			id: 0,
			_marker: PhantomData,
		}
	}
	#[cfg(not(debug_assertions))]
	{
		Tag {
			_marker: PhantomData,
		}
	}
}

#[derive(Debug)]
pub(crate) enum TailCallApply {
	Eval {
		expr: LocExpr,
		in_ctx: Tag<Context>,
		out_val: Tag<Val>,
	},
	ParseNormalArgs {
		params: ParamsDesc,
		args: ArgsDesc,
		tailstrict: bool,
		def_ctx: Tag<Context>,
		call_ctx: Tag<Context>,
		out_ctx: Tag<Context>,
	},
	IfCond {
		in_val: Tag<Val>,
		then_else_ctx: Tag<Context>,
		then_val: LocExpr,
		else_val: Option<LocExpr>,
		out_val: Tag<Val>,
	},
	ApplyUnknownFn {
		args: ArgsDesc,
		location: CallLocation<'static>,
		tailstrict: bool,
		lhs_val: Tag<Val>,
		call_ctx: Tag<Context>,
		out_val: Tag<Val>,
	},
	ApplyBopSpecial {
		and: bool,
		a_val: Tag<Val>,
		b_ctx: Tag<Context>,
		b_expr: LocExpr,
		out_val: Tag<Val>,
	},
	ApplyBopNormal {
		op: BinaryOpType,
		a_val: Tag<Val>,
		b_val: Tag<Val>,
		out_val: Tag<Val>,
	},
	ApplyUop {
		op: UnaryOpType,
		in_val: Tag<Val>,
		out_val: Tag<Val>,
	},
	EvalObj {
		in_ctx: Tag<Context>,
		obj: RcMap<Expr, ObjBody>,
		out_val: Tag<Val>,
	},
	AssertType {
		in_val: Tag<Val>,
		out_val: Tag<Val>,
		ty: ValType,
	},
	EvalIndex {
		lhs_val: Tag<Val>,
		rhs_val: Tag<Val>,
		out_val: Tag<Val>,
	},
	Unbound {
		invoke: MaybeUnbound,
		sup: Option<ObjValue>,
		this: Option<ObjValue>,
		out_val: Tag<Val>,
	},
}
#[derive(Debug)]
pub(crate) struct RcMap<T: ?Sized, U: ?Sized>(Rc<T>, fn(&T) -> &U);

impl<T: ?Sized, U: ?Sized> RcMap<T, U> {
	fn map(rc: Rc<T>, projection: fn(&T) -> &U) -> Self {
		RcMap(rc, projection)
	}
}

impl<T, U> Deref for RcMap<T, U>
where
	T: ?Sized,
	U: ?Sized,
{
	type Target = U;

	fn deref(&self) -> &Self::Target {
		(self.1)(&*self.0)
	}
}

pub(crate) struct TcVM {
	pub(crate) vals: Fifo<Val>,
	pub(crate) ctxs: Fifo<Context>,
	pub(crate) apply: Fifo<TailCallApply>,

	#[cfg(debug_assertions)]
	pub(crate) vals_offset: usize,
	#[cfg(debug_assertions)]
	pub(crate) ctxs_offset: usize,
	pub(crate) apply_offset: usize,
}
impl TcVM {
	fn has_apply(&self) -> bool {
		self.apply.len() > self.apply_offset
	}
}

#[inline(always)]
fn evaluate_inner(
	tcvm: &mut TcVM,
	expr: LocExpr,
	in_ctx: Tag<Context>,
	out_val: Tag<Val>,
) -> Result<()> {
	use Expr::*;
	let ctx = tcvm.ctxs.pop(in_ctx);

	if let Some(trivial) = evaluate_trivial(&expr) {
		tcvm.vals.push(trivial, out_val);
		return Ok(());
	}
	let LocExpr(expr, loc) = expr;
	tcvm.vals.push(
		match *expr {
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
			Parened(ref e) => {
				tcr!(
					ctx(parened, ctx.clone()),
					Eval {
						expr: e.clone(),
						in_ctx: parened,
						out_val
					}
				);
				return Ok(());
			}
			Str(ref v) => Val::Str(StrValue::Flat(v.clone())),
			Num(v) => Val::new_checked_num(v)?,
			BinaryOp(ref v1, op, ref v2) if matches!(op, BinaryOpType::And | BinaryOpType::Or) => {
				tcr!(
					ctx(in_ctx, ctx.clone()),
					Eval {
						expr: v1.clone(),
						in_ctx,
						out_val: val(a_val),
					},
					ctx(b_ctx, ctx.clone()),
					ApplyBopSpecial {
						a_val,
						b_ctx,
						b_expr: v2.clone(),
						and: op == BinaryOpType::And,
						out_val,
					}
				);
				return Ok(());
			}
			BinaryOp(ref v1, op, ref v2) => {
				// FIXME: short-circuiting binary op
				tcr!(
					ctx(lhsc, ctx.clone()),
					Eval {
						expr: v1.clone(),
						in_ctx: lhsc,
						out_val: val(a_val),
					},
					ctx(rhsc, ctx.clone()),
					Eval {
						expr: v2.clone(),
						in_ctx: rhsc,
						out_val: val(b_val),
					},
					ApplyBopNormal {
						op,
						a_val,
						b_val,
						out_val,
					}
				);
				return Ok(());
			}
			UnaryOp(op, ref v) => {
				tcr!(
					ctx(uop, ctx.clone()),
					Eval {
						expr: v.clone(),
						in_ctx: uop,
						out_val: val(in_val),
					},
					ApplyUop {
						op,
						in_val,
						out_val,
					},
				);
				return Ok(());
			}
			Var(ref name) => State::push(
				CallLocation::new(&loc),
				|| format!("variable <{name}> access"),
				|| ctx.binding(name.clone())?.evaluate(),
			)?,
			Index(LocExpr(ref v, _), ref index)
				if matches!(&**v, Expr::Literal(LiteralType::Super)) =>
			{
				let name = evaluate(ctx.clone(), &index)?;
				let Val::Str(name) = name else {
					throw!(ValueIndexMustBeTypeGot(
						ValType::Obj,
						ValType::Str,
						name.value_type(),
					))
				};
				ctx.super_obj()
					.expect("no super found")
					.get_for(name.into_flat(), ctx.this().expect("no this found").clone())?
					.expect("value not found")
			}
			Index(ref value, ref index) => {
				tcr!(
					ctx(lhsc, ctx.clone()),
					Eval {
						expr: value.clone(),
						in_ctx: lhsc,
						out_val: val(lhs_val),
					},
					ctx(rhsc, ctx.clone()),
					Eval {
						expr: index.clone(),
						in_ctx: rhsc,
						out_val: val(rhs_val),
					},
					EvalIndex {
						lhs_val,
						rhs_val,
						out_val,
					}
				);
				return Ok(());
			}
			LocalExpr(ref bindings, ref returned) => {
				let mut new_bindings: GcHashMap<IStr, Thunk<Val>> =
					GcHashMap::with_capacity(bindings.iter().map(BindSpec::capacity_hint).sum());
				let fctx = Context::new_future();
				for b in bindings {
					evaluate_dest(&b, fctx.clone(), &mut new_bindings)?;
				}
				let ctx = ctx.extend(new_bindings, None, None, None).into_future(fctx);
				evaluate(ctx, &returned.clone())?
			}
			Arr(ref items) => {
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
					Val::Arr(ArrValue::lazy(Cc::new(vec![Thunk::new(ArrayElement {
						ctx,
						item: items[0].clone(),
					})])))
				} else {
					Val::Arr(ArrValue::expr(ctx, items.iter().cloned()))
				}
			}
			ArrComp(ref expr, ref comp_specs) => {
				let mut out = Vec::new();
				evaluate_comp(ctx, &comp_specs, &mut |ctx| {
					out.push(evaluate(ctx, &expr)?);
					Ok(())
				})?;
				Val::Arr(ArrValue::eager(out))
			}
			Obj(ref body) => Val::Obj(evaluate_object(ctx, &body)?),
			ObjExtend(ref a, _) => {
				tcr!(
					ctx(base_ctx, ctx.clone()),
					Eval {
						expr: a.clone(),
						in_ctx: base_ctx,
						out_val: val(lhs),
					},
					ctx(obj_ctx, ctx.clone()),
					EvalObj {
						in_ctx: obj_ctx,
						obj: RcMap::map(expr.clone(), |e| {
							match e {
								ObjExtend(_, v) => v,
								_ => unreachable!(),
							}
						}),
						out_val: val(rhs)
					},
					ApplyBopNormal {
						op: BinaryOpType::Add,
						a_val: lhs,
						b_val: rhs,
						out_val,
					}
				);
				return Ok(());
			}
			Apply(ref value, ref args, tailstrict) => tcok!(
				ctx(in_ctx, ctx.clone()),
				Eval {
					expr: value.clone(),
					in_ctx,
					out_val: val(lhs_val),
				},
				ctx(call_ctx, ctx.clone()),
				ApplyUnknownFn {
					args: args.clone(),
					// TODO: preserve
					location: CallLocation::native(),
					tailstrict,
					call_ctx,
					lhs_val,
					out_val,
				},
			),
			Function(ref params, ref body) => {
				evaluate_method(ctx, "anonymous".into(), params.clone(), body.clone())
			}
			AssertExpr(ref assert, ref returned) => {
				evaluate_assert(ctx.clone(), &assert)?;
				evaluate(ctx, &returned)?
			}
			ErrorStmt(ref e) => State::push(
				CallLocation::new(&loc),
				|| "error statement".to_owned(),
				|| throw!(RuntimeError(evaluate(ctx, &e)?.to_string()?,)),
			)?,
			IfElse {
				ref cond,
				ref cond_then,
				ref cond_else,
			} => {
				tcr!(
					ctx(in_ctx, ctx.clone()),
					Eval {
						expr: cond.0.clone(),
						in_ctx,
						out_val: val(in_val),
					},
					ctx(then_else_ctx, ctx.clone()),
					IfCond {
						in_val: in_val.clone(),
						then_else_ctx,
						then_val: cond_then.clone(),
						else_val: cond_else.clone(),
						out_val
					}
				);
				return Ok(());
			}
			Slice(ref value, ref desc) => {
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

				let indexable = evaluate(ctx.clone(), &value)?;
				let loc = CallLocation::new(&loc);

				let start = parse_idx(loc, &ctx, desc.start.as_ref(), "start")?;
				let end = parse_idx(loc, &ctx, desc.end.as_ref(), "end")?;
				let step = parse_idx(loc, &ctx, desc.step.as_ref(), "step")?;

				IndexableVal::into_untyped(indexable.into_indexable()?.slice(start, end, step)?)?
			}
			ref i @ (Import(ref path) | ImportStr(ref path) | ImportBin(ref path)) => {
				let Expr::Str(path) = &*path.0 else {
						    throw!("computed imports are not supported")
					    };
				let tmp = loc.clone().0;
				let s = ctx.state();
				let resolved_path = s.resolve_from(tmp.source_path(), path as &str)?;
				match i {
					Import(_) => State::push(
						CallLocation::new(&loc),
						|| format!("import {:?}", path.clone()),
						|| s.import_resolved(resolved_path),
					)?,
					ImportStr(_) => Val::Str(StrValue::Flat(s.import_resolved_str(resolved_path)?)),
					ImportBin(_) => {
						Val::Arr(ArrValue::bytes(s.import_resolved_bin(resolved_path)?))
					}
					_ => unreachable!(),
				}
			}
		},
		out_val,
	);
	Ok(())
}

pub fn evaluate_reuse_inlined(tcvm: &mut TcVM, expr: &LocExpr) -> Result<()> {
	use TailCallApply::*;
	while tcvm.has_apply() {
		let op = tcvm.apply.pop(apply_tag());
		match op {
			TailCallApply::Eval {
				expr,
				in_ctx,
				out_val,
			} => evaluate_inner(&mut tcvm, expr, in_ctx, out_val)?,
			TailCallApply::ApplyUnknownFn {
				args,
				location,
				tailstrict,
				lhs_val,
				call_ctx,
				out_val,
			} => {
				let value = tcvm.vals.pop(lhs_val);
				let Val::Func(f) = value else {
					throw!(OnlyFunctionsCanBeCalledGot(value.value_type()));
				};
				match f {
					FuncVal::Normal(desc) => {
						tco!(
							ctx(def_ctx, desc.ctx.clone()),
							ParseNormalArgs {
								params: desc.params.clone(),
								args: args,
								tailstrict,
								def_ctx,
								call_ctx,
								out_ctx: ctx(body_ctx),
							},
							Eval {
								expr: desc.body.clone(),
								in_ctx: body_ctx.clone(),
								out_val
							}
						);
					}
					FuncVal::Builtin(_) | FuncVal::StaticBuiltin(_) | FuncVal::Id => {
						// TODO: Proper TCO optimization for builtins
						let call_ctx = tcvm.ctxs.pop(call_ctx);
						let out = f.evaluate(call_ctx, location, &args, tailstrict)?;
						tcvm.vals.push(out, out_val)
					}
				}
			}
			ParseNormalArgs {
				params,
				args,
				tailstrict,
				def_ctx,
				call_ctx,
				out_ctx,
			} => {
				let definition_context = tcvm.ctxs.pop(def_ctx);
				let lhs_ctx = tcvm.ctxs.pop(call_ctx);
				let ctx =
					parse_function_call(lhs_ctx, definition_context, &params, &args, tailstrict)?;
				tcvm.ctxs.push(ctx, out_ctx);
			}
			IfCond {
				in_val,
				then_else_ctx,
				then_val,
				else_val,
				out_val,
			} => {
				let cond = tcvm.vals.pop(in_val);
				let cond = bool::from_untyped(cond)?;
				if cond {
					tco!(Eval {
						expr: then_val,
						in_ctx: then_else_ctx,
						out_val
					});
				} else if let Some(else_val) = else_val {
					tco!(Eval {
						expr: else_val,
						in_ctx: then_else_ctx,
						out_val
					});
				} else {
					tcvm.ctxs.pop(then_else_ctx);
					tcvm.vals.push(Val::Null, out_val)
				}
			}
			ApplyBopNormal {
				op,
				a_val,
				b_val,
				out_val,
			} => {
				let b = tcvm.vals.pop(b_val);
				let a = tcvm.vals.pop(a_val);
				let v = evaluate_binary_op_normal(&a, op, &b)?;
				tcvm.vals.push(v, out_val);
			}
			ApplyBopSpecial {
				and,
				a_val,
				b_ctx,
				b_expr,
				out_val,
			} => {
				let a = tcvm.vals.pop(a_val);
				let a = bool::from_untyped(a)?;
				let b_ctx = tcvm.ctxs.pop(b_ctx);
				match (and, a) {
					(true, false) => tcvm.vals.push(Val::Bool(false), out_val),
					(false, true) => tcvm.vals.push(Val::Bool(true), out_val),
					(true, _) => tco!(
						ctx(in_ctx, b_ctx),
						Eval {
							expr: b_expr,
							in_ctx,
							out_val: val(res),
						},
						AssertType {
							in_val: res,
							out_val,
							ty: ValType::Bool,
						}
					),
					(false, _) => tco!(
						ctx(in_ctx, b_ctx),
						Eval {
							expr: b_expr,
							in_ctx,
							out_val: val(res),
						},
						AssertType {
							in_val: res,
							out_val,
							ty: ValType::Bool,
						},
					),
				}
			}
			ApplyUop {
				op,
				in_val,
				out_val,
			} => {
				let v = tcvm.vals.pop(in_val);
				let o = evaluate_unary_op(op, &v)?;
				tcvm.vals.push(o, out_val);
			}
			EvalObj {
				in_ctx,
				obj,
				out_val,
			} => {
				let in_ctx = tcvm.ctxs.pop(in_ctx);
				let v = evaluate_object(in_ctx, &obj)?;
				tcvm.vals.push(Val::Obj(v), out_val);
			}
			AssertType {
				in_val,
				out_val,
				ty,
			} => {
				let val = tcvm.vals.pop(in_val);
				ty.check(&val)?;
				tcvm.vals.push(val, out_val)
			}
			EvalIndex {
				lhs_val,
				rhs_val,
				out_val,
			} => {
				let rhs = tcvm.vals.pop(rhs_val);
				let lhs = tcvm.vals.pop(lhs_val);
				let v = match (lhs, rhs) {
					(Val::Obj(v), Val::Str(key)) => match v.get(key.clone().into_flat()) {
						Ok(Some(v)) => v,
						Ok(None) => {
							let suggestions = suggest_object_fields(&v, key.clone().into_flat());

							throw!(NoSuchField(key.clone().into_flat(), suggestions))
						}
						Err(e) => throw!(e),
					},
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
					(Val::Arr(_), Val::Str(n)) => {
						throw!(AttemptedIndexAnArrayWithString(n.into_flat()))
					}
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

					(v, _) => throw!(CantIndexInto(v.value_type())),
				};
				tcvm.vals.push(v, out_val)
			}
			Unbound {
				invoke,
				sup,
				this,
				out_val,
			} => {
				let o = invoke.evaluate(sup, this)?;
				tcvm.vals.push(o, out_val)
			}
		}
	}
	Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn evaluate(ctx: Context, expr: &LocExpr) -> Result<Val> {
	let init_ctx = ctx_tag("init");
	let init_val = val_tag("init");

	let mut tcvm = TcVM {
		vals: Fifo::<Val>::with_capacity(1),
		ctxs: Fifo::single(1, ctx, init_ctx.clone()),
		apply: Fifo::single(
			1,
			Eval {
				expr: expr.clone(),
				in_ctx: init_ctx.clone(),
				out_val: init_val.clone(),
			},
			apply_tag(),
		),
		apply_offset: 0,
		ctxs_offset: 0,
		vals_offset: 0,
	};

	evaluate_reuse_inlined(&mut tcvm, expr);

	debug_assert!(tcvm.ctxs.is_empty(), "ctx remains: {:?}", tcvm.ctxs.data);
	debug_assert!(tcvm.apply.is_empty());
	debug_assert!(tcvm.vals.data.len() == 1);
	Ok(tcvm.vals.pop(init_val))
}
