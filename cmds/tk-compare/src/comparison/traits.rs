//! Core comparison traits and types.

use anyhow::Result;
use rtk_diff::SimilarityScore;

use crate::execution::CommandExecution;

/// Result of comparing two command outputs.
#[derive(Debug)]
pub struct ComparisonResult {
	pub matched: bool,
	pub similarity: SimilarityScore,
}

impl ComparisonResult {
	pub fn new(matched: bool, similarity: SimilarityScore) -> Self {
		Self {
			matched,
			similarity,
		}
	}
}

/// Trait for output comparison strategies.
pub trait Comparer {
	/// Compare two command executions and return the result.
	fn compare(
		&self,
		exec1: &CommandExecution,
		exec2: &CommandExecution,
	) -> Result<ComparisonResult>;
}
