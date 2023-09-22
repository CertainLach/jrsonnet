use core::ops::Range;
use std::convert::TryFrom;

use logos::Logos;
use rowan::{TextRange, TextSize};

use crate::{
	string_block::{lex_str_block, StringBlockError},
	SyntaxKind,
};

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
		use SyntaxKind::*;

		let mut kind = self.inner.next()?;
		let text = self.inner.slice();

		if kind == STRING_BLOCK {
			// We use custom lexer, which skips enough bytes, but not returns error
			// Instead we should call lexer again to verify if there is something wrong with string block
			let mut lexer = logos::Lexer::<SyntaxKind>::new(text);
			// In kinds, string blocks is parsed at least as `|||`
			lexer.bump(3);
			let res = lex_str_block(&mut lexer);
			debug_assert!(lexer.next().is_none(), "str_block is lexed");
			match res {
				Ok(_) => {}
				Err(e) => {
					kind = match e {
						StringBlockError::UnexpectedEnd => ERROR_STRING_BLOCK_UNEXPECTED_END,
						StringBlockError::MissingNewLine => ERROR_STRING_BLOCK_MISSING_NEW_LINE,
						StringBlockError::MissingTermination => {
							ERROR_STRING_BLOCK_MISSING_TERMINATION
						}
						StringBlockError::MissingIndent => ERROR_STRING_BLOCK_MISSING_INDENT,
					}
				}
			}
		}

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
