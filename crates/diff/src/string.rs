//! Line-based text similarity helpers.

use crate::SimilarityScore;

/// Calculate similarity between two strings based on matching line positions.
///
/// This is intentionally positional rather than edit-distance based: line `N`
/// in the first output is compared to line `N` in the second output.
pub fn calculate_similarity(str1: &str, str2: &str) -> SimilarityScore {
	let lines1: Vec<&str> = str1.lines().collect();
	let lines2: Vec<&str> = str2.lines().collect();

	let max_lines = lines1.len().max(lines2.len());
	if max_lines == 0 {
		return SimilarityScore::perfect();
	}

	let mut matching_lines = 0;
	for i in 0..max_lines {
		if lines1.get(i) == lines2.get(i) {
			matching_lines += 1;
		}
	}

	SimilarityScore::new(matching_lines, max_lines)
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case("hello\nworld", "hello\nworld", 2, 2)]
	#[case("hello\nworld", "hello\nrust", 1, 2)]
	#[case("", "", 0, 0)]
	#[case("a\nb\nc", "a\nx\nc", 2, 3)]
	#[case("single", "single", 1, 1)]
	#[case("single", "different", 0, 1)]
	fn test_calculate_similarity(
		#[case] str1: &str,
		#[case] str2: &str,
		#[case] expected_matched: usize,
		#[case] expected_total: usize,
	) {
		let score = calculate_similarity(str1, str2);
		assert_eq!(score.matched, expected_matched);
		assert_eq!(score.total, expected_total);
	}
}
