use std::fmt::Debug;

use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;

use crate::{
	cc_ptr_eq, error::Error::*, gc::GcHashMap, map::LayeredHashMap, FutureWrapper, LazyBinding,
	LazyVal, ObjValue, Result, State, Val,
};

#[derive(Clone, Trace)]
pub struct ContextCreator(pub Context, pub FutureWrapper<GcHashMap<IStr, LazyBinding>>);
impl ContextCreator {
	pub fn create(
		&self,
		s: State,
		this: Option<ObjValue>,
		super_obj: Option<ObjValue>,
	) -> Result<Context> {
		self.0.clone().extend_unbound(
			s,
			self.1.clone().unwrap(),
			self.0.dollar().clone().or_else(|| this.clone()),
			this,
			super_obj,
		)
	}
}

#[derive(Trace)]
struct ContextInternals {
	dollar: Option<ObjValue>,
	this: Option<ObjValue>,
	super_obj: Option<ObjValue>,
	bindings: LayeredHashMap,
}
impl Debug for ContextInternals {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Context").finish()
	}
}

#[derive(Debug, Clone, Trace)]
pub struct Context(Cc<ContextInternals>);
impl Context {
	pub fn new_future() -> FutureWrapper<Self> {
		FutureWrapper::new()
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
		Self(Cc::new(ContextInternals {
			dollar: None,
			this: None,
			super_obj: None,
			bindings: LayeredHashMap::default(),
		}))
	}

	pub fn binding(&self, name: IStr) -> Result<LazyVal> {
		Ok(self
			.0
			.bindings
			.get(&name)
			.cloned()
			.ok_or(VariableIsNotDefined(name))?)
	}
	pub fn contains_binding(&self, name: IStr) -> bool {
		self.0.bindings.contains_key(&name)
	}
	pub fn into_future(self, ctx: FutureWrapper<Self>) -> Self {
		{
			ctx.0.borrow_mut().replace(self);
		}
		ctx.unwrap()
	}

	pub fn with_var(self, name: IStr, value: Val) -> Self {
		let mut new_bindings = GcHashMap::with_capacity(1);
		new_bindings.insert(name, LazyVal::new_resolved(value));
		self.extend(new_bindings, None, None, None)
	}

	pub fn with_this_super(self, new_this: ObjValue, new_super_obj: Option<ObjValue>) -> Self {
		self.extend(GcHashMap::new(), None, Some(new_this), new_super_obj)
	}

	pub fn extend(
		self,
		new_bindings: GcHashMap<IStr, LazyVal>,
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
		Self(Cc::new(ContextInternals {
			dollar,
			this,
			super_obj,
			bindings,
		}))
	}
	pub fn extend_bound(self, new_bindings: GcHashMap<IStr, LazyVal>) -> Self {
		let new_this = self.0.this.clone();
		let new_super_obj = self.0.super_obj.clone();
		self.extend(new_bindings, None, new_this, new_super_obj)
	}
	pub fn extend_unbound(
		self,
		s: State,
		new_bindings: GcHashMap<IStr, LazyBinding>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Result<Self> {
		let this = new_this.or_else(|| self.0.this.clone());
		let super_obj = new_super_obj.or_else(|| self.0.super_obj.clone());
		let mut new = GcHashMap::with_capacity(new_bindings.len());
		for (k, v) in new_bindings.0.into_iter() {
			new.insert(k, v.evaluate(s.clone(), this.clone(), super_obj.clone())?);
		}
		Ok(self.extend(new, new_dollar, this, super_obj))
	}
}

impl Default for Context {
	fn default() -> Self {
		Self::new()
	}
}

impl PartialEq for Context {
	fn eq(&self, other: &Self) -> bool {
		cc_ptr_eq(&self.0, &other.0)
	}
}
