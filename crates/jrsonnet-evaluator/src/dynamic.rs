use std::{cell::OnceCell, hash::Hasher, ptr::addr_of};

use educe::Educe;
use jrsonnet_gcmodule::{Cc, Trace};

use crate::{bail, error::ErrorKind::InfiniteRecursionDetected, val::ThunkValue, Result};

#[derive(Trace, Educe)]
#[educe(Clone)]
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
			.expect("wrapper is filled already");
	}
}
impl<T: Trace + 'static + Clone> Pending<T> {
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
		let Some(value) = self.0.get() else {
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

pub fn identity_hash<T, H: Hasher>(v: &Cc<T>, hasher: &mut H) {
	hasher.write_usize(addr_of!(**v) as usize);
}
