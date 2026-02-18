//! Auto-detecting comparer that selects the best comparison strategy.

use anyhow::Result;
use rtk_diff::unified::ParsedUnifiedDiff;

use super::{
	string::StringComparer,
	traits::{Comparer, ComparisonResult},
};
use crate::execution::CommandExecution;

/// Auto-detecting comparer that tries unified diff first, then falls back to string comparison.
pub struct AutoComparer;

impl Comparer for AutoComparer {
	fn compare(
		&self,
		exec1: &CommandExecution,
		exec2: &CommandExecution,
	) -> Result<ComparisonResult> {
		// Try to parse as unified diffs first
		if let (Some(diff1), Some(diff2)) = (
			ParsedUnifiedDiff::parse(&exec1.result.stdout),
			ParsedUnifiedDiff::parse(&exec2.result.stdout),
		) {
			let matched = diff1 == diff2;
			let similarity = diff1.similarity_score(&diff2);
			return Ok(ComparisonResult::new(matched, similarity));
		}

		// Fall back to string comparison
		StringComparer.compare(exec1, exec2)
	}
}
