use gc::{Finalize, Gc, GcCell, Trace};

#[derive(Clone, Trace, Finalize)]
pub struct FutureWrapper<V: Trace + 'static>(pub Gc<GcCell<Option<V>>>);
impl<T: Trace + 'static> FutureWrapper<T> {
	pub fn new() -> Self {
		Self(Gc::new(GcCell::new(None)))
	}
	pub fn fill(self, value: T) {
		assert!(self.0.borrow().is_none(), "wrapper is filled already");
		self.0.borrow_mut().replace(value);
	}
}
impl<T: Clone + Trace + 'static> FutureWrapper<T> {
	pub fn unwrap(&self) -> T {
		self.0.borrow().as_ref().cloned().unwrap()
	}
}

impl<T: Trace + 'static> Default for FutureWrapper<T> {
	fn default() -> Self {
		Self::new()
	}
}
