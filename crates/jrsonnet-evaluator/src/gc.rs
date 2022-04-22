/// Macros to help deal with Gc
use std::{
	borrow::{Borrow, BorrowMut},
	collections::{HashMap, HashSet},
	hash::BuildHasherDefault,
	ops::{Deref, DerefMut},
};

use gcmodule::{Trace, Tracer};
use rustc_hash::{FxHashMap, FxHashSet};

/// Replacement for box, which assumes that the underlying type is [`Trace`]
#[derive(Debug, Clone)]
pub struct TraceBox<T: ?Sized>(pub Box<T>);

impl<T: ?Sized + Trace> Trace for TraceBox<T> {
	fn trace(&self, tracer: &mut Tracer) {
		self.0.trace(tracer);
	}

	fn is_type_tracked() -> bool {
		true
	}
}

// TODO: Replace with CoerceUnsized
impl<T: ?Sized> From<Box<T>> for TraceBox<T> {
	fn from(inner: Box<T>) -> Self {
		Self(inner)
	}
}

impl<T: ?Sized> Deref for TraceBox<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl<T: Trace + ?Sized> DerefMut for TraceBox<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T: ?Sized> Borrow<T> for TraceBox<T> {
	fn borrow(&self) -> &T {
		&*self.0
	}
}

impl<T: ?Sized> BorrowMut<T> for TraceBox<T> {
	fn borrow_mut(&mut self) -> &mut T {
		&mut *self.0
	}
}

impl<T: ?Sized> AsRef<T> for TraceBox<T> {
	fn as_ref(&self) -> &T {
		&*self.0
	}
}

impl<T: ?Sized> AsMut<T> for TraceBox<T> {
	fn as_mut(&mut self) -> &mut T {
		&mut *self.0
	}
}

#[derive(Clone)]
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
impl<V> Trace for GcHashSet<V>
where
	V: Trace,
{
	fn trace(&self, tracer: &mut gcmodule::Tracer) {
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

#[derive(Clone)]
pub struct GcHashMap<K, V>(pub FxHashMap<K, V>);
impl<K, V> GcHashMap<K, V> {
	pub fn new() -> Self {
		Self(HashMap::default())
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self(FxHashMap::with_capacity_and_hasher(
			capacity,
			BuildHasherDefault::default(),
		))
	}
}
impl<K, V> Trace for GcHashMap<K, V>
where
	K: Trace,
	V: Trace,
{
	fn trace(&self, tracer: &mut gcmodule::Tracer) {
		for (k, v) in &self.0 {
			k.trace(tracer);
			v.trace(tracer);
		}
	}
}
impl<K, V> Deref for GcHashMap<K, V> {
	type Target = FxHashMap<K, V>;

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
