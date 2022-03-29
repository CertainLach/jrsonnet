use drop_bomb::DropBomb;
use rowan::TextRange;

use crate::{event::Event, lex::SyntaxKind, parser::Parser};

pub struct Ranger {
	pub pos: usize,
}
impl Ranger {
	pub fn finish(mut self, p: &Parser) -> FinishedRanger {
		FinishedRanger {
			start_token: self.pos,
			end_token: self.pos.max(p.offset.saturating_sub(1)),
		}
	}
}

pub struct FinishedRanger {
	pub start_token: usize,
	pub end_token: usize,
}
impl FinishedRanger {
	pub fn had_error_since(&self, p: &Parser) -> bool {
		p.last_error_token >= self.start_token
	}
}

#[must_use]
pub struct Marker {
	pub start_event_idx: usize,
	pub token: usize,
	bomb: DropBomb,
}
impl Marker {
	pub fn new(pos: usize, token: usize) -> Self {
		Self {
			start_event_idx: pos,
			token,
			bomb: DropBomb::new("marked dropped while not completed"),
		}
	}
	pub fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
		self.bomb.defuse();
		let event_at_pos = &mut p.events[self.start_event_idx];
		assert_eq!(*event_at_pos, Event::Placeholder);

		*event_at_pos = Event::Start {
			kind,
			forward_parent: None,
		};

		p.events.push(Event::Finish);
		p.entered -= 1;
		p.clear_outdated_hints();
		CompletedMarker {
			start_event_idx: self.start_event_idx,
			start_token: self.token,
			end_token: self.token.max(p.offset.saturating_sub(1)),
		}
	}
}
pub struct CompletedMarker {
	start_event_idx: usize,
	pub start_token: usize,
	pub end_token: usize,
}
impl CompletedMarker {
	pub(super) fn precede(self, p: &mut Parser) -> Marker {
		let mut new_m = p.start();
		new_m.token = self.start_token;

		if let Event::Start {
			ref mut forward_parent,
			..
		} = p.events[self.start_event_idx]
		{
			*forward_parent = Some(new_m.start_event_idx - self.start_event_idx);
		} else {
			unreachable!();
		}

		new_m
	}
}

pub trait AsRange {
	fn as_range(&self, p: &Parser) -> TextRange;
	fn end_token(&self) -> usize;
}

impl AsRange for CompletedMarker {
	fn as_range(&self, p: &Parser) -> TextRange {
		TextRange::new(
			p.start_of_token(self.start_token),
			p.end_of_token(self.end_token),
		)
	}
	fn end_token(&self) -> usize {
		self.end_token
	}
}

impl AsRange for FinishedRanger {
	fn as_range(&self, p: &Parser) -> TextRange {
		TextRange::new(
			p.start_of_token(self.start_token),
			p.end_of_token(self.end_token),
		)
	}

	fn end_token(&self) -> usize {
		self.end_token
	}
}
