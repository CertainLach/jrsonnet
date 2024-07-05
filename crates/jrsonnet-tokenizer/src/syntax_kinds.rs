//! This is a generated file, please do not edit manually. Changes can be
//! made in codegeneration that lives in `xtask` top-level dir.

#![allow(
	bad_style,
	missing_docs,
	unreachable_pub,
	clippy::manual_non_exhaustive,
	clippy::match_like_matches_macro
)]
use logos::Logos;
#[doc = r" The kind of syntax node, e.g. `IDENT`, `USE_KW`, or `STRUCT`."]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Logos)]
#[repr(u16)]
pub enum TokenKind {
	#[doc(hidden)]
	TOMBSTONE,
	#[doc(hidden)]
	EOF,
	#[token("||")]
	OR,
	#[token("??")]
	NULL_COAELSE,
	#[token("&&")]
	AND,
	#[token("|")]
	BIT_OR,
	#[token("^")]
	BIT_XOR,
	#[token("&")]
	BIT_AND,
	#[token("==")]
	EQ,
	#[token("!=")]
	NE,
	#[token("<")]
	LT,
	#[token(">")]
	GT,
	#[token("<=")]
	LE,
	#[token(">=")]
	GE,
	#[token("<<")]
	LHS,
	#[token(">>")]
	RHS,
	#[token("+")]
	PLUS,
	#[token("-")]
	MINUS,
	#[token("*")]
	MUL,
	#[token("/")]
	DIV,
	#[token("%")]
	MODULO,
	#[token("!")]
	NOT,
	#[token("~")]
	BIT_NOT,
	#[token("[")]
	L_BRACK,
	#[token("]")]
	R_BRACK,
	#[token("(")]
	L_PAREN,
	#[token(")")]
	R_PAREN,
	#[token("{")]
	L_BRACE,
	#[token("}")]
	R_BRACE,
	#[token(":")]
	COLON,
	#[token("::")]
	COLONCOLON,
	#[token(":::")]
	COLONCOLONCOLON,
	#[token(";")]
	SEMI,
	#[token(".")]
	DOT,
	#[token("...")]
	DOTDOTDOT,
	#[token(",")]
	COMMA,
	#[token("$")]
	DOLLAR,
	#[token("=")]
	ASSIGN,
	#[token("?")]
	QUESTION_MARK,
	#[regex("(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?")]
	FLOAT,
	#[regex("(?:0|[1-9][0-9]*)\\.[^0-9]")]
	ERROR_FLOAT_JUNK_AFTER_POINT,
	#[regex("(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?[eE][^+\\-0-9]")]
	ERROR_FLOAT_JUNK_AFTER_EXPONENT,
	#[regex("(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?[eE][+-][^0-9]")]
	ERROR_FLOAT_JUNK_AFTER_EXPONENT_SIGN,
	#[regex("\"(?s:[^\"\\\\]|\\\\.)*\"")]
	STRING_DOUBLE,
	#[regex("\"(?s:[^\"\\\\]|\\\\.)*")]
	ERROR_STRING_DOUBLE_UNTERMINATED,
	#[regex("'(?s:[^'\\\\]|\\\\.)*'")]
	STRING_SINGLE,
	#[regex("'(?s:[^'\\\\]|\\\\.)*")]
	ERROR_STRING_SINGLE_UNTERMINATED,
	#[regex("@\"(?:[^\"]|\"\")*\"")]
	STRING_DOUBLE_VERBATIM,
	#[regex("@\"(?:[^\"]|\"\")*")]
	ERROR_STRING_DOUBLE_VERBATIM_UNTERMINATED,
	#[regex("@'(?:[^']|'')*'")]
	STRING_SINGLE_VERBATIM,
	#[regex("@'(?:[^']|'')*")]
	ERROR_STRING_SINGLE_VERBATIM_UNTERMINATED,
	#[regex("@[^\"'\\s]\\S+")]
	ERROR_STRING_VERBATIM_MISSING_QUOTES,
	#[regex("\\|\\|\\|", crate::string_block::lex_str_block_test)]
	STRING_BLOCK,
	ERROR_STRING_BLOCK_UNEXPECTED_END,
	ERROR_STRING_BLOCK_MISSING_NEW_LINE,
	ERROR_STRING_BLOCK_MISSING_TERMINATION,
	ERROR_STRING_BLOCK_MISSING_INDENT,
	#[regex("[_a-zA-Z][_a-zA-Z0-9]*")]
	IDENT,
	#[regex("[ \\t\\n\\r]+")]
	WHITESPACE,
	#[regex("//[^\\r\\n]*(\\r\\n|\\n)?")]
	SINGLE_LINE_SLASH_COMMENT,
	#[regex("#[^\\r\\n]*(\\r\\n|\\n)?")]
	SINGLE_LINE_HASH_COMMENT,
	#[regex("/\\*([^*]|\\*[^/])*\\*/")]
	MULTI_LINE_COMMENT,
	#[regex("/\\*/")]
	ERROR_COMMENT_TOO_SHORT,
	#[regex("/\\*([^*/]|\\*[^/])+")]
	ERROR_COMMENT_UNTERMINATED,
	#[token("tailstrict")]
	TAILSTRICT_KW,
	#[token("local")]
	LOCAL_KW,
	#[token("importstr")]
	IMPORTSTR_KW,
	#[token("importbin")]
	IMPORTBIN_KW,
	#[token("import")]
	IMPORT_KW,
	#[token("if")]
	IF_KW,
	#[token("then")]
	THEN_KW,
	#[token("else")]
	ELSE_KW,
	#[token("function")]
	FUNCTION_KW,
	#[token("error")]
	ERROR_KW,
	#[token("in")]
	IN_KW,
	#[token("null")]
	NULL_KW,
	#[token("true")]
	TRUE_KW,
	#[token("false")]
	FALSE_KW,
	#[token("self")]
	SELF_KW,
	#[token("super")]
	SUPER_KW,
	#[token("for")]
	FOR_KW,
	#[token("assert")]
	ASSERT_KW,
	LEXING_ERROR,
	__LAST_TOKEN,
}
use self::TokenKind::*;
impl TokenKind {
	pub fn is_keyword(self) -> bool {
		match self {
			OR | NULL_COAELSE | AND | BIT_OR | BIT_XOR | BIT_AND | EQ | NE | LT | GT | LE | GE
			| LHS | RHS | PLUS | MINUS | MUL | DIV | MODULO | NOT | BIT_NOT | L_BRACK | R_BRACK
			| L_PAREN | R_PAREN | L_BRACE | R_BRACE | COLON | COLONCOLON | COLONCOLONCOLON
			| SEMI | DOT | DOTDOTDOT | COMMA | DOLLAR | ASSIGN | QUESTION_MARK | TAILSTRICT_KW
			| LOCAL_KW | IMPORTSTR_KW | IMPORTBIN_KW | IMPORT_KW | IF_KW | THEN_KW | ELSE_KW
			| FUNCTION_KW | ERROR_KW | IN_KW | NULL_KW | TRUE_KW | FALSE_KW | SELF_KW
			| SUPER_KW | FOR_KW | ASSERT_KW => true,
			_ => false,
		}
	}
	pub fn from_raw(r: u16) -> Self {
		assert!(r < Self::__LAST_TOKEN as u16);
		unsafe { std::mem::transmute(r) }
	}
	pub fn into_raw(self) -> u16 {
		self as u16
	}
}
#[macro_export]
macro_rules ! T { [||] => { $ crate :: SyntaxKind :: OR } ; [??] => { $ crate :: SyntaxKind :: NULL_COAELSE } ; [&&] => { $ crate :: SyntaxKind :: AND } ; [|] => { $ crate :: SyntaxKind :: BIT_OR } ; [^] => { $ crate :: SyntaxKind :: BIT_XOR } ; [&] => { $ crate :: SyntaxKind :: BIT_AND } ; [==] => { $ crate :: SyntaxKind :: EQ } ; [!=] => { $ crate :: SyntaxKind :: NE } ; [<] => { $ crate :: SyntaxKind :: LT } ; [>] => { $ crate :: SyntaxKind :: GT } ; [<=] => { $ crate :: SyntaxKind :: LE } ; [>=] => { $ crate :: SyntaxKind :: GE } ; [<<] => { $ crate :: SyntaxKind :: LHS } ; [>>] => { $ crate :: SyntaxKind :: RHS } ; [+] => { $ crate :: SyntaxKind :: PLUS } ; [-] => { $ crate :: SyntaxKind :: MINUS } ; [*] => { $ crate :: SyntaxKind :: MUL } ; [/] => { $ crate :: SyntaxKind :: DIV } ; [%] => { $ crate :: SyntaxKind :: MODULO } ; [!] => { $ crate :: SyntaxKind :: NOT } ; [~] => { $ crate :: SyntaxKind :: BIT_NOT } ; ['['] => { $ crate :: SyntaxKind :: L_BRACK } ; [']'] => { $ crate :: SyntaxKind :: R_BRACK } ; ['('] => { $ crate :: SyntaxKind :: L_PAREN } ; [')'] => { $ crate :: SyntaxKind :: R_PAREN } ; ['{'] => { $ crate :: SyntaxKind :: L_BRACE } ; ['}'] => { $ crate :: SyntaxKind :: R_BRACE } ; [:] => { $ crate :: SyntaxKind :: COLON } ; [::] => { $ crate :: SyntaxKind :: COLONCOLON } ; [:::] => { $ crate :: SyntaxKind :: COLONCOLONCOLON } ; [;] => { $ crate :: SyntaxKind :: SEMI } ; [.] => { $ crate :: SyntaxKind :: DOT } ; [...] => { $ crate :: SyntaxKind :: DOTDOTDOT } ; [,] => { $ crate :: SyntaxKind :: COMMA } ; ['$'] => { $ crate :: SyntaxKind :: DOLLAR } ; [=] => { $ crate :: SyntaxKind :: ASSIGN } ; [?] => { $ crate :: SyntaxKind :: QUESTION_MARK } ; [tailstrict] => { $ crate :: SyntaxKind :: TAILSTRICT_KW } ; [local] => { $ crate :: SyntaxKind :: LOCAL_KW } ; [importstr] => { $ crate :: SyntaxKind :: IMPORTSTR_KW } ; [importbin] => { $ crate :: SyntaxKind :: IMPORTBIN_KW } ; [import] => { $ crate :: SyntaxKind :: IMPORT_KW } ; [if] => { $ crate :: SyntaxKind :: IF_KW } ; [then] => { $ crate :: SyntaxKind :: THEN_KW } ; [else] => { $ crate :: SyntaxKind :: ELSE_KW } ; [function] => { $ crate :: SyntaxKind :: FUNCTION_KW } ; [error] => { $ crate :: SyntaxKind :: ERROR_KW } ; [in] => { $ crate :: SyntaxKind :: IN_KW } ; [null] => { $ crate :: SyntaxKind :: NULL_KW } ; [true] => { $ crate :: SyntaxKind :: TRUE_KW } ; [false] => { $ crate :: SyntaxKind :: FALSE_KW } ; [self] => { $ crate :: SyntaxKind :: SELF_KW } ; [super] => { $ crate :: SyntaxKind :: SUPER_KW } ; [for] => { $ crate :: SyntaxKind :: FOR_KW } ; [assert] => { $ crate :: SyntaxKind :: ASSERT_KW } }
#[allow(unused_imports)]
pub use T;
