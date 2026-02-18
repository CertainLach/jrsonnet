//! String-based output comparison.

use anyhow::Result;

use super::traits::{Comparer, ComparisonResult};
use crate::execution::CommandExecution;

/// Compares outputs as plain text, line by line.
pub struct StringComparer;

impl Comparer for StringComparer {
	fn compare(
		&self,
		exec1: &CommandExecution,
		exec2: &CommandExecution,
	) -> Result<ComparisonResult> {
		let matched = exec1.result.stdout == exec2.result.stdout;
		let similarity =
			rtk_diff::string::calculate_similarity(&exec1.result.stdout, &exec2.result.stdout);
		Ok(ComparisonResult::new(matched, similarity))
	}
}
