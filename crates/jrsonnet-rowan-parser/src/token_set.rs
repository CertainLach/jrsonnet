use crate::lex::SyntaxKind;

#[derive(Clone, Copy, Default)]
pub struct TokenSet(u64);

impl TokenSet {
	pub const EMPTY: Self = Self(0);
	pub const ALL: Self = Self(u64::MAX);

	pub const fn new(kinds: &[SyntaxKind]) -> TokenSet {
		let mut res = 0u64;
		let mut i = 0;
		while i < kinds.len() {
			res |= mask(kinds[i]);
			i += 1
		}
		TokenSet(res)
	}

	pub const fn union(self, other: TokenSet) -> TokenSet {
		TokenSet(self.0 | other.0)
	}

	pub const fn contains(&self, kind: SyntaxKind) -> bool {
		self.0 & mask(kind) != 0
	}
}

const fn mask(kind: SyntaxKind) -> u64 {
	1u64 << (kind as usize)
}
