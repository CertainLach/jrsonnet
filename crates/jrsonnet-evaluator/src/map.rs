use gc::{Finalize, Gc, Trace};
use jrsonnet_parser::GcStr;
use rustc_hash::FxHashMap;

#[derive(Default, Debug, Trace, Finalize)]
struct LayeredHashMapInternals<V: Trace + 'static> {
	parent: Option<LayeredHashMap<V>>,
	current: FxHashMap<GcStr, V>,
}

#[derive(Debug, Finalize)]
pub struct LayeredHashMap<V: Trace + 'static>(Gc<LayeredHashMapInternals<V>>);
unsafe impl<V: Trace + 'static> Trace for LayeredHashMap<V> {
	/// Marks all contained `Gc`s.
	unsafe fn trace(&self) {
		self.0.trace()
	}

	/// Increments the root-count of all contained `Gc`s.
	unsafe fn root(&self) {
		self.0.root()
	}

	/// Decrements the root-count of all contained `Gc`s.
	unsafe fn unroot(&self) {
		self.0.unroot()
	}

	/// Runs Finalize::finalize() on this object and all
	/// contained subobjects
	fn finalize_glue(&self) {
		self.0.finalize_glue()
	}
}

impl<V: Trace + 'static> LayeredHashMap<V> {
	pub fn extend(self, new_layer: FxHashMap<GcStr, V>) -> Self {
		let this = self.0;
		Self(Gc::new(LayeredHashMapInternals {
			parent: Some(Self(this)),
			current: new_layer,
		}))
	}

	pub fn get(&self, key: GcStr) -> Option<&V> {
		(self.0)
			.current
			.get(&key)
			.or_else(|| self.0.parent.as_ref().and_then(|p| p.get(key)))
	}
}

impl<V: Trace + 'static> Clone for LayeredHashMap<V> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<V: Trace + 'static> Default for LayeredHashMap<V> {
	fn default() -> Self {
		Self(Gc::new(LayeredHashMapInternals {
			parent: None,
			current: FxHashMap::default(),
		}))
	}
}
