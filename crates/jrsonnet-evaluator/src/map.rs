use jrsonnet_gc::{Gc, Trace};
use jrsonnet_interner::IStr;
use rustc_hash::FxHashMap;

use crate::LazyVal;

#[derive(Trace)]
#[trivially_drop]
pub struct LayeredHashMapInternals {
	parent: Option<LayeredHashMap>,
	current: FxHashMap<IStr, LazyVal>,
}

#[derive(Trace)]
#[trivially_drop]
pub struct LayeredHashMap(Gc<LayeredHashMapInternals>);

impl LayeredHashMap {
	pub fn extend(self, new_layer: FxHashMap<IStr, LazyVal>) -> Self {
		Self(Gc::new(LayeredHashMapInternals {
			parent: Some(self),
			current: new_layer,
		}))
	}

	pub fn get(&self, key: &IStr) -> Option<&LazyVal> {
		(self.0)
			.current
			.get(key)
			.or_else(|| self.0.parent.as_ref().and_then(|p| p.get(key)))
	}

	pub fn contains_key(&self, key: &IStr) -> bool {
		(self.0).current.contains_key(key)
			|| self
				.0
				.parent
				.as_ref()
				.map(|p| p.contains_key(key))
				.unwrap_or(false)
	}
}

impl Clone for LayeredHashMap {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl Default for LayeredHashMap {
	fn default() -> Self {
		Self(Gc::new(LayeredHashMapInternals {
			parent: None,
			current: FxHashMap::default(),
		}))
	}
}
