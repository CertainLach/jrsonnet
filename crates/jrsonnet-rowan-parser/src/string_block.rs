#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringBlockError {
	UnexpectedEnd,
	MissingNewLine,
	MissingTermination,
	MissingIndent,
}

use std::ops::Range;

use logos::Lexer;
use StringBlockError::*;

use crate::SyntaxKind;

pub fn lex_str_block_test(lex: &mut Lexer<SyntaxKind>) {
	let _ = lex_str_block(lex);
}

pub fn lex_str_block(lex: &mut Lexer<SyntaxKind>) -> Result<(), StringBlockError> {
	struct Context<'a> {
		source: &'a str,
		index: usize,
		offset: usize,
	}

	impl<'a> Context<'a> {
		fn rest(&self) -> &'a str {
			&self.source[self.index..]
		}

		fn next(&mut self) -> Option<char> {
			if self.index == self.source.len() {
				return None;
			}

			match self.rest().chars().next() {
				None => None,
				Some(c) => {
					self.index += c.len_utf8();
					Some(c)
				}
			}
		}

		fn peek(&self) -> Option<char> {
			if self.index == self.source.len() {
				return None;
			}

			self.rest().chars().next()
		}

		fn eat_while(&mut self, f: impl Fn(char) -> bool) -> usize {
			if self.index == self.source.len() {
				return 0;
			}

			let next_char = self.rest().char_indices().find(|(_, c)| !f(*c));

			match next_char {
				None => {
					let diff = self.source.len() - self.index;
					self.index = self.source.len();
					diff
				}
				Some((idx, _)) => {
					self.index += idx;
					idx
				}
			}
		}

		fn skip(&mut self, len: usize) {
			self.index = match self.index + len {
				n if n > self.source.len() => self.source.len(),
				n => n,
			};
		}

		fn pos(&self) -> Range<usize> {
			if self.index == self.source.len() {
				self.offset + self.index..self.offset + self.index
			} else {
				// TODO: char size
				self.offset + self.index..self.offset + self.index + 1
			}
		}
	}

	// Check that b has at least the same whitespace prefix as a and returns the
	// amount of this whitespace, otherwise returns 0.  If a has no whitespace
	// prefix than return 0.
	fn check_whitespace(a: &str, b: &str) -> usize {
		let a = a.as_bytes();
		let b = b.as_bytes();

		for i in 0..a.len() {
			if a[i] != b' ' && a[i] != b'\t' {
				// a has run out of whitespace and b matched up to this point. Return result.
				return i;
			}

			if i >= b.len() {
				// We ran off the edge of b while a still has whitespace. Return 0 as failure.
				return 0;
			}

			if a[i] != b[i] {
				// a has whitespace but b does not. Return 0 as failure.
				return 0;
			}
		}

		// We ran off the end of a and b kept up
		a.len()
	}

	fn guess_token_end_and_bump<'a>(lex: &mut Lexer<'a, SyntaxKind>, ctx: &Context<'a>) {
		let end_index = ctx
			.rest()
			.find("|||")
			.map(|v| v + 3)
			.unwrap_or_else(|| ctx.rest().len());
		lex.bump(ctx.index + end_index);
	}

	debug_assert_eq!(lex.slice(), "|||");
	let mut ctx = Context {
		source: lex.remainder(),
		index: 0,
		offset: lex.span().end,
	};

	// Skip whitespaces
	ctx.eat_while(|r| r == ' ' || r == '\t' || r == '\r');

	// Skip \n
	match ctx.next() {
		Some('\n') => (),
		None => {
			guess_token_end_and_bump(lex, &ctx);
			return Err(UnexpectedEnd);
		}
		// Text block requires new line after |||.
		Some(_) => {
			guess_token_end_and_bump(lex, &ctx);
			return Err(MissingNewLine);
		}
	}

	// Process leading blank lines before calculating string block indent
	while let Some('\n') = ctx.peek() {
		ctx.next();
	}

	let mut num_whitespace = check_whitespace(ctx.rest(), ctx.rest());
	let str_block_indent = &ctx.rest()[..num_whitespace];

	if num_whitespace == 0 {
		// Text block's first line must start with whitespace
		guess_token_end_and_bump(lex, &ctx);
		return Err(MissingIndent);
	}

	loop {
		debug_assert_ne!(num_whitespace, 0, "Unexpected value for num_whitespace");
		ctx.skip(num_whitespace);

		loop {
			match ctx.next() {
				None => {
					guess_token_end_and_bump(lex, &ctx);
					return Err(UnexpectedEnd);
				}
				Some('\n') => break,
				Some(_) => (),
			}
		}

		// Skip any blank lines
		while let Some('\n') = ctx.peek() {
			ctx.next();
		}

		// Look at the next line
		num_whitespace = check_whitespace(str_block_indent, ctx.rest());
		if num_whitespace == 0 {
			// End of the text block
			let mut term_indent = String::with_capacity(num_whitespace);
			while let Some(' ' | '\t') = ctx.peek() {
				term_indent.push(ctx.next().unwrap());
			}

			if !ctx.rest().starts_with("|||") {
				// Text block not terminated with |||
				let pos = ctx.pos();
				if pos.is_empty() {
					// eof
					lex.bump(ctx.index);
					return Err(UnexpectedEnd);
				}

				guess_token_end_and_bump(lex, &ctx);
				return Err(MissingTermination);
			}

			// Skip '|||'
			ctx.skip(3);
			break;
		}
	}

	lex.bump(ctx.index);
	Ok(())
}
