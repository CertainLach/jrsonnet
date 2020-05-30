use crate::{future_wrapper, rc_fn_helper, Binding, ObjValue};
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
	bindings: Rc<RefCell<HashMap<String, Binding>>>,
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
			bindings: Rc::new(RefCell::new(HashMap::new())),
		}))
	}

	pub fn binding(&self, name: &str) -> Binding {
		self.0
			.bindings
			.borrow()
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
		new_bindings: HashMap<String, Binding>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Context {
		println!("Extend with {:?} {:?}", new_dollar, new_this);
		let dollar = new_dollar.or_else(|| self.0.dollar.clone());
		let this = new_this.or_else(|| self.0.this.clone());
		let super_obj = new_super_obj.or_else(|| self.0.super_obj.clone());
		let bindings = if new_bindings.is_empty() {
			self.0.bindings.clone()
		} else {
			let new = self.0.bindings.clone();
			for (k, v) in new_bindings.into_iter() {
				new.borrow_mut().insert(k, v);
			}
			new
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
