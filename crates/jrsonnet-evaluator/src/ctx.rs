use crate::{
	error::Error::*, future_wrapper, map::LayeredHashMap, LazyBinding, LazyVal, ObjValue, Result,
	Val,
};
use crate::{evaluate::FutureNewBindings, LazyValBody};
use gc::{Finalize, Gc, GcCell, Trace};
use jrsonnet_parser::GcStr;
use rustc_hash::FxHashMap;
use std::hash::BuildHasherDefault;
use std::{collections::HashMap, fmt::Debug};

#[derive(Clone, Trace, Finalize)]
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

#[derive(Trace, Finalize)]
struct ContextInternals {
	dollar: Option<ObjValue>,
	this: Option<ObjValue>,
	super_obj: Option<ObjValue>,
	bindings: LayeredHashMap<LazyVal>,
}
impl Debug for ContextInternals {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Context")
			.field("bindings", &self.bindings)
			.finish()
	}
}

#[derive(Debug, Clone, Trace, Finalize)]
pub struct Context(Gc<ContextInternals>);
impl Context {
	pub fn new_future() -> FutureContext {
		FutureContext(Gc::new(GcCell::new(None)))
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
		Self(Gc::new(ContextInternals {
			dollar: None,
			this: None,
			super_obj: None,
			bindings: LayeredHashMap::default(),
		}))
	}

	pub fn binding(&self, name: GcStr) -> Result<LazyVal> {
		Ok(self
			.0
			.bindings
			.get(name.clone())
			.cloned()
			.ok_or_else(|| VariableIsNotDefined(name))?)
	}
	pub fn into_future(self, ctx: FutureContext) -> Self {
		{
			ctx.0.borrow_mut().replace(self);
		}
		ctx.unwrap()
	}

	pub fn with_var(self, name: GcStr, value: Val) -> Self {
		let mut new_bindings =
			FxHashMap::with_capacity_and_hasher(1, BuildHasherDefault::default());
		new_bindings.insert(name, LazyValBody::Resolved(value).into());
		self.extend(new_bindings, None, None, None)
	}

	pub fn extend(
		self,
		new_bindings: FxHashMap<GcStr, LazyVal>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Self {
		let ctx = &self.0;
		let dollar = new_dollar.or_else(|| ctx.dollar.clone());
		let this = new_this.or_else(|| ctx.this.clone());
		let super_obj = new_super_obj.or_else(|| ctx.super_obj.clone());
		let bindings = if new_bindings.is_empty() {
			ctx.bindings.clone()
		} else {
			ctx.bindings.clone().extend(new_bindings)
		};
		Self(Gc::new(ContextInternals {
			dollar,
			this,
			super_obj,
			bindings,
		}))
	}
	pub fn extend_unbound(
		self,
		new_bindings: HashMap<GcStr, LazyBinding>,
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
		Gc::ptr_eq(&self.0, &other.0)
	}
}
