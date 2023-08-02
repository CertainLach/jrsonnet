use std::cell::OnceCell;

use jrsonnet_gcmodule::{Cc, Trace};

use crate::{val::ThunkValue, Result, Thunk};

// TODO: Replace with OnceCell once in std
#[derive(Clone, Trace)]
pub struct Pending<V: Trace + 'static>(pub Cc<OnceCell<V>>);
impl<T: Trace + 'static> Pending<T> {
	pub fn new() -> Self {
		Self(Cc::new(OnceCell::new()))
	}
	pub fn new_filled(v: T) -> Self {
		let cell = OnceCell::new();
		let _ = cell.set(v);
		Self(Cc::new(cell))
	}
	/// # Panics
	/// If wrapper is filled already
	pub fn fill(self, value: T) {
		self.0
			.set(value)
			.map_err(|_| ())
			.expect("wrapper is filled already")
	}
}
impl<T: Clone + Trace + 'static> Pending<T> {
	/// # Panics
	/// If wrapper is not yet filled
	pub fn unwrap(&self) -> T {
		self.0.get().cloned().expect("pending was not filled")
	}
	pub fn try_get(&self) -> Option<T> {
		self.0.get().cloned()
	}
}

impl<T: Trace + Clone> ThunkValue for Pending<T> {
	type Output = T;

	fn get(self: Box<Self>) -> Result<Self::Output> {
		return Ok(self.0.get().unwrap().clone());
	}
}

impl<T: Trace + 'static> Default for Pending<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Trace + Clone> Into<Thunk<T>> for Pending<T> {
	fn into(self) -> Thunk<T> {
		Thunk::new(self)
	}
}
