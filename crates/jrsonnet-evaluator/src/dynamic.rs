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
	pub fn unwrap(self) -> T {
		self.0.borrow().as_ref().cloned().unwrap()
	}
}

impl<T> Default for FutureWrapper<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[macro_export]
macro_rules! rc_fn_helper {
	($name: ident, $macro_name: ident, $fn: ty) => {
		#[derive(Clone)]
		#[doc = "Function wrapper"]
		pub struct $name(pub std::rc::Rc<$fn>);
		impl std::fmt::Debug for $name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.debug_struct(std::stringify!($name)).finish()
			}
		}
		impl std::cmp::PartialEq for $name {
			fn eq(&self, other: &$name) -> bool {
				std::ptr::eq(&self.0, &other.0)
			}
		}
		#[doc = "Macro to ease wrapper creation"]
		#[macro_export]
		macro_rules! $macro_name {
			($val: expr) => {
				$crate::$name(std::rc::Rc::new($val))
			};
		}
	};
}
