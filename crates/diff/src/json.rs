//! JSON structural similarity helpers.

use crate::SimilarityScore;

/// Calculate similarity between two JSON values.
pub fn calculate_similarity(val1: &serde_json::Value, val2: &serde_json::Value) -> SimilarityScore {
	let (matched, total) = calculate_json_similarity_counts(val1, val2);
	SimilarityScore::new(matched, total)
}

/// Calculate `(matched, total)` recursively for two JSON values.
///
/// `total` is the number of comparable paths (object keys / array indices).
/// `matched` is the number of those paths where values are equal.
///
/// Leaf values return `(0, 0)` and are compared by the parent node. For nested
/// structures we add `sub_total - 1` because the parent has already counted the
/// current key/index itself.
fn calculate_json_similarity_counts(
	val1: &serde_json::Value,
	val2: &serde_json::Value,
) -> (usize, usize) {
	use serde_json::Value;

	match (val1, val2) {
		(Value::Object(obj1), Value::Object(obj2)) => {
			let mut matched = 0;
			let mut total = 0;

			let all_keys: std::collections::HashSet<_> = obj1.keys().chain(obj2.keys()).collect();

			for key in all_keys {
				total += 1;
				if let (Some(v1), Some(v2)) = (obj1.get(key), obj2.get(key)) {
					let (sub_matched, sub_total) = calculate_json_similarity_counts(v1, v2);
					if sub_total == 0 {
						if v1 == v2 {
							matched += 1;
						}
					} else {
						matched += sub_matched;
						total += sub_total - 1;
					}
				}
			}

			(matched, total)
		}
		(Value::Array(arr1), Value::Array(arr2)) => {
			let max_len = arr1.len().max(arr2.len());
			let mut matched = 0;
			let mut total = max_len;

			for i in 0..max_len {
				if let (Some(v1), Some(v2)) = (arr1.get(i), arr2.get(i)) {
					let (sub_matched, sub_total) = calculate_json_similarity_counts(v1, v2);
					if sub_total == 0 {
						if v1 == v2 {
							matched += 1;
						}
					} else {
						matched += sub_matched;
						total += sub_total - 1;
					}
				}
			}

			(matched, total)
		}
		_ => (0, 0),
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case(r#"{"a": 1, "b": 2}"#, r#"{"a": 1, "b": 2}"#, 2, 2)]
	#[case(r#"{"a": 1, "b": 2}"#, r#"{"a": 1, "b": 3}"#, 1, 2)]
	#[case(r#"{"a": 1, "b": 2}"#, r#"{"a": 1}"#, 1, 2)]
	#[case(r#"[1, 2, 3]"#, r#"[1, 2, 4]"#, 2, 3)]
	#[case(r#"[1, 2, 3]"#, r#"[1, 2, 3]"#, 3, 3)]
	#[case(r#"{}"#, r#"{}"#, 0, 0)]
	#[case(r#"{"nested": {"a": 1}}"#, r#"{"nested": {"a": 1}}"#, 1, 1)]
	#[case(r#"{"nested": {"a": 1}}"#, r#"{"nested": {"a": 2}}"#, 0, 1)]
	fn test_json_similarity(
		#[case] json1: &str,
		#[case] json2: &str,
		#[case] expected_matched: usize,
		#[case] expected_total: usize,
	) {
		let v1: serde_json::Value = serde_json::from_str(json1).unwrap();
		let v2: serde_json::Value = serde_json::from_str(json2).unwrap();

		let score = calculate_similarity(&v1, &v2);
		assert_eq!(score.matched, expected_matched);
		assert_eq!(score.total, expected_total);
	}
}
