use std::fmt;

use crate::SyntaxKind;

#[derive(Clone, Copy, Default)]
pub struct SyntaxKindSet(u128);

impl SyntaxKindSet {
	#[allow(dead_code)]
	pub const EMPTY: Self = Self(0);
	pub const ALL: Self = Self(u128::MAX);

	pub const fn new(kinds: &[SyntaxKind]) -> SyntaxKindSet {
		let mut res = 0u128;
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
	pub const fn with(self, kind: SyntaxKind) -> SyntaxKindSet {
		SyntaxKindSet(self.0 | mask(kind))
	}

	pub const fn contains(&self, kind: SyntaxKind) -> bool {
		self.0 & mask(kind) != 0
	}
}
impl fmt::Display for SyntaxKindSet {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut v = self.0;
		let mut variants = <Vec<SyntaxKind>>::new();
		for i in 0..128 {
			if v & 1 == 1 {
				variants.push(SyntaxKind::from_raw(i))
			}
			v >>= 1;
			if v == 0 {
				break;
			}
		}
		for (i, v) in variants.iter().enumerate() {
			if i == 0 {
			} else if i == variants.len() - 1 {
				write!(f, " or ")?;
			} else {
				write!(f, ", ")?;
			}
			write!(f, "{v:?}")?;
		}
		Ok(())
	}
}
impl fmt::Debug for SyntaxKindSet {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut v = self.0;
		let mut variants = <Vec<SyntaxKind>>::new();
		for i in 0..128 {
			if v & 1 == 1 {
				variants.push(SyntaxKind::from_raw(i))
			}
			v >>= 1;
			if v == 0 {
				break;
			}
		}
		f.debug_tuple("SyntaxKindSet").field(&variants).finish()
	}
}

const fn mask(kind: SyntaxKind) -> u128 {
	1u128 << (kind as u128)
}

#[macro_export]
macro_rules! TS {
	($($tt:tt)*) => {
		$crate::SyntaxKindSet::new(&[
			$(
				$crate::T![$tt]
			),*
		])
	};
}

#[test]
fn sanity() {
	assert!(
		(SyntaxKind::LEXING_ERROR as u32) < 127,
		"can't keep KindSet as bitset"
	);
}
