use crate::{dummy_debug, future_wrapper, BoxedBinding, ObjValue};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

pub trait ContextCreator: Debug {
	fn create_context(&self, this: &Option<ObjValue>, super_obj: &Option<ObjValue>) -> Context;
}
pub type BoxedContextCreator = Rc<dyn ContextCreator>;

#[derive(Debug)]
pub struct ConstantContextCreator {
	pub context: FutureContext,
}
impl ContextCreator for ConstantContextCreator {
	fn create_context(&self, _this: &Option<ObjValue>, _super_obj: &Option<ObjValue>) -> Context {
		self.context.clone().unwrap()
	}
}

future_wrapper!(Context, FutureContext);

#[derive(Debug)]
struct ContextInternals {
	dollar: Option<ObjValue>,
	this: Option<ObjValue>,
	super_obj: Option<ObjValue>,
	bindings: Rc<RefCell<HashMap<String, BoxedBinding>>>,
}
pub struct Context(Rc<ContextInternals>);
dummy_debug!(Context);
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

	pub fn binding(&self, name: &str) -> BoxedBinding {
		self.0
			.bindings
			.borrow()
			.get(name)
			.map(|e| e.clone())
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
		new_bindings: HashMap<String, BoxedBinding>,
		new_dollar: Option<ObjValue>,
		new_this: Option<ObjValue>,
		new_super_obj: Option<ObjValue>,
	) -> Context {
		let dollar = new_dollar.or(self.0.dollar.clone());
		let this = new_this.or(self.0.this.clone());
		let super_obj = new_super_obj.or(self.0.super_obj.clone());
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
