use std::{cell::Cell, marker::PhantomData};

use crate::error::{Error, ErrorKind};

struct StackLimit {
	max_stack_size: Cell<usize>,
	current_depth: Cell<usize>,
}

#[cfg(feature = "nightly")]
#[allow(clippy::thread_local_initializer_can_be_made_const)]
#[thread_local]
static STACK_LIMIT: StackLimit = StackLimit {
	max_stack_size: Cell::new(200),
	current_depth: Cell::new(0),
};
#[cfg(not(feature = "nightly"))]
thread_local! {
	static STACK_LIMIT: StackLimit = const {
		StackLimit {
			max_stack_size: Cell::new(200),
			current_depth: Cell::new(0),
		}
	};
}

pub struct StackOverflowError;
impl From<StackOverflowError> for ErrorKind {
	fn from(_: StackOverflowError) -> Self {
		Self::StackOverflow
	}
}
impl From<StackOverflowError> for Error {
	fn from(_: StackOverflowError) -> Self {
		ErrorKind::StackOverflow.into()
	}
}

/// Used to implement stack depth limitation
pub struct StackDepthGuard(PhantomData<()>);
impl Drop for StackDepthGuard {
	#[cfg(feature = "nightly")]
	fn drop(&mut self) {
		STACK_LIMIT
			.current_depth
			.set(STACK_LIMIT.current_depth.get() - 1);
	}
	#[cfg(not(feature = "nightly"))]
	fn drop(&mut self) {
		STACK_LIMIT.with(|limit| limit.current_depth.set(limit.current_depth.get() - 1));
	}
}

// #[cfg(feature = "nightly")]
pub fn check_depth() -> Result<StackDepthGuard, StackOverflowError> {
	fn internal(limit: &StackLimit) -> Result<StackDepthGuard, StackOverflowError> {
		let current = limit.current_depth.get();
		if current < limit.max_stack_size.get() {
			limit.current_depth.set(current + 1);
			Ok(StackDepthGuard(PhantomData))
		} else {
			Err(StackOverflowError)
		}
	}
	#[cfg(feature = "nightly")]
	{
		internal(&STACK_LIMIT)
	}
	#[cfg(not(feature = "nightly"))]
	{
		STACK_LIMIT.with(internal)
	}
}

pub struct StackDepthLimitOverrideGuard {
	old_limit: usize,
}
impl Drop for StackDepthLimitOverrideGuard {
	#[cfg(feature = "nightly")]
	fn drop(&mut self) {
		STACK_LIMIT.max_stack_size.set(self.old_limit);
	}
	#[cfg(not(feature = "nightly"))]
	fn drop(&mut self) {
		STACK_LIMIT.with(|limit| limit.max_stack_size.set(self.old_limit));
	}
}

pub fn limit_stack_depth(depth_limit: usize) -> StackDepthLimitOverrideGuard {
	fn internal(limit: &StackLimit, depth_limit: usize) -> StackDepthLimitOverrideGuard {
		let old_limit = limit.max_stack_size.get();
		let current_depth = limit.current_depth.get();

		limit.max_stack_size.set(current_depth + depth_limit);
		StackDepthLimitOverrideGuard { old_limit }
	}
	#[cfg(feature = "nightly")]
	{
		internal(&STACK_LIMIT, depth_limit)
	}
	#[cfg(not(feature = "nightly"))]
	{
		STACK_LIMIT.with(|limit| internal(limit, depth_limit))
	}
}

/// Like [`limit_stack_depth`], but set depth is not guarded, and will be kept
///
/// Used to implement `set_max_stack` in C api, prefer to use [`limit_stack_depth`] instead
pub fn set_stack_depth_limit(depth_limit: usize) {
	std::mem::forget(limit_stack_depth(depth_limit));
}
