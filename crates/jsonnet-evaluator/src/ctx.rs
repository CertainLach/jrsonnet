use crate::{future_wrapper, rc_fn_helper, LazyBinding, ObjValue, LazyVal, Val};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

rc_fn_helper!(
	ContextCreator,
	context_creator,
	dyn Fn(Option<ObjValue>, Option<ObjValue>) -> Context
);

future_wrapper!(Context, FutureContext);

#[derive(Debug)]
struct ContextInternals {
	dollar: Option<ObjValue>,
	this: Option<ObjValue>,
	super_obj: Option<ObjValue>,
	bindings: Rc<HashMap<String, LazyVal>>,
}
pub struct Context(Rc<ContextInternals>);
impl Debug for Context {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Context")
			.field("this", &self.0.this.clone().map(|e| Rc::as_ptr(&e.0)))
			.finish()
	}
}
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
			bindings: Rc::new(HashMap::new()),
		}))
	}

	pub fn binding(&self, name: &str) -> LazyVal {
		self.0
			.bindings
			.get(name)
			.cloned()
			.unwrap_or_else(|| {
				panic!("can't find {} in {:?}", name, self);
			})
	}
	pub fn into_future(self, ctx: FutureContext) -> Context {
		{
			ctx.0.borrow_mut().replace(self);
		}
		ctx.unwrap()
	}

	pub fn extend(
		&self,
		new_bindings: HashMap<String, LazyBinding>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Context {
		let dollar = new_dollar.or_else(|| self.0.dollar.clone());
		let this = new_this.or_else(|| self.0.this.clone());
		let super_obj = new_super_obj.or_else(|| self.0.super_obj.clone());
		let bindings = if new_bindings.is_empty() {
			self.0.bindings.clone()
		} else {
			let mut new = HashMap::new(); // = self.0.bindings.clone();
			for (k, v) in self.0.bindings.iter() {
				new.insert(k.clone(), v.clone());
			}
			for (k, v) in new_bindings.into_iter() {
				new.insert(k, v.0(this.clone(), super_obj.clone()));
			}
			Rc::new(new)
		};
		Context(Rc::new(ContextInternals {
			dollar,
			this,
			super_obj,
			bindings,
		}))
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

impl Clone for Context {
	fn clone(&self) -> Self {
		Context(self.0.clone())
	}
}
