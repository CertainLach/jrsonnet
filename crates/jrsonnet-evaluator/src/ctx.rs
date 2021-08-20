use crate::{
	error::Error::*, map::LayeredHashMap, FutureWrapper, LazyBinding, LazyVal, ObjValue, Result,
	Val,
};
use jrsonnet_gc::{Gc, Trace};
use jrsonnet_interner::IStr;
use rustc_hash::FxHashMap;
use std::fmt::Debug;
use std::hash::BuildHasherDefault;

#[derive(Clone, Trace)]
#[trivially_drop]
pub struct ContextCreator(pub Context, pub FutureWrapper<FxHashMap<IStr, LazyBinding>>);
impl ContextCreator {
	pub fn create(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<Context> {
		self.0.clone().extend_unbound(
			self.1.clone().unwrap(),
			self.0.dollar().clone().or_else(|| this.clone()),
			this,
			super_obj,
		)
	}
}

#[derive(Trace)]
#[trivially_drop]
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
#[trivially_drop]
pub struct Context(Gc<ContextInternals>);
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
		Self(Gc::new(ContextInternals {
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
		let mut new_bindings =
			FxHashMap::with_capacity_and_hasher(1, BuildHasherDefault::default());
		new_bindings.insert(name, LazyVal::new_resolved(value));
		self.extend(new_bindings, None, None, None)
	}

	pub fn with_this_super(self, new_this: ObjValue, new_super_obj: Option<ObjValue>) -> Self {
		self.extend(FxHashMap::default(), None, Some(new_this), new_super_obj)
	}

	pub fn extend(
		self,
		new_bindings: FxHashMap<IStr, LazyVal>,
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
	pub fn extend_bound(self, new_bindings: FxHashMap<IStr, LazyVal>) -> Self {
		let new_this = self.0.this.clone();
		let new_super_obj = self.0.super_obj.clone();
		self.extend(new_bindings, None, new_this, new_super_obj)
	}
	pub fn extend_unbound(
		self,
		new_bindings: FxHashMap<IStr, LazyBinding>,
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
