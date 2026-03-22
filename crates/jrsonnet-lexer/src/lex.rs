use core::ops::Range;

use logos::Logos;
// use rowan::{TextRange, TextSize};

use crate::{
	generated::syntax_kinds::SyntaxKind,
	string_block::{lex_str_block, StringBlockError},
	Span,
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

		if kind == Ok(STRING_BLOCK) {
			// We use custom lexer, which skips enough bytes, but not returns error
			// Instead we should call lexer again to verify if there is something wrong with string block
			let mut lexer = logos::Lexer::<SyntaxKind>::new(text);
			// In kinds, string blocks is parsed at least as `|||`
			lexer.bump(3);
			let res = lex_str_block(&mut lexer);
			let next = lexer.next();
			assert!(next.is_none(), "str_block is lexed");
			match res {
				Ok(()) => {}
				Err(e) => {
					kind = Ok(match e {
						StringBlockError::UnexpectedEnd => ERROR_STRING_BLOCK_UNEXPECTED_END,
						StringBlockError::MissingNewLine => ERROR_STRING_BLOCK_MISSING_NEW_LINE,
						StringBlockError::MissingTermination => {
							ERROR_STRING_BLOCK_MISSING_TERMINATION
						}
						StringBlockError::MissingIndent => ERROR_STRING_BLOCK_MISSING_INDENT,
					});
				}
			}
		}

		Some(Self::Item {
			kind: kind.unwrap_or(SyntaxKind::LEXING_ERROR),
			text,
			range: {
				let Range { start, end } = self.inner.span();

				Span(start as u32, end as u32)
			},
		})
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Lexeme<'s> {
	pub kind: SyntaxKind,
	pub text: &'s str,
	pub range: Span,
}

pub fn lex(input: &str) -> Vec<Lexeme<'_>> {
	Lexer::new(input).collect()
}
