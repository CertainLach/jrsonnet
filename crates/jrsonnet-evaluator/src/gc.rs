/// Macros to help deal with Gc
use jrsonnet_gcmodule::Trace;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

pub trait WithCapacityExt {
	fn new() -> Self;
	fn with_capacity(capacity: usize) -> Self;
}
impl<V> WithCapacityExt for FxHashSet<V> {
	fn with_capacity(capacity: usize) -> Self {
		Self::with_capacity_and_hasher(capacity, FxBuildHasher::default())
	}

	fn new() -> Self {
		Self::with_hasher(FxBuildHasher::default())
	}
}
impl<K, V> WithCapacityExt for FxHashMap<K, V> {
	fn with_capacity(capacity: usize) -> Self {
		Self::with_capacity_and_hasher(capacity, FxBuildHasher::default())
	}

	fn new() -> Self {
		Self::with_hasher(FxBuildHasher::default())
	}
}

pub fn assert_trace<T: Trace>(_v: &T) {}
