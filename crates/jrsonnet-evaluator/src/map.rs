use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;

use crate::{GcHashMap, Thunk, Val};

#[derive(Trace)]
#[force_tracking]
pub struct LayeredHashMapInternals {
	parent: Option<LayeredHashMap>,
	current: GcHashMap<IStr, Thunk<Val>>,
}

#[derive(Trace)]
pub struct LayeredHashMap(Cc<LayeredHashMapInternals>);

impl LayeredHashMap {
	pub fn iter_keys(self, mut handler: impl FnMut(IStr)) {
		for (k, _) in self.0.current.iter() {
			handler(k.clone());
		}
		if let Some(parent) = self.0.parent.clone() {
			parent.iter_keys(handler);
		}
	}

	pub fn extend(self, new_layer: GcHashMap<IStr, Thunk<Val>>) -> Self {
		Self(Cc::new(LayeredHashMapInternals {
			parent: Some(self),
			current: new_layer,
		}))
	}

	pub fn get(&self, key: &IStr) -> Option<&Thunk<Val>> {
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
