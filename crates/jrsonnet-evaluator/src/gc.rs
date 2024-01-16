/// Macros to help deal with Gc
use std::{
	borrow::{Borrow, BorrowMut},
	collections::HashSet,
	hash::BuildHasherDefault,
	ops::{Deref, DerefMut},
};

use boa_gc::{Finalize, Trace, Tracer};
use hashbrown::HashMap;
use rustc_hash::{FxHashSet, FxHasher};

#[derive(Clone, Trace, Finalize)]
pub struct GcHashSet<V>(pub FxHashSet<V>);
impl<V> GcHashSet<V> {
	pub fn new() -> Self {
		Self(HashSet::default())
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self(FxHashSet::with_capacity_and_hasher(
			capacity,
			BuildHasherDefault::default(),
		))
	}
}
impl<V> Deref for GcHashSet<V> {
	type Target = FxHashSet<V>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl<V> DerefMut for GcHashSet<V> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
impl<V> Default for GcHashSet<V> {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Debug, Trace, Finalize)]
pub struct GcHashMap<K, V>(pub HashMap<K, V, BuildHasherDefault<FxHasher>>);
impl<K, V> GcHashMap<K, V> {
	pub fn new() -> Self {
		Self(HashMap::default())
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self(HashMap::with_capacity_and_hasher(
			capacity,
			BuildHasherDefault::default(),
		))
	}
}
impl<K, V> Deref for GcHashMap<K, V> {
	type Target = HashMap<K, V, BuildHasherDefault<FxHasher>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl<K, V> DerefMut for GcHashMap<K, V> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
impl<K, V> Default for GcHashMap<K, V> {
	fn default() -> Self {
		Self::new()
	}
}
