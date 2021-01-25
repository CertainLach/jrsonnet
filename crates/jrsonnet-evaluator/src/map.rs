use jrsonnet_interner::IStr;
use rustc_hash::FxHashMap;
use std::rc::Rc;

#[derive(Default, Debug)]
struct LayeredHashMapInternals<V> {
	parent: Option<LayeredHashMap<V>>,
	current: FxHashMap<IStr, V>,
}

#[derive(Debug)]
pub struct LayeredHashMap<V>(Rc<LayeredHashMapInternals<V>>);

impl<V> LayeredHashMap<V> {
	pub fn extend(self, new_layer: FxHashMap<IStr, V>) -> Self {
		match Rc::try_unwrap(self.0) {
			Ok(mut map) => {
				map.current.extend(new_layer);
				Self(Rc::new(map))
			}
			Err(this) => Self(Rc::new(LayeredHashMapInternals {
				parent: Some(Self(this)),
				current: new_layer,
			})),
		}
	}

	pub fn get(&self, key: &IStr) -> Option<&V> {
		(self.0)
			.current
			.get(key)
			.or_else(|| self.0.parent.as_ref().and_then(|p| p.get(key)))
	}
}

impl<V> Clone for LayeredHashMap<V> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<V> Default for LayeredHashMap<V> {
	fn default() -> Self {
		Self(Rc::new(LayeredHashMapInternals {
			parent: None,
			current: FxHashMap::default(),
		}))
	}
}
