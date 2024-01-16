use std::{cell::OnceCell, ops::Deref};

use boa_gc::{Finalize, Gc, GcRefCell, Trace};
use derivative::Derivative;

use crate::{bail, error::ErrorKind::InfiniteRecursionDetected, val::ThunkValue, Result};

// TODO: Replace with OnceCell
#[derive(Clone, Trace, Finalize)]
pub struct Pending<V: Trace + 'static>(pub Gc<GcRefCell<Option<V>>>);
impl<T: Trace + 'static> Pending<T> {
	pub fn new() -> Self {
		Self(Gc::new(GcRefCell::new(None)))
	}
	pub fn new_filled(v: T) -> Self {
		Self(Gc::new(GcRefCell::new(Some(v))))
	}
	/// # Panics
	/// If wrapper is filled already
	pub fn fill(self, value: T) {
		// TODO: Panic if set
		*self.0.borrow_mut() = Some(value);
	}
}
impl<T: Clone + Trace + 'static> Pending<T> {
	/// # Panics
	/// If wrapper is not yet filled
	pub fn unwrap(&self) -> T {
		self.0.borrow().clone().expect("pending was not filled")
	}
	pub fn try_get(&self) -> Option<T> {
		self.0.borrow().clone()
	}
}

impl<T: Trace + Clone> ThunkValue for Pending<T> {
	type Output = T;

	fn get(self: Box<Self>) -> Result<Self::Output> {
		let v = self.0.borrow();
		let Some(value) = &*v else {
			bail!(InfiniteRecursionDetected);
		};
		Ok(value.clone())
	}
}

impl<T: Trace + 'static> Default for Pending<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Trace, Finalize, Derivative)]
#[derivative(Debug)]
pub struct DynGcBox<T: ?Sized + Trace + Finalize + 'static>(Gc<Box<T>>);
impl<T: ?Sized + Trace + Finalize + 'static> DynGcBox<T> {
	#[doc(hidden)]
	pub fn wrap(v: Gc<Box<T>>) -> Self {
		Self(v)
	}
	pub fn value(&self) -> &T {
		&self.0
	}
}
impl<T: ?Sized + Trace + Finalize + 'static> Clone for DynGcBox<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}
#[macro_export]
macro_rules! dyn_gc_box {
	($t:expr) => {
		$crate::dynamic::DynGcBox::wrap(boa_gc::Gc::new(Box::new($t)))
	};
}

impl<T: ?Sized + Trace + Finalize + 'static> Deref for DynGcBox<T> {
	type Target = Gc<Box<T>>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
