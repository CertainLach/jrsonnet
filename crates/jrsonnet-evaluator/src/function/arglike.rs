use hashbrown::HashMap;
use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, LocExpr};

use crate::{evaluate, gc::GcHashMap, typed::Typed, val::ThunkValue, Context, Result, Thunk, Val};

/// Marker for arguments, which can be evaluated with context set to None
pub trait OptionalContext {}

#[derive(Trace)]
struct EvaluateThunk {
	ctx: Context,
	expr: LocExpr,
}
impl ThunkValue for EvaluateThunk {
	type Output = Val;
	fn get(self: Box<Self>) -> Result<Val> {
		evaluate(self.ctx, &self.expr)
	}
}

pub trait ArgLike {
	fn evaluate_arg(&self, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>>;
}

impl ArgLike for &LocExpr {
	fn evaluate_arg(&self, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>> {
		Ok(if tailstrict {
			Thunk::evaluated(evaluate(ctx, self)?)
		} else {
			Thunk::new(EvaluateThunk {
				ctx,
				expr: (*self).clone(),
			})
		})
	}
}

impl<T> ArgLike for T
where
	T: Typed + Clone,
{
	fn evaluate_arg(&self, _ctx: Context, tailstrict: bool) -> Result<Thunk<Val>> {
		if T::provides_lazy() && !tailstrict {
			return Ok(T::into_lazy_untyped(self.clone()));
		}
		let val = T::into_untyped(self.clone())?;
		Ok(Thunk::evaluated(val))
	}
}
impl<T> OptionalContext for T where T: Typed + Clone {}

#[derive(Clone, Trace)]
pub enum TlaArg {
	String(IStr),
	Code(LocExpr),
	Val(Val),
	Lazy(Thunk<Val>),
}
impl ArgLike for TlaArg {
	fn evaluate_arg(&self, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>> {
		match self {
			Self::String(s) => Ok(Thunk::evaluated(Val::string(s.clone()))),
			Self::Code(code) => Ok(if tailstrict {
				Thunk::evaluated(evaluate(ctx, code)?)
			} else {
				Thunk::new(EvaluateThunk {
					ctx,
					expr: code.clone(),
				})
			}),
			Self::Val(val) => Ok(Thunk::evaluated(val.clone())),
			Self::Lazy(lazy) => Ok(lazy.clone()),
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
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()>;
	fn named_iter(
		&self,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()>;
	fn named_names(&self, handler: &mut dyn FnMut(&IStr));
}

impl ArgsLike for Vec<Val> {
	fn unnamed_len(&self) -> usize {
		self.len()
	}
	fn unnamed_iter(
		&self,
		_ctx: Context,
		_tailstrict: bool,
		handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		for (idx, el) in self.iter().enumerate() {
			handler(idx, Thunk::evaluated(el.clone()))?;
		}
		Ok(())
	}
	fn named_iter(
		&self,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}
	fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
}

impl ArgsLike for ArgsDesc {
	fn unnamed_len(&self) -> usize {
		self.unnamed.len()
	}

	fn unnamed_iter(
		&self,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		for (id, arg) in self.unnamed.iter().enumerate() {
			handler(
				id,
				if tailstrict {
					Thunk::evaluated(evaluate(ctx.clone(), arg)?)
				} else {
					Thunk::new(EvaluateThunk {
						ctx: ctx.clone(),
						expr: arg.clone(),
					})
				},
			)?;
		}
		Ok(())
	}

	fn named_iter(
		&self,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		for (name, arg) in &self.named {
			handler(
				name,
				if tailstrict {
					Thunk::evaluated(evaluate(ctx.clone(), arg)?)
				} else {
					Thunk::new(EvaluateThunk {
						ctx: ctx.clone(),
						expr: arg.clone(),
					})
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

impl<V: ArgLike, S> sealed::Named for HashMap<IStr, V, S> {}
impl<V: ArgLike, S> ArgsLike for HashMap<IStr, V, S> {
	fn unnamed_len(&self) -> usize {
		0
	}

	fn unnamed_iter(
		&self,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		for (name, value) in self {
			handler(name, value.evaluate_arg(ctx.clone(), tailstrict)?)?;
		}
		Ok(())
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		for (name, _) in self {
			handler(name);
		}
	}
}
impl<V, S> OptionalContext for HashMap<IStr, V, S> where V: ArgLike + OptionalContext {}

impl<A: ArgLike> ArgsLike for GcHashMap<IStr, A> {
	fn unnamed_len(&self) -> usize {
		self.0.unnamed_len()
	}

	fn unnamed_iter(
		&self,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		self.0.unnamed_iter(ctx, tailstrict, handler)
	}

	fn named_iter(
		&self,
		ctx: Context,
		tailstrict: bool,
		handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		self.0.named_iter(ctx, tailstrict, handler)
	}

	fn named_names(&self, handler: &mut dyn FnMut(&IStr)) {
		self.0.named_names(handler);
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
				ctx: Context,
				tailstrict: bool,
				handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
			) -> Result<()> {
				let mut i = 0usize;
				let ($($gen,)*) = self;
				$(
					handler(i, $gen.evaluate_arg(ctx.clone(), tailstrict)?)?;
					i+=1;
				)*
				Ok(())
			}
			fn named_iter(
				&self,
				_ctx: Context,
				_tailstrict: bool,
				_handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
			) -> Result<()> {
				Ok(())
			}
			fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
		}
		impl<$($gen: ArgLike,)*> OptionalContext for ($($gen,)*) where $($gen: OptionalContext),* {}

		impl<$($gen: ArgLike,)*> sealed::Named for ($((IStr, $gen),)*) {}
		impl<$($gen: ArgLike,)*> ArgsLike for ($((IStr, $gen),)*) {
			fn unnamed_len(&self) -> usize {
				0
			}
			fn unnamed_iter(
				&self,
				_ctx: Context,
				_tailstrict: bool,
				_handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
			) -> Result<()> {
				Ok(())
			}
			#[allow(non_snake_case)]
			fn named_iter(
				&self,
				ctx: Context,
				tailstrict: bool,
				handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
			) -> Result<()> {
				let ($($gen,)*) = self;
				$(
					handler(&$gen.0, $gen.1.evaluate_arg(ctx.clone(), tailstrict)?)?;
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
		impl<$($gen: ArgLike,)*> OptionalContext for ($((IStr, $gen),)*) where $($gen: OptionalContext),* {}
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
	// First argument is already in position, so count starts from 1
	1usize; A @ B C D E F G H I J K L
}

impl ArgsLike for () {
	fn unnamed_len(&self) -> usize {
		0
	}

	fn unnamed_iter(
		&self,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(usize, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_iter(
		&self,
		_ctx: Context,
		_tailstrict: bool,
		_handler: &mut dyn FnMut(&IStr, Thunk<Val>) -> Result<()>,
	) -> Result<()> {
		Ok(())
	}

	fn named_names(&self, _handler: &mut dyn FnMut(&IStr)) {}
}
impl OptionalContext for () {}
