use core::ops::Range;
use std::{iter::Enumerate, marker::PhantomData, ops::RangeInclusive};

use logos::{Logos, Span};
use nom::{IResult, InputIter, InputTake, Needed};

// use rowan::{TextRange, TextSize};
use crate::{
	string_block::{lex_str_block, StringBlockError},
	TokenKind::{self, *},
};

#[derive(Clone)]
pub struct Lexer<'a> {
	inner: logos::Lexer<'a, TokenKind>,
}

impl<'a> Lexer<'a> {
	pub fn new(input: &'a str) -> Self {
		Self {
			inner: TokenKind::lexer(input),
		}
	}
}

impl<'a> Iterator for Lexer<'a> {
	type Item = Lexeme<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		use TokenKind::*;

		let mut kind = self.inner.next()?;
		let text = self.inner.slice();

		if kind == Ok(STRING_BLOCK) {
			// We use custom lexer, which skips enough bytes, but not returns error
			// Instead we should call lexer again to verify if there is something wrong with string block
			let mut lexer = logos::Lexer::<TokenKind>::new(text);
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
			kind: kind.unwrap_or(TokenKind::LEXING_ERROR),
			text,
			range: self.inner.span(),
		})
	}
}

#[derive(Clone, Debug)]
pub struct Lexeme<'i> {
	pub kind: TokenKind,
	pub text: &'i str,
	pub range: Span,
}

pub fn lex(input: &str) -> Vec<Lexeme<'_>> {
	Lexer::new(input).collect()
}

// impl<'i> InputIter for Lexer<'i> {
// 	type Item = Lexeme<'i>;
//
// 	type Iter = Enumerate<Self>;
//
// 	type IterElem = Self;
//
// 	fn iter_indices(&self) -> Self::Iter {
// 		self.clone().enumerate()
// 	}
//
// 	fn iter_elements(&self) -> Self::IterElem {
// 		self.clone()
// 	}
//
// 	fn position<P>(&self, predicate: P) -> Option<usize>
// 	where
// 		P: Fn(Self::Item) -> bool,
// 	{
// 		for (o, c) in self.iter_indices() {
// 			if predicate(c) {
// 				return Some(o);
// 			}
// 		}
// 		None
// 	}
//
// 	fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
// 		let mut cnt = 0;
// 		let mut last_end = 0;
// 		for (index, e) in self.iter_indices() {
// 			if cnt == count {
// 				return Ok(index);
// 			}
// 			cnt += 1;
// 			last_end = e.range.end;
// 		}
// 		if cnt == count {
// 			return Ok(last_end);
// 		}
// 		Err(Needed::Unknown)
// 	}
// }
// impl InputTake for Lexer<'i> {
// 	fn take(&self, count: usize) -> Self {
// 		let lex = self.inner.clone();
// 		lex.
// 	}
//
// 	fn take_split(&self, count: usize) -> (Self, Self) {
// 		todo!()
// 	}
// }
//
// fn parse_tok(i: Lexer<'_>) -> IResult<Lexer<'_>, ()> {}
