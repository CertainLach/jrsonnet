mod generated;
mod lex;
mod string_block;

#[derive(Clone, Copy, Debug)]
pub struct Span(pub u32, pub u32);

pub use generated::syntax_kinds::SyntaxKind;
pub use lex::{Lexeme, Lexer};
pub use string_block::{collect_lexed_str_block, CollectStrBlock};
