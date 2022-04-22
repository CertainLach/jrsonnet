use std::cell::RefCell;

use gcmodule::{Cc, Trace};

#[derive(Clone, Trace)]
pub struct FutureWrapper<V: Trace + 'static>(pub Cc<RefCell<Option<V>>>);
impl<T: Trace + 'static> FutureWrapper<T> {
	pub fn new() -> Self {
		Self(Cc::new(RefCell::new(None)))
	}
	/// # Panics
	/// If wrapper is filled already
	pub fn fill(self, value: T) {
		assert!(self.0.borrow().is_none(), "wrapper is filled already");
		self.0.borrow_mut().replace(value);
	}
}
impl<T: Clone + Trace + 'static> FutureWrapper<T> {
	/// # Panics
	/// If wrapper is not yet filled
	pub fn unwrap(&self) -> T {
		self.0.borrow().as_ref().cloned().unwrap()
	}
}

impl<T: Trace + 'static> Default for FutureWrapper<T> {
	fn default() -> Self {
		Self::new()
	}
}
