#[macro_export]
macro_rules! future_wrapper {
	($orig: ty, $wrapper: ident) => {
		#[derive(Debug, Clone, gc::Trace, gc::Finalize)]
		pub struct $wrapper(pub gc::Gc<gc::GcCell<Option<$orig>>>);
		impl $wrapper {
			pub fn unwrap(self) -> $orig {
				self.0.borrow().as_ref().map(|e| e.clone()).unwrap()
			}
			pub fn new() -> Self {
				$wrapper(gc::Gc::new(gc::GcCell::new(None)))
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
