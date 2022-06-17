//! This is a generated file, please do not edit manually. Changes can be
//! made in codegeneration that lives in `xtask` top-level dir.

#![allow(bad_style, missing_docs, unreachable_pub)]
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
	#[regex("(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?")]
	NUMBER,
	#[regex("\"(?s:[^\"\\\\]|\\\\.)*\"")]
	STRING_DOUBLE,
	#[regex("'(?s:[^'\\\\]|\\\\.)*'")]
	STRING_SINGLE,
	#[regex("@\"(?:[^\"]|\"\")*\"")]
	STRING_DOUBLE_VERBATIM,
	#[regex("@'(?:[^']|'')*'")]
	STRING_SINGLE_VERBATIM,
	#[regex("\\|\\|\\|")]
	STRING_BLOCK,
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
	#[error]
	ERROR,
	SOURCE_FILE,
	EXPR_BINARY,
	BINARY_OPERATOR,
	EXPR_UNARY,
	UNARY_OPERATOR,
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
	LITERAL,
	EXPR_INTRINSIC_THIS_FILE,
	EXPR_INTRINSIC_ID,
	EXPR_INTRINSIC,
	EXPR_STRING,
	STRING,
	EXPR_NUMBER,
	EXPR_ARRAY,
	EXPR_OBJECT,
	EXPR_ARRAY_COMP,
	FOR_SPEC,
	EXPR_IMPORT,
	EXPR_VAR,
	EXPR_LOCAL,
	EXPR_IF_THEN_ELSE,
	EXPR_FUNCTION,
	PARAMS_DESC,
	EXPR_ASSERT,
	ASSERTION,
	EXPR_ERROR,
	ARG,
	OBJ_BODY_COMP,
	OBJ_LOCAL_POST_COMMA,
	OBJ_LOCAL_PRE_COMMA,
	OBJ_BODY_MEMBER_LIST,
	OBJ_LOCAL,
	MEMBER_BIND_STMT,
	MEMBER_ASSERT_STMT,
	MEMBER_FIELD,
	FIELD_NORMAL,
	VISIBILITY,
	FIELD_METHOD,
	FIELD_NAME_FIXED,
	FIELD_NAME_DYNAMIC,
	IF_SPEC,
	BIND_DESTRUCT,
	DESTRUCT,
	BIND_FUNCTION,
	PARAM,
	DESTRUCT_FULL,
	DESTRUCT_SKIP,
	DESTRUCT_ARRAY,
	DESTRUCT_REST,
	DESTRUCT_OBJECT,
	DESTRUCT_OBJECT_FIELD,
	EXPR,
	OBJ_BODY,
	COMP_SPEC,
	BIND,
	MEMBER,
	FIELD,
	FIELD_NAME,
	#[doc(hidden)]
	__LAST,
}
use self::SyntaxKind::*;
impl SyntaxKind {
	pub fn is_keyword(self) -> bool {
		match self {
			TAILSTRICT_KW | IMPORTSTR_KW | IMPORTBIN_KW | IMPORT_KW | LOCAL_KW | IF_KW
			| THEN_KW | ELSE_KW | FUNCTION_KW | ERROR_KW | IN_KW | NULL_KW | TRUE_KW | FALSE_KW
			| SELF_KW | SUPER_KW | FOR_KW | ASSERT_KW => true,
			_ => false,
		}
	}
	pub fn is_punct(self) -> bool {
		match self {
			OR | AND | BIT_OR | BIT_XOR | BIT_AND | EQ | NE | LT | GT | LE | GE | LHS | RHS
			| PLUS | MINUS | MUL | DIV | MODULO | NOT | BIT_NOT | L_BRACK | R_BRACK | L_PAREN
			| R_PAREN | L_BRACE | R_BRACE | COLON | COLONCOLON | COLONCOLONCOLON | SEMI | DOT
			| DOTDOTDOT | COMMA | DOLLAR | ASSIGN | QUESTION_MARK | INTRINSIC_THIS_FILE
			| INTRINSIC_ID | INTRINSIC => true,
			_ => false,
		}
	}
	pub fn from_keyword(ident: &str) -> Option<SyntaxKind> {
		let kw = match ident {
			"tailstrict" => TAILSTRICT_KW,
			"importstr" => IMPORTSTR_KW,
			"importbin" => IMPORTBIN_KW,
			"import" => IMPORT_KW,
			"local" => LOCAL_KW,
			"if" => IF_KW,
			"then" => THEN_KW,
			"else" => ELSE_KW,
			"function" => FUNCTION_KW,
			"error" => ERROR_KW,
			"in" => IN_KW,
			"null" => NULL_KW,
			"true" => TRUE_KW,
			"false" => FALSE_KW,
			"self" => SELF_KW,
			"super" => SUPER_KW,
			"for" => FOR_KW,
			"assert" => ASSERT_KW,
			_ => return None,
		};
		Some(kw)
	}
	pub fn from_char(c: char) -> Option<SyntaxKind> {
		let tok = match c {
			'|' => BIT_OR,
			'^' => BIT_XOR,
			'&' => BIT_AND,
			'<' => LT,
			'>' => GT,
			'+' => PLUS,
			'-' => MINUS,
			'*' => MUL,
			'/' => DIV,
			'%' => MODULO,
			'!' => NOT,
			'~' => BIT_NOT,
			'[' => L_BRACK,
			']' => R_BRACK,
			'(' => L_PAREN,
			')' => R_PAREN,
			'{' => L_BRACE,
			'}' => R_BRACE,
			':' => COLON,
			';' => SEMI,
			'.' => DOT,
			',' => COMMA,
			'$' => DOLLAR,
			'=' => ASSIGN,
			'?' => QUESTION_MARK,
			_ => return None,
		};
		Some(tok)
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
macro_rules ! T { [||] => { $ crate :: SyntaxKind :: OR } ; [&&] => { $ crate :: SyntaxKind :: AND } ; [|] => { $ crate :: SyntaxKind :: BIT_OR } ; [^] => { $ crate :: SyntaxKind :: BIT_XOR } ; [&] => { $ crate :: SyntaxKind :: BIT_AND } ; [==] => { $ crate :: SyntaxKind :: EQ } ; [!=] => { $ crate :: SyntaxKind :: NE } ; [<] => { $ crate :: SyntaxKind :: LT } ; [>] => { $ crate :: SyntaxKind :: GT } ; [<=] => { $ crate :: SyntaxKind :: LE } ; [>=] => { $ crate :: SyntaxKind :: GE } ; [<<] => { $ crate :: SyntaxKind :: LHS } ; [>>] => { $ crate :: SyntaxKind :: RHS } ; [+] => { $ crate :: SyntaxKind :: PLUS } ; [-] => { $ crate :: SyntaxKind :: MINUS } ; [*] => { $ crate :: SyntaxKind :: MUL } ; [/] => { $ crate :: SyntaxKind :: DIV } ; [%] => { $ crate :: SyntaxKind :: MODULO } ; [!] => { $ crate :: SyntaxKind :: NOT } ; [~] => { $ crate :: SyntaxKind :: BIT_NOT } ; ['['] => { $ crate :: SyntaxKind :: L_BRACK } ; [']'] => { $ crate :: SyntaxKind :: R_BRACK } ; ['('] => { $ crate :: SyntaxKind :: L_PAREN } ; [')'] => { $ crate :: SyntaxKind :: R_PAREN } ; ['{'] => { $ crate :: SyntaxKind :: L_BRACE } ; ['}'] => { $ crate :: SyntaxKind :: R_BRACE } ; [:] => { $ crate :: SyntaxKind :: COLON } ; [::] => { $ crate :: SyntaxKind :: COLONCOLON } ; [:::] => { $ crate :: SyntaxKind :: COLONCOLONCOLON } ; [;] => { $ crate :: SyntaxKind :: SEMI } ; [.] => { $ crate :: SyntaxKind :: DOT } ; [...] => { $ crate :: SyntaxKind :: DOTDOTDOT } ; [,] => { $ crate :: SyntaxKind :: COMMA } ; ['$'] => { $ crate :: SyntaxKind :: DOLLAR } ; [=] => { $ crate :: SyntaxKind :: ASSIGN } ; [?] => { $ crate :: SyntaxKind :: QUESTION_MARK } ; ["$intrinsicThisFile"] => { $ crate :: SyntaxKind :: INTRINSIC_THIS_FILE } ; ["$intrinsicId"] => { $ crate :: SyntaxKind :: INTRINSIC_ID } ; ["$intrinsic"] => { $ crate :: SyntaxKind :: INTRINSIC } ; [tailstrict] => { $ crate :: SyntaxKind :: TAILSTRICT_KW } ; [importstr] => { $ crate :: SyntaxKind :: IMPORTSTR_KW } ; [importbin] => { $ crate :: SyntaxKind :: IMPORTBIN_KW } ; [import] => { $ crate :: SyntaxKind :: IMPORT_KW } ; [local] => { $ crate :: SyntaxKind :: LOCAL_KW } ; [if] => { $ crate :: SyntaxKind :: IF_KW } ; [then] => { $ crate :: SyntaxKind :: THEN_KW } ; [else] => { $ crate :: SyntaxKind :: ELSE_KW } ; [function] => { $ crate :: SyntaxKind :: FUNCTION_KW } ; [error] => { $ crate :: SyntaxKind :: ERROR_KW } ; [in] => { $ crate :: SyntaxKind :: IN_KW } ; [null] => { $ crate :: SyntaxKind :: NULL_KW } ; [true] => { $ crate :: SyntaxKind :: TRUE_KW } ; [false] => { $ crate :: SyntaxKind :: FALSE_KW } ; [self] => { $ crate :: SyntaxKind :: SELF_KW } ; [super] => { $ crate :: SyntaxKind :: SUPER_KW } ; [for] => { $ crate :: SyntaxKind :: FOR_KW } ; [assert] => { $ crate :: SyntaxKind :: ASSERT_KW } ; [lifetime_ident] => { $ crate :: SyntaxKind :: LIFETIME_IDENT } ; [ident] => { $ crate :: SyntaxKind :: IDENT } ; [shebang] => { $ crate :: SyntaxKind :: SHEBANG } ; }
pub use T;
