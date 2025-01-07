use std::{
	cell::{OnceCell, RefCell},
	fmt::{self, Debug, Formatter},
	hash::Hasher,
	ptr::addr_of,
};

use educe::Educe;
use hashbrown::HashSet;
use jrsonnet_gcmodule::{Cc, Trace};

#[derive(Trace, Educe, Debug)]
#[educe(Clone)]
pub struct Pending<V: Trace + 'static>(pub Cc<OnceCell<V>>);
impl<T: Trace + 'static> Pending<T> {
	pub fn new() -> Self {
		Self(Cc::new(OnceCell::new()))
	}
	pub fn new_filled(v: T) -> Self {
		let cell = OnceCell::new();
		let res = cell.set(v);
		assert!(res.is_ok(), "cell is just constructed, there is no value");
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
	pub fn get(&self) -> &T {
		self.0.get().expect("pending was not filled")
	}
	pub fn try_get(&self) -> Option<T> {
		self.0.get().cloned()
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
thread_local! {
	static DEBUG_OF_THUNK: RefCell<HashSet<usize>> = RefCell::new(HashSet::new())
}

pub fn debug_cyclic<T: std::fmt::Debug + ?Sized>(v: &Cc<T>, fmt: &mut Formatter<'_>) -> fmt::Result {
	let ptr = addr_of!(*v) as usize;
	if DEBUG_OF_THUNK.with_borrow_mut(|v| v.insert(ptr)) {
		Debug::fmt(v, fmt)
	} else {
		write!(fmt, "<loop>")
	}
}

pub fn error_slow_path<O>(
	fast: impl FnOnce() -> crate::Result<O>,
	slow: impl FnOnce() -> crate::Result<O>,
) -> crate::Result<O> {
	match fast() {
		Ok(v) => Ok(v),
		Err(_) => slow(),
	}
}
