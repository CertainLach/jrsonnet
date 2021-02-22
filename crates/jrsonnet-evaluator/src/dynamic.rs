use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub struct FutureWrapper<V>(pub Rc<RefCell<Option<V>>>);
impl<T> FutureWrapper<T> {
	pub fn new() -> Self {
		Self(Rc::new(RefCell::new(None)))
	}
	pub fn fill(self, value: T) {
		assert!(self.0.borrow().is_none(), "wrapper is filled already");
		self.0.borrow_mut().replace(value);
	}
}
impl<T: Clone> FutureWrapper<T> {
	pub fn unwrap(&self) -> T {
		self.0.borrow().as_ref().cloned().unwrap()
	}
}

impl<T> Default for FutureWrapper<T> {
	fn default() -> Self {
		Self::new()
	}
}
