//! JSON-based output comparison.

use anyhow::{Context, Result};

use super::traits::{Comparer, ComparisonResult};
use crate::execution::CommandExecution;

/// Compares outputs as JSON, performing deep structural comparison.
pub struct JsonComparer;

impl Comparer for JsonComparer {
	fn compare(
		&self,
		exec1: &CommandExecution,
		exec2: &CommandExecution,
	) -> Result<ComparisonResult> {
		let value1: serde_json::Value = serde_json::from_str(&exec1.result.stdout)
			.with_context(|| "Failed to parse first output as JSON")?;
		let value2: serde_json::Value = serde_json::from_str(&exec2.result.stdout)
			.with_context(|| "Failed to parse second output as JSON")?;

		let matched = value1 == value2;
		let similarity = rtk_diff::json::calculate_similarity(&value1, &value2);

		Ok(ComparisonResult::new(matched, similarity))
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	#[rstest]
	#[case(r#"{"a": 1, "b": 2}"#, r#"{"a": 1, "b": 2}"#, 2, 2)]
	#[case(r#"{"a": 1, "b": 2}"#, r#"{"a": 1, "b": 3}"#, 1, 2)]
	#[case(r#"{"a": 1, "b": 2}"#, r#"{"a": 1}"#, 1, 2)]
	#[case(r#"[1, 2, 3]"#, r#"[1, 2, 4]"#, 2, 3)]
	#[case(r#"[1, 2, 3]"#, r#"[1, 2, 3]"#, 3, 3)]
	#[case(r#"{}"#, r#"{}"#, 0, 0)]
	#[case(r#"{"nested": {"a": 1}}"#, r#"{"nested": {"a": 1}}"#, 1, 1)] // nested counts as 1 path
	#[case(r#"{"nested": {"a": 1}}"#, r#"{"nested": {"a": 2}}"#, 0, 1)] // nested counts as 1 path
	fn test_json_similarity(
		#[case] json1: &str,
		#[case] json2: &str,
		#[case] expected_matched: usize,
		#[case] expected_total: usize,
	) {
		let v1: serde_json::Value = serde_json::from_str(json1).unwrap();
		let v2: serde_json::Value = serde_json::from_str(json2).unwrap();

		let score = rtk_diff::json::calculate_similarity(&v1, &v2);
		assert_eq!(score.matched, expected_matched);
		assert_eq!(score.total, expected_total);
	}
}
