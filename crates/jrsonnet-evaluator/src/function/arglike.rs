use std::collections::HashMap;

use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, LocExpr};

use crate::{
	error::Result, evaluate, tb, typed::Typed, val::ThunkValue, Context, State, Thunk, Val,
};

#[derive(Trace)]
struct EvaluateThunk {
	ctx: Context,
	expr: LocExpr,
}
impl ThunkValue for EvaluateThunk {
	type Output = Val;
	fn get(self: Box<Self>, s: State) -> Result<Val> {
		evaluate(s, self.ctx, &self.expr)
	}
}

pub trait ArgLike {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>>;
}

impl ArgLike for &LocExpr {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>> {
		Ok(if tailstrict {
			Thunk::evaluated(evaluate(s, ctx, self)?)
		} else {
			Thunk::new(tb!(EvaluateThunk {
				ctx,
				expr: (*self).clone(),
			}))
		})
	}
}

impl<T> ArgLike for T
where
	T: Typed + Clone,
{
	fn evaluate_arg(&self, s: State, _ctx: Context, _tailstrict: bool) -> Result<Thunk<Val>> {
		let val = T::into_untyped(self.clone(), s)?;
		Ok(Thunk::evaluated(val))
	}
}

#[derive(Clone)]
pub enum TlaArg {
	String(IStr),
	Code(LocExpr),
	Val(Val),
}
impl ArgLike for TlaArg {
	fn evaluate_arg(&self, s: State, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>> {
		match self {
			TlaArg::String(s) => Ok(Thunk::evaluated(Val::Str(s.clone()))),
			TlaArg::Code(code) => Ok(if tailstrict {
				Thunk::evaluated(evaluate(s, ctx, code)?)
			} else {
				Thunk::new(tb!(EvaluateThunk {
					ctx,
					expr: code.clone(),
				}))
			}),
			TlaArg::Val(val) => Ok(Thunk::evaluated(val.clone())),
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
		handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()>;
	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
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
		handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		for (id, arg) in self.unnamed.iter().enumerate() {
			handler(
				id,
				if tailstrict {
					Thunk::evaluated(evaluate(s.clone(), ctx.clone(), arg)?)
				} else {
					Thunk::new(tb!(EvaluateThunk {
						ctx: ctx.clone(),
						expr: arg.clone(),
					}))
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
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		for (name, arg) in &self.named {
			handler(
				name,
				if tailstrict {
					Thunk::evaluated(evaluate(s.clone(), ctx.clone(), arg)?)
				} else {
					Thunk::new(tb!(EvaluateThunk {
						ctx: ctx.clone(),
						expr: arg.clone(),
					}))
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
		_handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		s: State,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
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
				handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
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
				_handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
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
				_handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
			) -> Result<()> {
				Ok(())
			}
			#[allow(non_snake_case)]
			fn named_iter(
				&self,
				s: State,
				ctx: Context,
				tailstrict: bool,
				handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
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
		_handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		_s: State,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
}
