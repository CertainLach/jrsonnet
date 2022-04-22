use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;

use crate::{GcHashMap, LazyVal};

#[derive(Trace)]
#[force_tracking]
pub struct LayeredHashMapInternals {
	parent: Option<LayeredHashMap>,
	current: GcHashMap<IStr, LazyVal>,
}

#[derive(Trace)]
pub struct LayeredHashMap(Cc<LayeredHashMapInternals>);

impl LayeredHashMap {
	pub fn extend(self, new_layer: GcHashMap<IStr, LazyVal>) -> Self {
		Self(Cc::new(LayeredHashMapInternals {
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
				.map_or(false, |p| p.contains_key(key))
	}
}

impl Clone for LayeredHashMap {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl Default for LayeredHashMap {
	fn default() -> Self {
		Self(Cc::new(LayeredHashMapInternals {
			parent: None,
			current: GcHashMap::new(),
		}))
	}
}
