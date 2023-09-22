use rowan::Language;

use crate::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JsonnetLanguage {}
impl Language for JsonnetLanguage {
	type Kind = SyntaxKind;

	fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
		SyntaxKind::from_raw(raw.0)
	}

	fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
		rowan::SyntaxKind(kind.into_raw())
	}
}

pub type SyntaxNode = rowan::SyntaxNode<JsonnetLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<JsonnetLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<JsonnetLanguage>;
pub type SyntaxNodeChildren = rowan::SyntaxNodeChildren<JsonnetLanguage>;
pub type SyntaxElementChildren = rowan::SyntaxElementChildren<JsonnetLanguage>;
pub type PreorderWithTokens = rowan::api::PreorderWithTokens<JsonnetLanguage>;
