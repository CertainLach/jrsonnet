#![deny(unused_must_use)]

mod ast;
mod event;
mod generated;
mod language;
mod lex;
mod marker;
mod parser;
mod precedence;
mod string_block;
mod tests;
mod token_set;

pub use ast::{AstChildren, AstNode, AstToken};
use event::Sink;
use generated::nodes::SourceFile;
pub use generated::{nodes, syntax_kinds::SyntaxKind};
pub use language::{
	JsonnetLanguage, PreorderWithTokens, SyntaxElement, SyntaxElementChildren, SyntaxNode,
	SyntaxNodeChildren, SyntaxToken,
};
use lex::lex;
use parser::{Parser, SyntaxError};
pub fn parse(input: &str) -> (SourceFile, Vec<SyntaxError>) {
	let lexemes = lex(input);
	let parser = Parser::new(&lexemes);
	let events = parser.parse();
	let sink = Sink::new(events, &lexemes);

	let parse = sink.finish();
	(
		SourceFile {
			syntax: parse.syntax(),
		},
		parse.errors,
	)
}
