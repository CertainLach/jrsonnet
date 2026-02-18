//! Output comparison strategies for tk-compare.
//!
//! This module provides different strategies for comparing command outputs,
//! abstracted behind the [`Comparer`] trait.

pub mod auto;
pub mod json;
pub mod string;
pub mod traits;
