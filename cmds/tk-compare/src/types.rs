//! Shared types for tk-compare.

/// A pair of values, one for each executable being compared.
#[derive(Debug, Clone)]
pub struct Pair<T> {
	pub first: T,
	pub second: T,
}

impl<T> Pair<T> {
	pub fn new(first: T, second: T) -> Self {
		Self { first, second }
	}
}

impl<T> From<(T, T)> for Pair<T> {
	fn from((first, second): (T, T)) -> Self {
		Self { first, second }
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_pair_new() {
		let pair = Pair::new(1, 2);
		assert_eq!(pair.first, 1);
		assert_eq!(pair.second, 2);
	}

	#[test]
	fn test_pair_from_tuple() {
		let pair: Pair<i32> = (10, 20).into();
		assert_eq!(pair.first, 10);
		assert_eq!(pair.second, 20);
	}
}
