#![deny(unused_must_use)]

use event::Sink;
use generated::nodes::{SourceFile, Trivia};
use lex::lex;
use parser::{LocatedSyntaxError, Parser};
pub use rowan;

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
pub use generated::{nodes, syntax_kinds::SyntaxKind};
pub use language::*;
pub use string_block::{collect_lexed_str_block, CollectStrBlock};
pub use token_set::SyntaxKindSet;

use self::{
	ast::support,
	generated::nodes::{Expr, ExprBinary, ExprObjExtend},
};

pub fn parse(input: &str) -> (SourceFile, Vec<LocatedSyntaxError>) {
	let lexemes = lex(input);
	let kinds = lexemes
		.iter()
		.map(|l| l.kind)
		.filter(|k| !Trivia::can_cast(*k))
		.collect();
	let parser = Parser::new(kinds);
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
impl ExprObjExtend {
	pub fn lhs_work(&self) -> Option<Expr> {
		support::child(self.syntax())
	}
	pub fn rhs_work(&self) -> Option<Expr> {
		let mut children = support::children(self.syntax());
		// skip lhs
		children.next()?;
		children.next()
	}
}
