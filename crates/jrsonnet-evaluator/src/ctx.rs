use crate::{
	create_error, future_wrapper, map::LayeredHashMap, rc_fn_helper, resolved_lazy_val, Error,
	LazyBinding, LazyVal, ObjValue, Result, Val,
};
use std::{
	cell::RefCell,
	collections::HashMap,
	fmt::Debug,
	rc::{Rc, Weak},
};

rc_fn_helper!(
	ContextCreator,
	context_creator,
	dyn Fn(Option<ObjValue>, Option<ObjValue>) -> Result<Context>
);

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

	pub fn new() -> Context {
		Context(Rc::new(ContextInternals {
			dollar: None,
			this: None,
			super_obj: None,
			bindings: LayeredHashMap::default(),
		}))
	}

	pub fn binding(&self, name: Rc<str>) -> Result<LazyVal> {
		self.0
			.bindings
			.get(&name)
			.cloned()
			.ok_or_else(|| create_error(Error::UnknownVariable(name)))
	}
	pub fn into_future(self, ctx: FutureContext) -> Context {
		{
			ctx.0.borrow_mut().replace(self);
		}
		ctx.unwrap()
	}

	pub fn with_var(&self, name: Rc<str>, value: Val) -> Result<Context> {
		let mut new_bindings = HashMap::with_capacity(1);
		new_bindings.insert(name, resolved_lazy_val!(value));
		self.extend(new_bindings, None, None, None)
	}

	pub fn extend(
		&self,
		new_bindings: HashMap<Rc<str>, LazyVal>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Result<Context> {
		let dollar = new_dollar.or_else(|| self.0.dollar.clone());
		let this = new_this.or_else(|| self.0.this.clone());
		let super_obj = new_super_obj.or_else(|| self.0.super_obj.clone());
		let bindings = if new_bindings.is_empty() {
			self.0.bindings.clone()
		} else {
			self.0.bindings.extend(new_bindings)
		};
		Ok(Context(Rc::new(ContextInternals {
			dollar,
			this,
			super_obj,
			bindings,
		})))
	}
	pub fn extend_unbound(
		&self,
		new_bindings: HashMap<Rc<str>, LazyBinding>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Result<Context> {
		let this = new_this.or_else(|| self.0.this.clone());
		let super_obj = new_super_obj.or_else(|| self.0.super_obj.clone());
		let mut new = HashMap::with_capacity(new_bindings.len());
		for (k, v) in new_bindings.into_iter() {
			new.insert(k, v.evaluate(this.clone(), super_obj.clone())?);
		}
		self.extend(new, new_dollar, this, super_obj)
	}
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

#[derive(Debug, Clone)]
pub struct WeakContext(Weak<ContextInternals>);
impl WeakContext {
	pub fn upgrade(&self) -> Context {
		Context(self.0.upgrade().expect("context is removed"))
	}
}
impl PartialEq for WeakContext {
	fn eq(&self, other: &Self) -> bool {
		self.0.ptr_eq(&other.0)
	}
}
