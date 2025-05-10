/// Macros to help deal with Gc
use std::{
	collections::HashSet,
	ops::{Deref, DerefMut},
};

use hashbrown::HashMap;
use jrsonnet_gcmodule::{Trace, Tracer};
use rustc_hash::{FxBuildHasher, FxHashSet};

#[derive(Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct GcHashSet<V>(pub FxHashSet<V>);
impl<V> GcHashSet<V> {
	pub fn new() -> Self {
		Self(HashSet::default())
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self(FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher))
	}
}
impl<V> Trace for GcHashSet<V>
where
	V: Trace,
{
	fn trace(&self, tracer: &mut Tracer<'_>) {
		for v in &self.0 {
			v.trace(tracer);
		}
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

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct GcHashMap<K, V>(pub HashMap<K, V, FxBuildHasher>);
impl<K, V> GcHashMap<K, V> {
	pub fn new() -> Self {
		Self(HashMap::default())
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self(HashMap::with_capacity_and_hasher(capacity, FxBuildHasher))
	}
}
impl<K, V> Trace for GcHashMap<K, V>
where
	K: Trace,
	V: Trace,
{
	fn trace(&self, tracer: &mut Tracer<'_>) {
		for (k, v) in &self.0 {
			k.trace(tracer);
			v.trace(tracer);
		}
	}
}
impl<K, V> Deref for GcHashMap<K, V> {
	type Target = HashMap<K, V, FxBuildHasher>;

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

pub fn assert_trace<T: Trace>(_v: &T) {}
