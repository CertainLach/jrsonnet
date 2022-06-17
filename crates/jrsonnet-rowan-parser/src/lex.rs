use core::ops::Range;
use std::convert::TryFrom;

use logos::Logos;
use rowan::{TextRange, TextSize};

use crate::SyntaxKind;

impl SyntaxKind {
	pub fn is_trivia(self) -> bool {
		matches!(
			self,
			Self::WHITESPACE
				| Self::MULTI_LINE_COMMENT
				| Self::SINGLE_LINE_HASH_COMMENT
				| Self::SINGLE_LINE_SLASH_COMMENT
		)
	}
	pub fn is_string(self) -> bool {
		matches!(
			self,
			Self::STRING_SINGLE
				| Self::STRING_DOUBLE
				| Self::STRING_SINGLE_VERBATIM
				| Self::STRING_DOUBLE_VERBATIM
				| Self::STRING_BLOCK
		)
	}
	pub fn is_number(self) -> bool {
		matches!(self, Self::NUMBER)
	}
	pub fn is_literal(self) -> bool {
		matches!(
			self,
			Self::NULL_KW
				| Self::TRUE_KW | Self::FALSE_KW
				| Self::SELF_KW | Self::DOLLAR
				| Self::SUPER_KW
		)
	}
}

pub struct Lexer<'a> {
	inner: logos::Lexer<'a, SyntaxKind>,
}

impl<'a> Lexer<'a> {
	pub fn new(input: &'a str) -> Self {
		Self {
			inner: SyntaxKind::lexer(input),
		}
	}
}

impl<'a> Iterator for Lexer<'a> {
	type Item = Lexeme<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		let kind = self.inner.next()?;
		let text = self.inner.slice();

		Some(Self::Item {
			kind,
			text,
			range: {
				let Range { start, end } = self.inner.span();

				TextRange::new(
					TextSize::try_from(start).unwrap(),
					TextSize::try_from(end).unwrap(),
				)
			},
		})
	}
}

#[derive(Clone, Copy)]
pub struct Lexeme<'i> {
	pub kind: SyntaxKind,
	pub text: &'i str,
	pub range: TextRange,
}

pub fn lex(input: &str) -> Vec<Lexeme<'_>> {
	Lexer::new(input).collect()
}
