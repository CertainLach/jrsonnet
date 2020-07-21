use std::{borrow::Borrow, collections::HashMap, hash::Hash, rc::Rc};

#[derive(Default, Debug)]
struct LayeredHashMapInternals<K: Hash, V> {
	parent: Option<LayeredHashMap<K, V>>,
	current: HashMap<K, V>,
}

#[derive(Debug)]
pub struct LayeredHashMap<K: Hash, V>(Rc<LayeredHashMapInternals<K, V>>);

impl<K: Hash + Eq, V> LayeredHashMap<K, V> {
	pub fn extend(self, new_layer: HashMap<K, V>) -> Self {
		match Rc::try_unwrap(self.0) {
			Ok(mut map) => {
				map.current.extend(new_layer);
				LayeredHashMap(Rc::new(map))
			}
			Err(this) => LayeredHashMap(Rc::new(LayeredHashMapInternals {
				parent: Some(LayeredHashMap(this)),
				current: new_layer,
			})),
		}
	}

	pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
	where
		K: Borrow<Q>,
		Q: Hash + Eq,
	{
		(self.0)
			.current
			.get(&key)
			.or_else(|| self.0.parent.as_ref().and_then(|p| p.get(key)))
	}
}

impl<K: Hash, V> Clone for LayeredHashMap<K, V> {
	fn clone(&self) -> Self {
		LayeredHashMap(self.0.clone())
	}
}

impl<K: Hash + Eq, V> Default for LayeredHashMap<K, V> {
	fn default() -> Self {
		LayeredHashMap(Rc::new(LayeredHashMapInternals {
			parent: None,
			current: HashMap::new(),
		}))
	}
}
