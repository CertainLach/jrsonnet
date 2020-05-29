#[macro_export]
macro_rules! dynamic_wrapper {
	($orig: ident, $wrapper: ident) => {
		#[derive(Debug, Clone)]
		pub struct $wrapper(pub std::rc::Rc<dyn $orig>);
		impl std::ops::Deref for $wrapper {
			type Target = dyn $orig;
			fn deref(&self) -> &Self::Target {
				&*self.0
			}
		}
		impl std::cmp::PartialEq for $wrapper {
			fn eq(&self, other: &Self) -> bool {
				Rc::ptr_eq(&self.0, &other.0)
			}
		}
	};
}

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
	};
}

#[macro_export]
macro_rules! dummy_debug {
	($struct: ident) => {
		impl std::fmt::Debug for $struct {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.debug_struct(std::stringify!($struct))
					.finish_non_exhaustive()
			}
		}
	};
}
