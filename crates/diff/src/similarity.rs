/// Similarity metrics between two outputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimilarityScore {
	pub percentage: f64,
	pub matched: usize,
	pub total: usize,
}

impl SimilarityScore {
	/// Create a similarity score from raw matched/total counts.
	pub fn new(matched: usize, total: usize) -> Self {
		let percentage = if total > 0 {
			(matched as f64 / total as f64) * 100.0
		} else {
			100.0
		};
		Self {
			percentage,
			matched,
			total,
		}
	}

	/// A perfect score (100% match), used for empty comparisons.
	pub fn perfect() -> Self {
		Self {
			percentage: 100.0,
			matched: 0,
			total: 0,
		}
	}

	/// Returns true when similarity meets or exceeds `threshold`.
	pub fn is_near_match(&self, threshold: f64) -> bool {
		self.percentage >= threshold
	}
}

impl From<(usize, usize)> for SimilarityScore {
	fn from((matched, total): (usize, usize)) -> Self {
		Self::new(matched, total)
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case(75, 100, 75.0)]
	#[case(50, 100, 50.0)]
	#[case(100, 100, 100.0)]
	#[case(0, 100, 0.0)]
	#[case(0, 0, 100.0)]
	fn test_similarity_score_new(
		#[case] matched: usize,
		#[case] total: usize,
		#[case] expected_percentage: f64,
	) {
		let score = SimilarityScore::new(matched, total);
		assert_eq!(score.percentage, expected_percentage);
		assert_eq!(score.matched, matched);
		assert_eq!(score.total, total);
	}

	#[test]
	fn test_similarity_score_perfect() {
		assert_eq!(
			SimilarityScore::perfect(),
			SimilarityScore {
				percentage: 100.0,
				matched: 0,
				total: 0
			}
		);
	}

	#[rstest]
	#[case(995, 1000, 99.0, true)]
	#[case(995, 1000, 99.5, true)]
	#[case(995, 1000, 99.6, false)]
	#[case(100, 100, 100.0, true)]
	#[case(0, 100, 50.0, false)]
	fn test_similarity_score_near_match(
		#[case] matched: usize,
		#[case] total: usize,
		#[case] threshold: f64,
		#[case] expected: bool,
	) {
		let score = SimilarityScore::new(matched, total);
		assert_eq!(score.is_near_match(threshold), expected);
	}
}
