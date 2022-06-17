use crate::SyntaxKind;

#[derive(Clone, Copy, Default)]
pub struct SyntaxKindSet(u64);

impl SyntaxKindSet {
	pub const EMPTY: Self = Self(0);
	pub const ALL: Self = Self(u64::MAX);

	pub const fn new(kinds: &[SyntaxKind]) -> SyntaxKindSet {
		let mut res = 0u64;
		let mut i = 0;
		while i < kinds.len() {
			res |= mask(kinds[i]);
			i += 1
		}
		SyntaxKindSet(res)
	}

	pub const fn union(self, other: SyntaxKindSet) -> SyntaxKindSet {
		SyntaxKindSet(self.0 | other.0)
	}

	pub const fn contains(&self, kind: SyntaxKind) -> bool {
		self.0 & mask(kind) != 0
	}
}

const fn mask(kind: SyntaxKind) -> u64 {
	1u64 << (kind as usize)
}

#[macro_export]
macro_rules! TS {
	($($tt:tt)*) => {
		SyntaxKindSet::new(&[
			$(
				T![$tt]
			),*
		])
	};
}
