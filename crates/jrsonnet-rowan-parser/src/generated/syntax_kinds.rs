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
pub enum SyntaxKind {
	#[doc(hidden)]
	TOMBSTONE,
	#[doc(hidden)]
	EOF,
	#[token("||")]
	OR,
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
	#[token("$intrinsicThisFile")]
	INTRINSIC_THIS_FILE,
	#[token("$intrinsicId")]
	INTRINSIC_ID,
	#[token("$intrinsic")]
	INTRINSIC,
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
	#[regex("/\\*([^*]|\\*[^/])+")]
	ERROR_COMMENT_UNTERMINATED,
	#[token("tailstrict")]
	TAILSTRICT_KW,
	#[token("importstr")]
	IMPORTSTR_KW,
	#[token("importbin")]
	IMPORTBIN_KW,
	#[token("import")]
	IMPORT_KW,
	#[token("local")]
	LOCAL_KW,
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
	META_OBJECT_APPLY,
	ERROR_NO_OPERATOR,
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
	ERROR_MISSING_TOKEN,
	ERROR_UNEXPECTED_TOKEN,
	ERROR_CUSTOM,
	#[doc = r" Also acts as __LAST_TOKEN"]
	#[error]
	LEXING_ERROR,
	SOURCE_FILE,
	EXPR_BINARY,
	LHS_EXPR,
	EXPR_UNARY,
	EXPR_SLICE,
	SLICE_DESC,
	EXPR_INDEX,
	NAME,
	EXPR_INDEX_EXPR,
	EXPR_APPLY,
	ARGS_DESC,
	EXPR_OBJ_EXTEND,
	EXPR_PARENED,
	EXPR_LITERAL,
	EXPR_INTRINSIC_THIS_FILE,
	EXPR_INTRINSIC_ID,
	EXPR_INTRINSIC,
	EXPR_STRING,
	EXPR_NUMBER,
	EXPR_ARRAY,
	EXPR_OBJECT,
	EXPR_ARRAY_COMP,
	EXPR_IMPORT,
	EXPR_VAR,
	EXPR_LOCAL,
	EXPR_IF_THEN_ELSE,
	TRUE_EXPR,
	FALSE_EXPR,
	EXPR_FUNCTION,
	PARAMS_DESC,
	EXPR_ASSERT,
	ASSERTION,
	EXPR_ERROR,
	SLICE_DESC_END,
	SLICE_DESC_STEP,
	ARG,
	OBJ_BODY_COMP,
	OBJ_LOCAL_POST_COMMA,
	OBJ_LOCAL_PRE_COMMA,
	OBJ_BODY_MEMBER_LIST,
	OBJ_LOCAL,
	MEMBER_BIND_STMT,
	MEMBER_ASSERT_STMT,
	MEMBER_FIELD_NORMAL,
	MEMBER_FIELD_METHOD,
	FIELD_NAME_FIXED,
	FIELD_NAME_DYNAMIC,
	FOR_SPEC,
	IF_SPEC,
	BIND_DESTRUCT,
	BIND_FUNCTION,
	PARAM,
	DESTRUCT_FULL,
	DESTRUCT_SKIP,
	DESTRUCT_ARRAY,
	DESTRUCT_OBJECT,
	DESTRUCT_OBJECT_FIELD,
	DESTRUCT_REST,
	DESTRUCT_ARRAY_ELEMENT,
	EXPR,
	OBJ_BODY,
	COMP_SPEC,
	BIND,
	MEMBER,
	FIELD_NAME,
	DESTRUCT,
	DESTRUCT_ARRAY_PART,
	BINARY_OPERATOR,
	UNARY_OPERATOR,
	LITERAL,
	TEXT,
	NUMBER,
	IMPORT_KIND,
	VISIBILITY,
	TRIVIA,
	PARSING_ERROR,
	#[doc(hidden)]
	__LAST,
}
use self::SyntaxKind::*;
impl SyntaxKind {
	pub fn is_keyword(self) -> bool {
		match self {
			OR | AND | BIT_OR | BIT_XOR | BIT_AND | EQ | NE | LT | GT | LE | GE | LHS | RHS
			| PLUS | MINUS | MUL | DIV | MODULO | NOT | BIT_NOT | L_BRACK | R_BRACK | L_PAREN
			| R_PAREN | L_BRACE | R_BRACE | COLON | COLONCOLON | COLONCOLONCOLON | SEMI | DOT
			| DOTDOTDOT | COMMA | DOLLAR | ASSIGN | QUESTION_MARK | INTRINSIC_THIS_FILE
			| INTRINSIC_ID | INTRINSIC | TAILSTRICT_KW | IMPORTSTR_KW | IMPORTBIN_KW
			| IMPORT_KW | LOCAL_KW | IF_KW | THEN_KW | ELSE_KW | FUNCTION_KW | ERROR_KW | IN_KW
			| NULL_KW | TRUE_KW | FALSE_KW | SELF_KW | SUPER_KW | FOR_KW | ASSERT_KW => true,
			_ => false,
		}
	}
	pub fn is_enum(self) -> bool {
		match self {
			EXPR | OBJ_BODY | COMP_SPEC | BIND | MEMBER | FIELD_NAME | DESTRUCT
			| DESTRUCT_ARRAY_PART | BINARY_OPERATOR | UNARY_OPERATOR | LITERAL | TEXT | NUMBER
			| IMPORT_KIND | VISIBILITY | TRIVIA | PARSING_ERROR => true,
			_ => false,
		}
	}
	pub fn from_raw(r: u16) -> Self {
		assert!(r < Self::__LAST as u16);
		unsafe { std::mem::transmute(r) }
	}
	pub fn into_raw(self) -> u16 {
		self as u16
	}
}
#[macro_export]
macro_rules ! T { [||] => { $ crate :: SyntaxKind :: OR } ; [&&] => { $ crate :: SyntaxKind :: AND } ; [|] => { $ crate :: SyntaxKind :: BIT_OR } ; [^] => { $ crate :: SyntaxKind :: BIT_XOR } ; [&] => { $ crate :: SyntaxKind :: BIT_AND } ; [==] => { $ crate :: SyntaxKind :: EQ } ; [!=] => { $ crate :: SyntaxKind :: NE } ; [<] => { $ crate :: SyntaxKind :: LT } ; [>] => { $ crate :: SyntaxKind :: GT } ; [<=] => { $ crate :: SyntaxKind :: LE } ; [>=] => { $ crate :: SyntaxKind :: GE } ; [<<] => { $ crate :: SyntaxKind :: LHS } ; [>>] => { $ crate :: SyntaxKind :: RHS } ; [+] => { $ crate :: SyntaxKind :: PLUS } ; [-] => { $ crate :: SyntaxKind :: MINUS } ; [*] => { $ crate :: SyntaxKind :: MUL } ; [/] => { $ crate :: SyntaxKind :: DIV } ; [%] => { $ crate :: SyntaxKind :: MODULO } ; [!] => { $ crate :: SyntaxKind :: NOT } ; [~] => { $ crate :: SyntaxKind :: BIT_NOT } ; ['['] => { $ crate :: SyntaxKind :: L_BRACK } ; [']'] => { $ crate :: SyntaxKind :: R_BRACK } ; ['('] => { $ crate :: SyntaxKind :: L_PAREN } ; [')'] => { $ crate :: SyntaxKind :: R_PAREN } ; ['{'] => { $ crate :: SyntaxKind :: L_BRACE } ; ['}'] => { $ crate :: SyntaxKind :: R_BRACE } ; [:] => { $ crate :: SyntaxKind :: COLON } ; [::] => { $ crate :: SyntaxKind :: COLONCOLON } ; [:::] => { $ crate :: SyntaxKind :: COLONCOLONCOLON } ; [;] => { $ crate :: SyntaxKind :: SEMI } ; [.] => { $ crate :: SyntaxKind :: DOT } ; [...] => { $ crate :: SyntaxKind :: DOTDOTDOT } ; [,] => { $ crate :: SyntaxKind :: COMMA } ; ['$'] => { $ crate :: SyntaxKind :: DOLLAR } ; [=] => { $ crate :: SyntaxKind :: ASSIGN } ; [?] => { $ crate :: SyntaxKind :: QUESTION_MARK } ; ["$intrinsicThisFile"] => { $ crate :: SyntaxKind :: INTRINSIC_THIS_FILE } ; ["$intrinsicId"] => { $ crate :: SyntaxKind :: INTRINSIC_ID } ; ["$intrinsic"] => { $ crate :: SyntaxKind :: INTRINSIC } ; [tailstrict] => { $ crate :: SyntaxKind :: TAILSTRICT_KW } ; [importstr] => { $ crate :: SyntaxKind :: IMPORTSTR_KW } ; [importbin] => { $ crate :: SyntaxKind :: IMPORTBIN_KW } ; [import] => { $ crate :: SyntaxKind :: IMPORT_KW } ; [local] => { $ crate :: SyntaxKind :: LOCAL_KW } ; [if] => { $ crate :: SyntaxKind :: IF_KW } ; [then] => { $ crate :: SyntaxKind :: THEN_KW } ; [else] => { $ crate :: SyntaxKind :: ELSE_KW } ; [function] => { $ crate :: SyntaxKind :: FUNCTION_KW } ; [error] => { $ crate :: SyntaxKind :: ERROR_KW } ; [in] => { $ crate :: SyntaxKind :: IN_KW } ; [null] => { $ crate :: SyntaxKind :: NULL_KW } ; [true] => { $ crate :: SyntaxKind :: TRUE_KW } ; [false] => { $ crate :: SyntaxKind :: FALSE_KW } ; [self] => { $ crate :: SyntaxKind :: SELF_KW } ; [super] => { $ crate :: SyntaxKind :: SUPER_KW } ; [for] => { $ crate :: SyntaxKind :: FOR_KW } ; [assert] => { $ crate :: SyntaxKind :: ASSERT_KW } }
pub use T;
