use gc::{Finalize, Gc, Trace};
use jrsonnet_interner::IStr;
use rustc_hash::FxHashMap;

pub struct LayeredHashMapInternals<V: Trace + Finalize + 'static> {
	parent: Option<LayeredHashMap<V>>,
	current: FxHashMap<IStr, V>,
}

unsafe impl<V: Trace + Finalize + 'static> Trace for LayeredHashMapInternals<V> {
	gc::custom_trace!(this, {
		mark(&this.parent);
		mark(&this.current);
	});
}
impl<V: Trace + Finalize + 'static> Finalize for LayeredHashMapInternals<V> {}

#[derive(Trace, Finalize)]
pub struct LayeredHashMap<V: Trace + Finalize + 'static>(Gc<LayeredHashMapInternals<V>>);

impl<V: Trace + 'static> LayeredHashMap<V> {
	pub fn extend(self, new_layer: FxHashMap<IStr, V>) -> Self {
		Self(Gc::new(LayeredHashMapInternals {
			parent: Some(self),
			current: new_layer,
		}))
	}

	pub fn get(&self, key: &IStr) -> Option<&V> {
		(self.0)
			.current
			.get(key)
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
