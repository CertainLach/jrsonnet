use core::{fmt, panic};
use std::{marker::PhantomData, task::Poll};

use jrsonnet_parser::{ArgsDesc, IStr, LocExpr};

use crate::{
	bail,
	error::{ErrorKind::*, Result},
	function::CallLocation,
	Context, Val,
};

pub enum ApplyTCO<'a> {
	Eval {
		in_expr: Tag<&'a LocExpr>,
		in_ctx: Tag<Context>,
		out_val: Tag<Val>,
	},
	Apply {
		in_ctx: Tag<Context>,
		in_value: Tag<Val>,
		in_args: &'a ArgsDesc,
		in_tailstrict: bool,
		out_val: Tag<Val>,
	},
	PopFrame,
	PushFrame {
		tag: Tag<FrameTCO>,
	},
}
enum FrameTCO {
	FunctionCall { name: Tag<IStr> },
}
impl FrameTCO {
	fn to_string(self, vm: &mut TcVM) -> String {
		match self {
			FrameTCO::FunctionCall { name } => {
				let name = vm.strs.pop(name);
				format!("function <{name}> call")
			}
		}
	}
}

pub struct TcVM<'e> {
	apply: Fifo<ApplyTCO<'e>>,
	exprs: Fifo<&'e LocExpr>,
	vals: Fifo<Val>,
	ctxs: Fifo<Context>,
	strs: Fifo<IStr>,
	frames: Fifo<FrameTCO>,
	#[cfg(debug_assertions)]
	pub(crate) vals_offset: usize,
	#[cfg(debug_assertions)]
	pub(crate) ctxs_offset: usize,
	pub(crate) apply_offset: usize,
	active_frames: Vec<FrameTCO>,

	init_val: Tag<Val>,
}
impl<'e> TcVM<'e> {
	pub fn root(ctx: Context, expr: &'e LocExpr) -> Self {
		let init_ctx = ctx_tag("init");
		let init_val = val_tag("init");
		let init_expr = expr_tag("expr");
		Self {
			exprs: Fifo::single(1, expr, init_expr),

			vals: Fifo::<Val>::with_capacity(1),
			ctxs: Fifo::single(1, ctx, init_ctx),
			apply: Fifo::single(
				1,
				ApplyTCO::Eval {
					in_expr: init_expr,
					in_ctx: init_ctx,
					out_val: init_val,
				},
				apply_tag(),
			),
			strs: Fifo::with_capacity(0),
			frames: Fifo::with_capacity(0),
			apply_offset: 0,
			#[cfg(debug_assertions)]
			ctxs_offset: 0,
			#[cfg(debug_assertions)]
			vals_offset: 0,
			active_frames: vec![],

			init_val,
		}
	}
	fn has_apply(&self) -> bool {
		self.apply.len() > self.apply_offset
	}
	pub fn apply(&mut self, apply: ApplyTCO<'e>) {
		self.apply.push(apply, apply_tag())
	}
	pub fn poll(&mut self) -> Poll<Result<Val>> {
		use ApplyTCO::*;
		if !self.has_apply() {
			panic!("ready tcvm shouldn't be polled again");
		}
		let op = self.apply.pop(apply_tag());

		match op {
			Eval {
				in_expr,
				in_ctx,
				out_val,
			} => super::evaluate_inner(self, in_expr, in_ctx, out_val)?,

			Apply {
				in_ctx,
				in_value,
				in_args,
				in_tailstrict,
				out_val,
			} => {
				let value = self.vals.pop(in_value);
				let ctx = self.ctxs.pop(in_ctx);
				match value {
					Val::Func(f) => {
						self.vals.push(
							f.evaluate(ctx, CallLocation::native(), in_args, in_tailstrict)?,
							out_val,
						);
					}
					v => {
						return Poll::Ready(Err(OnlyFunctionsCanBeCalledGot(v.value_type()).into()))
					}
				}
			}
			PopFrame => {
				self.active_frames.pop();
			}
			PushFrame { tag } => {
				let frame = self.frames.pop(tag);
				self.active_frames.push(frame);
			}
		}
		if self.has_apply() {
			Poll::Pending
		} else {
			Poll::Ready(Ok(self.vals.pop(self.init_val)))
		}
	}
}

pub(crate) struct Fifo<T> {
	data: Vec<(T, Tag<T>)>,
}
impl<T> Fifo<T> {
	pub fn with_capacity(cap: usize) -> Self {
		Self {
			data: Vec::with_capacity(cap),
		}
	}
	pub fn single(cap: usize, data: T, tag: Tag<T>) -> Self {
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
#[inline(always)]
pub(crate) fn apply_tag<'e>() -> Tag<ApplyTCO<'e>> {
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
#[inline(always)]
pub(crate) fn expr_tag<'a>(name: &'static str) -> Tag<&'a LocExpr> {
	#[cfg(debug_assertions)]
	{
		Tag {
			name,
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
#[inline(always)]
pub(crate) fn val_tag(name: &'static str) -> Tag<Val> {
	#[cfg(debug_assertions)]
	{
		Tag {
			name,
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
#[inline(always)]
pub(crate) fn ctx_tag(name: &'static str) -> Tag<Context> {
	#[cfg(debug_assertions)]
	{
		Tag {
			name,
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
pub(crate) fn str_tag(name: &'static str) -> Tag<IStr> {
	#[cfg(debug_assertions)]
	{
		Tag {
			name,
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

pub trait Pop<T> {
	fn pop(&mut self, tag: Tag<T>) -> T;
	fn push(&mut self, tag: Tag<T>, value: T);
}

impl Pop<Context> for TcVM<'_> {
	fn pop(&mut self, tag: Tag<Context>) -> Context {
		self.ctxs.pop(tag)
	}

	fn push(&mut self, tag: Tag<Context>, value: Context) {
		self.ctxs.push(value, tag)
	}
}
impl Pop<Val> for TcVM<'_> {
	fn pop(&mut self, tag: Tag<Val>) -> Val {
		self.vals.pop(tag)
	}

	fn push(&mut self, tag: Tag<Val>, value: Val) {
		self.vals.push(value, tag)
	}
}
impl<'e> Pop<&'e LocExpr> for TcVM<'e> {
	fn pop(&mut self, tag: Tag<&'e LocExpr>) -> &'e LocExpr {
		self.exprs.pop(tag)
	}

	fn push(&mut self, tag: Tag<&'e LocExpr>, value: &'e LocExpr) {
		self.exprs.push(value, tag)
	}
}
