use std::collections::HashMap;
use std::rc::Rc;

use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ArgsDesc, Expr, SourceFifo, SourcePath, Spanned};

use crate::{evaluate, typed::Typed, with_state, Context, Result, Thunk, Val};

pub trait ArgLike {
	fn evaluate_arg(&self, ctx: Context, tailstrict: bool) -> Result<Thunk<Val>>;
}

impl ArgLike for &Rc<Spanned<Expr>> {
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

#[derive(Clone, Trace)]
pub enum TlaArg {
	String(IStr),
	Val(Val),
	Lazy(Thunk<Val>),
	Import(String),
	ImportStr(String),
	InlineCode(String),
}
impl TlaArg {
	pub fn evaluate_tailstrict(&self) -> Result<Val> {
		match self {
			Self::String(s) => Ok(Val::string(s.clone())),
			Self::Val(val) => Ok(val.clone()),
			Self::Lazy(lazy) => Ok(lazy.evaluate()?),
			Self::Import(p) => with_state(|s| {
				let resolved = s.resolve_from_default(&p.as_str())?;
				s.import_resolved(resolved)
			}),
			Self::ImportStr(p) => with_state(|s| {
				let resolved = s.resolve_from_default(&p.as_str())?;
				s.import_resolved_str(resolved).map(Val::string)
			}),
			Self::InlineCode(p) => with_state(|s| {
				let resolved =
					SourcePath::new(SourceFifo("<inline code>".to_owned(), p.as_bytes().into()));
				s.import_resolved(resolved)
			}),
		}
	}
	pub fn evaluate(&self) -> Result<Thunk<Val>> {
		match self {
			Self::String(s) => Ok(Thunk::evaluated(Val::string(s.clone()))),
			Self::Val(val) => Ok(Thunk::evaluated(val.clone())),
			Self::Lazy(lazy) => Ok(lazy.clone()),
			Self::Import(p) => with_state(|s| {
				let resolved = s.resolve_from_default(&p.as_str())?;
				Ok(Thunk!(move || s.import_resolved(resolved)))
			}),
			Self::ImportStr(p) => with_state(|s| {
				let resolved = s.resolve_from_default(&p.as_str())?;
				Ok(Thunk!(move || s
					.import_resolved_str(resolved)
					.map(Val::string)))
			}),
			Self::InlineCode(p) => with_state(|s| {
				let resolved =
					SourcePath::new(SourceFifo("<inline code>".to_owned(), p.as_bytes().into()));
				Ok(Thunk!(move || s.import_resolved(resolved)))
			}),
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
