use std::mem;

use rowan::{GreenNodeBuilder, Language, TextRange, TextSize};

use crate::{
	lex::Lexeme,
	nodes::Trivia,
	parser::{LocatedSyntaxError, Parse, SyntaxError},
	AstToken, JsonnetLanguage, SyntaxKind,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
	/// Used for unfinished markers
	Pending,
	/// After marker is completed, Pending event is replaced with Start
	Start {
		kind: SyntaxKind,
		/// If marker is preceded or wrapped - instead of reordering events, we
		/// insert start event in the end of events Vec instead, and store relative offset to this event here
		forward_parent: Option<usize>,
	},
	/// Eat token
	Token {
		kind: SyntaxKind,
	},
	/// Push token, but do not eat anything,
	VirtualToken {
		kind: SyntaxKind,
	},
	/// Position of finished node
	Finish {
		/// Same as forward_parent of Start, but for wrapping
		wrapper: Option<usize>,
	},
	Error(SyntaxError),
	/// Used for dropped markers and other things
	Noop,
}

pub(super) struct Sink<'i> {
	pub builder: GreenNodeBuilder<'static>,
	lexemes: &'i [Lexeme<'i>],
	offset: usize,
	events: Vec<Event>,
	pub errors: Vec<LocatedSyntaxError>,
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

	fn text_offset(&self) -> TextSize {
		if self.offset == 0 {
			return 0.into();
		};
		if let Some(lex) = self.lexemes.get(self.offset) {
			lex.range.start()
		} else if let Some(lex) = self.lexemes.get(self.offset - 1) {
			lex.range.end()
		} else {
			panic!("hard oob")
		}
	}

	pub(super) fn finish(mut self) -> Parse {
		let mut eat_start_whitespace = false;
		let mut depth = 0;
		let mut error_starts_at = Vec::new();
		let mut error_last_range = None;
		for idx in 0..self.events.len() {
			match mem::replace(&mut self.events[idx], Event::Noop) {
				Event::Start {
					kind,
					forward_parent,
				} => {
					if depth != 0 {
						self.skip_whitespace();
					}
					error_last_range = None;
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
						} = mem::replace(&mut self.events[idx], Event::Noop)
						{
							kinds.push(kind);
							forward_parent
						} else {
							unreachable!()
						};
					}

					for kind in kinds.into_iter().rev() {
						self.builder.start_node(JsonnetLanguage::kind_to_raw(kind));
						depth += 1;
						if depth == 1 {
							self.skip_whitespace();
						}
						error_starts_at.push(self.text_offset());
					}

					eat_start_whitespace = false;
				}
				Event::Token { kind } => {
					if eat_start_whitespace {
						self.skip_whitespace();
					}
					error_last_range = None;
					self.token(kind);
					eat_start_whitespace = true;
				}
				Event::VirtualToken { kind } => {
					if eat_start_whitespace {
						self.skip_whitespace();
					}
					error_last_range = None;
					self.virtual_token(kind);
					eat_start_whitespace = false;
				}
				Event::Finish { wrapper } => {
					if depth == 1 {
						self.skip_whitespace();
					}
					error_last_range = Some((
						error_starts_at.pop().expect("starts == finishes"),
						self.text_offset(),
					));
					self.builder.finish_node();
					depth -= 1;
					let mut idx = idx;
					let mut wrapper = wrapper;
					while let Some(w) = wrapper {
						idx += w;
						wrapper = if let Event::Finish { wrapper } =
							mem::replace(&mut self.events[idx], Event::Noop)
						{
							error_last_range = Some((
								error_starts_at.pop().expect("starts == finishes"),
								self.text_offset(),
							));
							if depth == 1 {
								self.skip_whitespace();
							}
							self.builder.finish_node();
							depth -= 1;
							wrapper
						} else {
							unreachable!()
						}
					}
					eat_start_whitespace = true;
				}
				Event::Pending => panic!("pending event should not appear in finished events"),
				Event::Noop => {}
				Event::Error(error) => {
					let (start, end) = error_last_range
						.take()
						.expect("expected error event right after closed node");
					self.errors.push(LocatedSyntaxError {
						error,
						range: TextRange::new(start, end),
					});
				}
			}
		}

		Parse {
			green_node: self.builder.finish(),
			errors: self.errors,
		}
	}
	fn virtual_token(&mut self, kind: SyntaxKind) {
		self.builder.token(JsonnetLanguage::kind_to_raw(kind), "")
	}
	fn token(&mut self, kind: SyntaxKind) {
		let lexeme = self.lexemes[self.offset];
		self.builder
			.token(JsonnetLanguage::kind_to_raw(kind), lexeme.text);
		self.offset += 1;
	}
	fn skip_whitespace(&mut self) {
		while let Some(lexeme) = self.lexemes.get(self.offset) {
			if !Trivia::can_cast(lexeme.kind) {
				break;
			}

			self.token(lexeme.kind);
		}
	}
}
