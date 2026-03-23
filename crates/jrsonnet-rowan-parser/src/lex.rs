use jrsonnet_lexer::Lexer;
use rowan::{TextRange, TextSize};

use crate::SyntaxKind;

#[derive(Clone, Copy, Debug)]
pub struct Lexeme<'s> {
	pub kind: SyntaxKind,
	pub text: &'s str,
	pub range: TextRange,
}

pub fn lex(input: &str) -> Vec<Lexeme<'_>> {
	Lexer::new(input)
		.map(|l| Lexeme {
			kind: SyntaxKind::from_raw(l.kind.into_raw()),
			text: l.text,
			range: TextRange::new(TextSize::from(l.range.0), TextSize::from(l.range.1)),
		})
		.collect()
}
