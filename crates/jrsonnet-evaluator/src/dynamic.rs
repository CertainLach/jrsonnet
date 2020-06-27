#[macro_export]
macro_rules! future_wrapper {
	($orig: ty, $wrapper: ident) => {
		#[derive(Debug, Clone)]
		pub struct $wrapper(pub std::rc::Rc<std::cell::RefCell<Option<$orig>>>);
		impl $wrapper {
			pub fn unwrap(self) -> $orig {
				self.0.borrow().as_ref().map(|e| e.clone()).unwrap()
			}
			pub fn new() -> Self {
				$wrapper(std::rc::Rc::new(std::cell::RefCell::new(None)))
			}
			pub fn fill(self, val: $orig) -> $orig {
				if self.0.borrow().is_some() {
					panic!("wrapper is filled already");
				}
				{
					self.0.borrow_mut().replace(val);
				}
				self.unwrap()
			}
		}
		impl Default for $wrapper {
			fn default() -> Self {
				Self::new()
			}
		}
	};
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
