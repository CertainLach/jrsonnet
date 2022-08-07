use std::cell::RefCell;

use jrsonnet_gcmodule::{Cc, Trace};

// TODO: Replace with OnceCell once in std
#[derive(Clone, Trace)]
pub struct Pending<V: Trace + 'static>(pub Cc<RefCell<Option<V>>>);
impl<T: Trace + 'static> Pending<T> {
	pub fn new() -> Self {
		Self(Cc::new(RefCell::new(None)))
	}
	pub fn new_filled(v: T) -> Self {
		Self(Cc::new(RefCell::new(Some(v))))
	}
	/// # Panics
	/// If wrapper is filled already
	pub fn fill(self, value: T) {
		assert!(self.0.borrow().is_none(), "wrapper is filled already");
		self.0.borrow_mut().replace(value);
	}
}
impl<T: Clone + Trace + 'static> Pending<T> {
	/// # Panics
	/// If wrapper is not yet filled
	pub fn unwrap(&self) -> T {
		self.0.borrow().as_ref().cloned().unwrap()
	}
}

impl<T: Trace + 'static> Default for Pending<T> {
	fn default() -> Self {
		Self::new()
	}
}
