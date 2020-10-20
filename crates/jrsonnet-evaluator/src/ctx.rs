use crate::{
	error::Error::*, future_wrapper, map::LayeredHashMap, LazyBinding, LazyVal, ObjValue, Result,
	Val,
};
use crate::{evaluate::FutureNewBindings, LazyValBody};
use rustc_hash::FxHashMap;
use std::hash::BuildHasherDefault;
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

#[derive(Clone)]
pub enum ContextCreator {
	MemberList {
		context: Context,
		new_bindings: FutureNewBindings,
		/// If false - then created context will have no `this` set
		has_this: bool,
	},
	Future(FutureContext),
}
impl ContextCreator {
	pub fn create(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<Context> {
		Ok(match self {
			Self::MemberList {
				context,
				new_bindings,
				has_this,
			} => {
				if *has_this {
					assert!(this.is_some());
				}
				context.clone().extend_unbound(
					new_bindings.clone().unwrap(),
					context.dollar().clone().or_else(|| this.clone()),
					if *has_this { this } else { None },
					super_obj,
				)?
			}
			Self::Future(future) => future.clone().unwrap(),
		})
	}
}

future_wrapper!(Context, FutureContext);

struct ContextInternals {
	dollar: Option<ObjValue>,
	this: Option<ObjValue>,
	super_obj: Option<ObjValue>,
	bindings: LayeredHashMap<Rc<str>, LazyVal>,
}
impl Debug for ContextInternals {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Context")
			.field("this", &self.this.as_ref().map(|e| Rc::as_ptr(&e.0)))
			.field("bindings", &self.bindings)
			.finish()
	}
}

#[derive(Debug, Clone)]
pub struct Context(Rc<ContextInternals>);
impl Context {
	pub fn new_future() -> FutureContext {
		FutureContext(Rc::new(RefCell::new(None)))
	}

	pub fn dollar(&self) -> &Option<ObjValue> {
		&self.0.dollar
	}

	pub fn this(&self) -> &Option<ObjValue> {
		&self.0.this
	}

	pub fn super_obj(&self) -> &Option<ObjValue> {
		&self.0.super_obj
	}

	pub fn new() -> Self {
		Self(Rc::new(ContextInternals {
			dollar: None,
			this: None,
			super_obj: None,
			bindings: LayeredHashMap::default(),
		}))
	}

	pub fn binding(&self, name: Rc<str>) -> Result<LazyVal> {
		Ok(self
			.0
			.bindings
			.get(&name)
			.cloned()
			.ok_or_else(|| VariableIsNotDefined(name))?)
	}
	pub fn into_future(self, ctx: FutureContext) -> Self {
		{
			ctx.0.borrow_mut().replace(self);
		}
		ctx.unwrap()
	}

	pub fn with_var(self, name: Rc<str>, value: Val) -> Self {
		let mut new_bindings =
			FxHashMap::with_capacity_and_hasher(1, BuildHasherDefault::default());
		new_bindings.insert(name, LazyValBody::Resolved(value).into());
		self.extend(new_bindings, None, None, None)
	}

	pub fn extend(
		self,
		new_bindings: FxHashMap<Rc<str>, LazyVal>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Self {
		match Rc::try_unwrap(self.0) {
			Ok(mut ctx) => {
				// Extended context aren't used by anything else, we can freely mutate it without cloning
				if let Some(dollar) = new_dollar {
					ctx.dollar = Some(dollar);
				}
				if let Some(this) = new_this {
					ctx.this = Some(this);
				}
				if let Some(super_obj) = new_super_obj {
					ctx.super_obj = Some(super_obj);
				}
				if !new_bindings.is_empty() {
					ctx.bindings = ctx.bindings.extend(new_bindings);
				}
				Self(Rc::new(ctx))
			}
			Err(ctx) => {
				let dollar = new_dollar.or_else(|| ctx.dollar.clone());
				let this = new_this.or_else(|| ctx.this.clone());
				let super_obj = new_super_obj.or_else(|| ctx.super_obj.clone());
				let bindings = if new_bindings.is_empty() {
					ctx.bindings.clone()
				} else {
					ctx.bindings.clone().extend(new_bindings)
				};
				Self(Rc::new(ContextInternals {
					dollar,
					this,
					super_obj,
					bindings,
				}))
			}
		}
	}
	pub fn extend_unbound(
		self,
		new_bindings: HashMap<Rc<str>, LazyBinding>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Result<Self> {
		let this = new_this.or_else(|| self.0.this.clone());
		let super_obj = new_super_obj.or_else(|| self.0.super_obj.clone());
		let mut new =
			FxHashMap::with_capacity_and_hasher(new_bindings.len(), BuildHasherDefault::default());
		for (k, v) in new_bindings.into_iter() {
			new.insert(k, v.evaluate(this.clone(), super_obj.clone())?);
		}
		Ok(self.extend(new, new_dollar, this, super_obj))
	}
	#[cfg(feature = "unstable")]
	pub fn into_weak(self) -> WeakContext {
		WeakContext(Rc::downgrade(&self.0))
	}
}

impl Default for Context {
	fn default() -> Self {
		Self::new()
	}
}

impl PartialEq for Context {
	fn eq(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.0, &other.0)
	}
}
