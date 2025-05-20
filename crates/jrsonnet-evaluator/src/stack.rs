use std::{cell::Cell, marker::PhantomData};

use crate::error::{Error, ErrorKind};

struct Limit {
	max_stack_size: Cell<usize>,
	current_depth: Cell<usize>,
}

#[cfg(feature = "nightly")]
#[thread_local]
static STACK_LIMIT: Limit = Limit {
	max_stack_size: Cell::new(200),
	current_depth: Cell::new(0),
};
#[cfg(not(feature = "nightly"))]
thread_local! {
	static STACK_LIMIT: Limit = const {
		Limit {
			max_stack_size: Cell::new(200),
			current_depth: Cell::new(0),
		}
	};
}

pub struct OverflowError;
impl From<OverflowError> for ErrorKind {
	fn from(_: OverflowError) -> Self {
		Self::StackOverflow
	}
}
impl From<OverflowError> for Error {
	fn from(_: OverflowError) -> Self {
		ErrorKind::StackOverflow.into()
	}
}

/// Used to implement stack depth limitation
pub struct DepthGuard(PhantomData<()>);
impl Drop for DepthGuard {
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
pub fn check_depth() -> Result<DepthGuard, OverflowError> {
	fn internal(limit: &Limit) -> Result<DepthGuard, OverflowError> {
		let current = limit.current_depth.get();
		if current < limit.max_stack_size.get() {
			limit.current_depth.set(current + 1);
			Ok(DepthGuard(PhantomData))
		} else {
			Err(OverflowError)
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

pub struct DepthLimitOverrideGuard {
	old_limit: usize,
}
impl Drop for DepthLimitOverrideGuard {
	#[cfg(feature = "nightly")]
	fn drop(&mut self) {
		STACK_LIMIT.max_stack_size.set(self.old_limit);
	}
	#[cfg(not(feature = "nightly"))]
	fn drop(&mut self) {
		STACK_LIMIT.with(|limit| limit.max_stack_size.set(self.old_limit));
	}
}

pub fn limit_stack_depth(depth_limit: usize) -> DepthLimitOverrideGuard {
	fn internal(limit: &Limit, depth_limit: usize) -> DepthLimitOverrideGuard {
		let old_limit = limit.max_stack_size.get();
		let current_depth = limit.current_depth.get();

		limit.max_stack_size.set(current_depth + depth_limit);
		DepthLimitOverrideGuard { old_limit }
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
