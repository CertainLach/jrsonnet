use std::collections::HashMap;

use gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, LocExpr};

use crate::{
	error::Result, evaluate, gc::TraceBox, typed::Typed, val::LazyValValue, Context, LazyVal,
	State, Val,
};

#[derive(Trace)]
struct EvaluateLazyVal {
	ctx: Context,
	expr: LocExpr,
}
impl LazyValValue for EvaluateLazyVal {
	fn get(self: Box<Self>, s: State) -> Result<Val> {
		evaluate(s, self.ctx, &self.expr)
	}
}

pub trait ArgLike {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<LazyVal>;
}

impl ArgLike for &LocExpr {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<LazyVal> {
		Ok(if tailstrict {
			LazyVal::new_resolved(evaluate(s, ctx, self)?)
		} else {
			LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
				ctx,
				expr: (*self).clone(),
			})))
		})
	}
}

impl<T> ArgLike for T
where
	T: Typed + Clone,
{
	fn evaluate_arg(&self, s: State, _ctx: Context, _tailstrict: bool) -> Result<LazyVal> {
		let val = T::into_untyped(self.clone(), s)?;
		Ok(LazyVal::new_resolved(val))
	}
}

pub enum TlaArg {
	String(IStr),
	Code(LocExpr),
	Val(Val),
}
impl ArgLike for TlaArg {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<LazyVal> {
		match self {
			TlaArg::String(s) => Ok(LazyVal::new_resolved(Val::Str(s.clone()))),
			TlaArg::Code(code) => Ok(if tailstrict {
				LazyVal::new_resolved(evaluate(s, ctx, code)?)
			} else {
				LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
					ctx,
					expr: code.clone(),
				})))
			}),
			TlaArg::Val(val) => Ok(LazyVal::new_resolved(val.clone())),
		}
	}
}

mod sealed {
	/// Implemented for `ArgsLike`, where only unnamed arguments present
	pub trait Unnamed {}
	/// Implemented for `ArgsLike`, where only named arguments present
	pub trait Named {}
}

pub trait ArgsLike {
	fn unnamed_len(&self) -> usize;
	fn unnamed_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()>;
	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()>;
	fn named_names(&self, handler: &mut dyn FnMut(&IStr));
}

impl ArgsLike for ArgsDesc {
	fn unnamed_len(&self) -> usize {
		self.unnamed.len()
	}

	fn unnamed_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (id, arg) in self.unnamed.iter().enumerate() {
			handler(
				id,
				if tailstrict {
					LazyVal::new_resolved(evaluate(s.clone(), ctx.clone(), arg)?)
				} else {
					LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
						ctx: ctx.clone(),
						expr: arg.clone(),
					})))
				},
			)?;
		}
		Ok(())
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (name, arg) in &self.named {
			handler(
				name,
				if tailstrict {
					LazyVal::new_resolved(evaluate(s.clone(), ctx.clone(), arg)?)
				} else {
					LazyVal::new(TraceBox(Box::new(EvaluateLazyVal {
						ctx: ctx.clone(),
						expr: arg.clone(),
					})))
				},
			)?;
		}
		Ok(())
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		for (name, _) in &self.named {
			handler(name);
		}
	}
}

impl<A: ArgLike, S> sealed::Named for HashMap<IStr, A, S> {}
impl<A: ArgLike, S> ArgsLike for HashMap<IStr, A, S> {
	fn unnamed_len(&self) -> usize {
		0
	}

	fn unnamed_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		for (name, value) in self.iter() {
			handler(
				name,
				value.evaluate_arg(s.clone(), ctx.clone(), tailstrict)?,
			)?;
		}
		Ok(())
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		for (name, _) in self.iter() {
			handler(name);
		}
	}
}

macro_rules! impl_args_like {
	($count:expr; $($gen:ident)*) => {
		impl<$($gen: ArgLike,)*> sealed::Unnamed for ($($gen,)*) {}
		impl<$($gen: ArgLike,)*> ArgsLike for ($($gen,)*) {
			fn unnamed_len(&self) -> usize {
				$count
			}
			#[allow(non_snake_case, unused_assignments)]
			fn unnamed_iter(
				&self,
				s: State,
				ctx: Context,
				tailstrict: bool,
				handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
			) -> Result<()> {
				let mut i = 0usize;
				let ($($gen,)*) = self;
				$(
					handler(i, $gen.evaluate_arg(s.clone(), ctx.clone(), tailstrict)?)?;
					i+=1;
				)*
				Ok(())
			}
			fn named_iter(
				&self,
				_s: State,
				_ctx: Context,
				_tailstrict: bool,
				_handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
			) -> Result<()> {
				Ok(())
			}
			fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
		}
		impl<$($gen: ArgLike,)*> sealed::Named for ($((IStr, $gen),)*) {}
		impl<$($gen: ArgLike,)*> ArgsLike for ($((IStr, $gen),)*) {
			fn unnamed_len(&self) -> usize {
				0
			}
			fn unnamed_iter(
				&self,
				_s: State,
				_ctx: Context,
				_tailstrict: bool,
				_handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
			) -> Result<()> {
				Ok(())
			}
			#[allow(non_snake_case)]
			fn named_iter(
				&self,
				s: State,
				ctx: Context,
				tailstrict: bool,
				handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
			) -> Result<()> {
				let ($($gen,)*) = self;
				$(
					handler(&$gen.0, $gen.1.evaluate_arg(s.clone(), ctx.clone(), tailstrict)?)?;
				)*
				Ok(())
			}
			#[allow(non_snake_case)]
			fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
				let ($($gen,)*) = self;
				$(
					handler(&$gen.0);
				)*
			}
		}
	};
	($count:expr; $($cur:ident)* @ $c:ident $($rest:ident)*) => {
		impl_args_like!($count; $($cur)*);
		impl_args_like!($count + 1usize; $($cur)* $c @ $($rest)*);
	};
	($count:expr; $($cur:ident)* @) => {
		impl_args_like!($count; $($cur)*);
	}
}
impl_args_like! {
	0usize; A @ B C D E F G H I J K L
}

impl ArgsLike for () {
	fn unnamed_len(&self) -> usize {
		0
	}

	fn unnamed_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(usize, LazyVal) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(&IStr, LazyVal) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
}
