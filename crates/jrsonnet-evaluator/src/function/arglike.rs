use hashbrown::HashMap;
use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, LocExpr, SourceFifo, SourcePath};

use crate::{evaluate, gc::GcHashMap, typed::Typed, Context, Result, Thunk, Val};

/// Marker for arguments, which can be evaluated with context set to None
pub trait OptionalContext {}

pub trait ArgLike {
	fn evaluate_arg(&self, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>>;
}

impl ArgLike for &LocExpr {
	fn evaluate_arg(&self, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>> {
		Ok(if tailstrict {
			Thunk::evaluated(evaluate(ctx, self)?)
		} else {
			let expr = (*self).clone();
			Thunk!(move || evaluate(ctx, &expr))
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
	Val(Val),
	Lazy(Thunk<Val>),
	Import(String),
	ImportStr(String),
	InlineCode(String),
}
impl ArgLike for TlaArg {
	fn evaluate_arg(&self, ctx: Context, _tailstrict: bool) -> Result<Thunk<Val>> {
		match self {
			Self::String(s) => Ok(Thunk::evaluated(Val::string(s.clone()))),
			Self::Val(val) => Ok(Thunk::evaluated(val.clone())),
			Self::Lazy(lazy) => Ok(lazy.clone()),
			Self::Import(p) => {
				let resolved = ctx.state().resolve_from_default(&p.as_str())?;
				Ok(Thunk!(move || ctx.state().import_resolved(resolved)))
			}
			Self::ImportStr(p) => {
				let resolved = ctx.state().resolve_from_default(&p.as_str())?;
				Ok(Thunk!(move || ctx
					.state()
					.import_resolved_str(resolved)
					.map(Val::string)))
			}
			Self::InlineCode(p) => {
				let resolved =
					SourcePath::new(SourceFifo("<inline code>".to_owned(), p.as_bytes().into()));
				Ok(Thunk!(move || ctx.state().import_resolved(resolved)))
			}
		}
	}
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
	fn is_empty(&self) -> bool;
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
	fn is_empty(&self) -> bool {
		self.is_empty()
	}
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
					let ctx = ctx.clone();
					let arg = arg.clone();

					Thunk!(move || evaluate(ctx, &arg))
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
					let ctx = ctx.clone();
					let arg = arg.clone();

					Thunk!(move || evaluate(ctx, &arg))
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

	fn is_empty(&self) -> bool {
		self.unnamed.is_empty() && self.named.is_empty()
	}
}

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

	fn is_empty(&self) -> bool {
		self.is_empty()
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

	fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

macro_rules! impl_args_like {
	($count:expr; $($gen:ident)*) => {
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

			fn is_empty(&self) -> bool {
				// impl_args_like only implements non-empty tuples.
				false
			}
		}
		impl<$($gen: ArgLike,)*> OptionalContext for ($($gen,)*) where $($gen: OptionalContext),* {}
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
	fn is_empty(&self) -> bool {
		true
	}
}
impl OptionalContext for () {}
