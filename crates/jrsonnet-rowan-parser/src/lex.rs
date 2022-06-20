use core::ops::Range;
use std::convert::TryFrom;

use logos::Logos;
use rowan::{TextRange, TextSize};

use crate::SyntaxKind;

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

#[derive(Clone, Copy, Debug)]
pub struct Lexeme<'i> {
	pub kind: SyntaxKind,
	pub text: &'i str,
	pub range: TextRange,
}

pub fn lex(input: &str) -> Vec<Lexeme<'_>> {
	Lexer::new(input).collect()
}
