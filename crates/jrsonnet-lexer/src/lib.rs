mod generated;
mod lex;
mod string_block;

#[derive(Clone, Copy, Debug)]
pub struct Span(pub u32, pub u32);

pub use lex::{Lexeme, Lexer};
