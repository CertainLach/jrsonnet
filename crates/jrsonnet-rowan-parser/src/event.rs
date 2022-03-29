use std::mem;

use rowan::{GreenNode, GreenNodeBuilder, Language};

use crate::{
	lex::{Lang, Lexeme, SyntaxKind},
	parser::{Parse, SyntaxError},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
	Start {
		kind: SyntaxKind,
		forward_parent: Option<usize>,
	},
	Token,
	Finish,
	Placeholder,
	Error(SyntaxError),
}

pub(super) struct Sink<'i> {
	pub builder: GreenNodeBuilder<'static>,
	lexemes: &'i [Lexeme<'i>],
	offset: usize,
	events: Vec<Event>,
	pub errors: Vec<SyntaxError>,
}

impl<'i> Sink<'i> {
	pub(super) fn new(events: Vec<Event>, lexemes: &'i [Lexeme<'i>]) -> Self {
		Self {
			builder: GreenNodeBuilder::new(),
			lexemes,
			offset: 0,
			events,
			errors: vec![],
		}
	}

	pub(super) fn finish(mut self) -> Parse {
		for idx in 0..self.events.len() {
			match mem::replace(&mut self.events[idx], Event::Placeholder) {
				Event::Start {
					kind,
					forward_parent,
				} => {
					let mut kinds = vec![kind];

					let mut idx = idx;
					let mut forward_parent = forward_parent;

					// Walk through the forward parent of the forward parent, and the forward parent
					// of that, and of that, etc. until we reach a StartNode event without a forward
					// parent.
					while let Some(fp) = forward_parent {
						idx += fp;

						forward_parent = if let Event::Start {
							kind,
							forward_parent,
						} = mem::replace(&mut self.events[idx], Event::Placeholder)
						{
							kinds.push(kind);
							forward_parent
						} else {
							unreachable!()
						};
					}

					for kind in kinds.into_iter().rev() {
						self.builder.start_node(Lang::kind_to_raw(kind));
					}
				}
				Event::Token => self.token(),
				Event::Finish => {
					self.builder.finish_node();
				}
				Event::Placeholder => {}
				Event::Error(e) => {
					self.errors.push(e);
				}
			}
			self.skip_whitespace();
		}

		Parse {
			green_node: self.builder.finish(),
			errors: self.errors,
		}
	}
	fn token(&mut self) {
		let lexeme = self.lexemes[self.offset];
		self.builder
			.token(Lang::kind_to_raw(lexeme.kind), lexeme.text);
		self.offset += 1;
	}
	fn skip_whitespace(&mut self) {
		while let Some(lexeme) = self.lexemes.get(self.offset) {
			if !lexeme.kind.is_trivia() {
				break;
			}

			self.token();
		}
	}
}
