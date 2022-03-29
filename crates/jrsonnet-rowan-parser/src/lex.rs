use crate::string_block::lex_str_block_test;
use core::ops::Range;
use logos::Logos;
use rowan::{Checkpoint, TextRange, TextSize};
use std::{convert::TryFrom, iter::Peekable};

#[derive(Logos, Debug, PartialEq, Hash, Eq, PartialOrd, Ord, Clone, Copy)]
#[repr(u16)]
pub enum SyntaxKind {
	#[token("assert")]
	KeywordAssert = 0,

	#[token("else")]
	KeywordElse,

	#[token("error")]
	KeywordError,

	#[token("false")]
	KeywordFalse,

	#[token("for")]
	KeywordFor,

	#[token("function")]
	KeywordFunction,

	#[token("if")]
	KeywordIf,

	#[token("import")]
	KeywordImport,

	#[token("importstr")]
	KeywordImportStr,

	#[token("local")]
	KeywordLocal,

	#[token("null")]
	KeywordNull,

	#[token("tailstrict")]
	KeywordTailStrict,

	#[token("then")]
	KeywordThen,

	#[token("self")]
	KeywordSelf,

	#[token("super")]
	KeywordSuper,

	#[token("true")]
	KeywordTrue,

	#[regex(r"[_a-zA-Z][_a-zA-Z0-9]*")]
	Ident,

	#[regex(r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?")]
	Number,

	#[regex(r"(?:0|[1-9][0-9]*)\.[^0-9]")]
	ErrorNumJunkAfterDecimalPoint,

	#[regex(r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?[eE][^+\-0-9]")]
	ErrorNumJunkAfterExponent,

	#[regex(r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?[eE][+-][^0-9]")]
	ErrorNumJunkAfterExponentSign,

	#[token("{")]
	SymbolLeftBrace,

	#[token("}")]
	SymbolRightBrace,

	#[token("[")]
	SymbolLeftBracket,

	#[token("]")]
	SymbolRightBracket,

	#[token(",")]
	SymbolComma,

	#[token(".")]
	SymbolDot,

	#[token("(")]
	LParen,

	#[token(")")]
	RParen,

	#[token(";")]
	SymbolSemi,
	#[token(":")]
	SymbolColon,

	#[token("$")]
	SymbolDollar,

	#[token("*")]
	OpMul,
	#[token("/")]
	OpDiv,
	#[token("%")]
	OpMod,
	#[token("+")]
	OpPlus,
	#[token("-")]
	OpMinus,
	#[token("<<")]
	OpShiftLeft,
	#[token(">>")]
	OpShiftRight,
	#[token("<")]
	OpLessThan,
	#[token(">")]
	OpGreaterThan,
	#[token("<=")]
	OpLessThanOrEqual,
	#[token(">=")]
	OpGreaterThanOrEqual,
	#[token("==")]
	OpEqual,
	#[token("!=")]
	OpNotEqual,
	#[token("&")]
	OpBitAnd,
	#[token("^")]
	OpBitXor,
	#[token("|")]
	OpBitOr,
	#[token("&&")]
	OpAnd,
	#[token("||")]
	OpOr,
	#[token("in")]
	OpIn,
	#[token("!")]
	OpNot,
	#[token("~")]
	OpBitNegate,
	#[token("=")]
	SymbolAssign,

	#[regex("\"(?s:[^\"\\\\]|\\\\.)*\"")]
	StringDoubleQuoted,

	#[regex("'(?s:[^'\\\\]|\\\\.)*'")]
	StringSingleQuoted,

	#[regex("@\"(?:[^\"]|\"\")*\"")]
	StringDoubleVerbatim,

	#[regex("@'(?:[^']|'')*'")]
	StringSingleVerbatim,

	#[regex(r"\|\|\|", lex_str_block_test)]
	StringBlock, //(StringBlockToken),

	#[regex("\"(?s:[^\"\\\\]|\\\\.)*")]
	ErrorStringDoubleQuotedUnterminated,

	#[regex("'(?s:[^'\\\\]|\\\\.)*")]
	ErrorStringSingleQuotedUnterminated,

	#[regex("@\"(?:[^\"]|\"\")*")]
	ErrorStringDoubleVerbatimUnterminated,

	#[regex("@'(?:[^']|'')*")]
	ErrorStringSingleVerbatimUnterminated,

	#[regex("@[^\"'\\s]\\S+")]
	ErrorStringMissingQuotes,

	#[token("/*/")]
	ErrorCommentTooShort,

	#[regex(r"/\*([^*]|\*[^/])+")]
	ErrorCommentUnterminated,

	#[regex(r"[ \t\n\r]+")]
	Whitespace,

	#[regex(r"//[^\r\n]*(\r\n|\n)?")]
	SingelLineSlashComment,

	#[regex(r"#[^\r\n]*(\r\n|\n)?")]
	SingleLineHashComment,

	#[regex(r"/\*([^*]|\*[^/])*\*/")]
	MultiLineComment,

	#[error]
	Error,

	ErrorPositionalAfterNamed,

	Literal,
	Expr,
	Array,
	ArrayElem,
	Object,
	Field,

	CompspecFor,
	CompspecIf,

	Slice,
	FieldAccess,
	ObjectApply,
	FunctionCall,
	FunctionDef,
	BodyDef,

	BinOp,
	UnaryOp,
	Local,
	ExprError,
	ExprAssert,
	ExprImport,

	DefParam,
	DefParams,

	DefArgs,
	DefNamedArg,
	DefPositionalArg,

	Parened,

	Root,
}

impl SyntaxKind {
	pub fn is_trivia(self) -> bool {
		matches!(
			self,
			Self::Whitespace
				| Self::MultiLineComment
				| Self::SingelLineSlashComment
				| Self::SingleLineHashComment
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

impl From<SyntaxKind> for rowan::SyntaxKind {
	fn from(kind: SyntaxKind) -> Self {
		Self(kind as u16)
	}
}

use SyntaxKind::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {}
impl rowan::Language for Lang {
	type Kind = SyntaxKind;
	fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
		assert!(raw.0 <= Root as u16);
		unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
	}
	fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
		kind.into()
	}
}
