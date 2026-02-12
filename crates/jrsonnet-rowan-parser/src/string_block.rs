#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringBlockError {
	UnexpectedEnd,
	MissingNewLine,
	MissingTermination,
	MissingIndent,
}

use logos::Lexer;
use StringBlockError::*;

use crate::SyntaxKind;

pub(crate) fn lex_str_block_test<'d>(lex: &mut Lexer<'d, SyntaxKind>) {
	let _ = lex_str_block(lex);
}

pub(crate) struct Context<'a> {
	source: &'a str,
	index: usize,
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

	fn eat_if(&mut self, f: impl Fn(char) -> bool) -> usize {
		if self.peek().map(f).unwrap_or(false) {
			self.index += 1;
			return 1;
		}
		0
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

pub(crate) trait StrBlockLexCtx<'d> {
	fn remainder(&self) -> &'d str;
	fn eat_error(&mut self, ctx: &Context<'d>);
	fn bump_pos(&mut self, s: usize);
	fn mark_truncating(&mut self);
	fn mark_line(&mut self, line: &'d str);
}

impl<'d> StrBlockLexCtx<'d> for Lexer<'d, SyntaxKind> {
	fn remainder(&self) -> &'d str {
		self.remainder()
	}
	fn eat_error(&mut self, ctx: &Context<'d>) {
		let end_index = ctx
			.rest()
			.find("|||")
			.map_or_else(|| ctx.rest().len(), |v| v + 3);
		self.bump(ctx.index + end_index);
	}
	fn bump_pos(&mut self, s: usize) {
		self.bump(s);
	}
	fn mark_truncating(&mut self) {
		// Lexer test doesn't collect anything
	}
	fn mark_line(&mut self, _line: &'d str) {
		// Lexer test doesn't collect anything
	}
}

pub fn collect_lexed_str_block<'s>(
	input: &'s str,
) -> Result<CollectStrBlock<'s>, StringBlockError> {
	let mut collect = CollectStrBlock {
		truncate: false,
		lines: vec![],
		input,
		offset: 0,
	};
	lex_str_block(&mut collect)?;
	Ok(collect)
}

pub struct CollectStrBlock<'s> {
	pub truncate: bool,
	pub lines: Vec<&'s str>,
	input: &'s str,
	offset: usize,
}

impl<'d> StrBlockLexCtx<'d> for CollectStrBlock<'d> {
	fn remainder(&self) -> &'d str {
		self.input
	}

	fn eat_error(&mut self, _ctx: &Context<'d>) {
		// Error will be returned, no need to record it here
	}

	fn bump_pos(&mut self, s: usize) {
		self.offset += s;
	}

	fn mark_truncating(&mut self) {
		self.truncate = true;
	}

	fn mark_line(&mut self, line: &'d str) {
		self.lines.push(line)
	}
}

pub(crate) fn lex_str_block<'a>(lex: &mut impl StrBlockLexCtx<'a>) -> Result<(), StringBlockError> {
	// debug_assert_eq!(lex.slice(), "|||");
	let mut ctx = Context::<'a> {
		source: lex.remainder(),
		index: 0,
	};

	if ctx.eat_if(|v| v == '-') != 0 {
		lex.mark_truncating();
	}

	// Skip whitespaces
	ctx.eat_while(|r| r == ' ' || r == '\t' || r == '\r');

	// Skip \n
	match ctx.next() {
		Some('\n') => (),
		None => {
			lex.eat_error(&ctx);
			return Err(UnexpectedEnd);
		}
		// Text block requires new line after |||.
		Some(_) => {
			lex.eat_error(&ctx);
			return Err(MissingNewLine);
		}
	}

	// Process leading blank lines before calculating string block indent
	while ctx.peek() == Some('\n') {
		ctx.next();
	}

	let mut num_whitespace = check_whitespace(ctx.rest(), ctx.rest());
	let str_block_indent = &ctx.rest()[..num_whitespace];

	if num_whitespace == 0 {
		// Text block's first line must start with whitespace
		lex.eat_error(&ctx);
		return Err(MissingIndent);
	}

	loop {
		debug_assert_ne!(num_whitespace, 0, "Unexpected value for num_whitespace");
		ctx.skip(num_whitespace);

		let line_start = ctx.index;
		let mut line_size = 0;
		loop {
			match ctx.next() {
				None => {
					lex.eat_error(&ctx);
					return Err(UnexpectedEnd);
				}
				Some('\n') => {
					lex.mark_line(&ctx.source[line_start..line_start + line_size]);
					break;
				}
				Some(c) => {
					line_size += c.len_utf8();
				}
			}
		}

		// Skip any blank lines
		while ctx.peek() == Some('\n') {
			lex.mark_line("");
			ctx.next();
		}

		// Look at the next line
		num_whitespace = check_whitespace(str_block_indent, ctx.rest());
		if num_whitespace == 0 {
			// End of the text block
			// let mut term_indent = String::with_capacity(num_whitespace);
			while let Some(' ' | '\t') = ctx.peek() {
				// term_indent.push(
				ctx.next().unwrap();
				// );
			}

			if !ctx.rest().starts_with("|||") {
				if ctx.rest().is_empty() {
					lex.bump_pos(ctx.index);
					return Err(UnexpectedEnd);
				}
				lex.eat_error(&ctx);
				return Err(MissingTermination);
			}

			// Skip '|||'
			ctx.skip(3);
			break;
		}
	}

	lex.bump_pos(ctx.index);
	Ok(())
}
