use drop_bomb::DropBomb;
use rowan::TextRange;

use crate::{event::Event, parser::Parser, SyntaxKind};

pub struct Ranger {
	pub pos: usize,
}
impl Ranger {
	pub fn finish(self, p: &Parser) -> FinishedRanger {
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
	#[allow(dead_code)]
	pub fn had_error_since(&self, p: &Parser) -> bool {
		p.last_error_token >= self.start_token
	}
}

#[must_use]
pub struct Marker {
	pub start_event_idx: usize,
	bomb: DropBomb,
}
impl Marker {
	pub fn new(pos: usize) -> Self {
		Self {
			start_event_idx: pos,
			bomb: DropBomb::new("marked dropped while not completed"),
		}
	}
	pub fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
		self.bomb.defuse();
		assert!(
			!kind.is_enum(),
			"{kind:?} is a enum kind, you should use variant kinds instead"
		);
		// TODO: is_parser should return true if enum variant has #[regex]/#[token] over it
		// debug_assert!(
		// 	!kind.is_parser(),
		// 	"{kind:?} should be only emitted by parser, not used directly"
		// );
		let event_at_pos = &mut p.events[self.start_event_idx];
		assert_eq!(*event_at_pos, Event::Pending);

		*event_at_pos = Event::Start {
			kind,
			forward_parent: None,
		};

		let finish_event_idx = p.events.len();
		p.events.push(Event::Finish { wrapper: None });
		p.entered -= 1;
		p.clear_outdated_hints();
		CompletedMarker {
			start_event_idx: self.start_event_idx,
			finish_event_idx,
		}
	}
	pub fn forget(mut self, p: &mut Parser) {
		self.bomb.defuse();
		let event_at_pos = &mut p.events[self.start_event_idx];
		assert_eq!(*event_at_pos, Event::Pending);

		*event_at_pos = Event::Noop;
		p.entered -= 1;
		p.clear_outdated_hints();
	}
}
pub struct CompletedMarker {
	start_event_idx: usize,
	finish_event_idx: usize,
}
impl CompletedMarker {
	pub(super) fn precede(self, p: &mut Parser) -> Marker {
		let new_m = p.start();
		match &mut p.events[self.start_event_idx] {
			Event::Start { forward_parent, .. } => {
				*forward_parent = Some(new_m.start_event_idx - self.start_event_idx);
			}
			_ => unreachable!(),
		}

		new_m
	}
	/// Create new node around existing marker, not counting anything that comes after it
	pub fn wrap(self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
		let new_m = p.start();
		match &mut p.events[self.start_event_idx] {
			Event::Start { forward_parent, .. } => {
				*forward_parent = Some(new_m.start_event_idx - self.start_event_idx);
			}
			_ => unreachable!(),
		}

		let completed = new_m.complete(p, kind);

		match &mut p.events[self.finish_event_idx] {
			Event::Finish { wrapper } => {
				*wrapper = Some(completed.finish_event_idx - self.finish_event_idx);
			}
			_ => unreachable!(),
		}
		completed
	}
}

pub trait AsRange {
	fn as_range(&self, p: &Parser) -> TextRange;
	fn end_token(&self) -> usize;
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
