use std::num::NonZeroUsize;

use drop_bomb::DropBomb;

use crate::{
	event::Event,
	parser::{ExpectedSyntax, Parser, SyntaxError},
	SyntaxKind,
};

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
	fn complete_raw(
		mut self,
		p: &mut Parser,
		kind: SyntaxKind,
		error: Option<SyntaxError>,
	) -> CompletedMarker {
		self.bomb.defuse();
		assert!(
			!kind.is_enum(),
			"{kind:?} is a enum kind, you should use variant kinds instead"
		);
		// TODO: is_lexer should return true if enum variant has #[regex]/#[token] over it, or it is defined as lexer error explicitly
		// debug_assert!(
		// 	!kind.is_lexer(),
		// 	"{kind:?} should be only emitted by lexer, not used directly"
		// );
		let event_at_pos = &mut p.events[self.start_event_idx];
		assert!(matches!(event_at_pos, Event::Pending));

		*event_at_pos = Event::Start {
			kind,
			forward_parent: None,
		};

		let finish_event_idx = p.events.len();
		p.events.push(Event::Finish {
			wrapper: None,
			error: error.map(Box::new),
		});
		p.entered -= 1;
		p.clear_outdated_hints();
		CompletedMarker {
			start_event_idx: self.start_event_idx,
			finish_event_idx,
		}
	}
	pub fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
		self.complete_raw(p, kind, None)
	}
	pub fn complete_error(mut self, p: &mut Parser, msg: impl AsRef<str>) -> CompletedMarker {
		self.complete_raw(
			p,
			SyntaxKind::ERROR_CUSTOM,
			Some(SyntaxError::Custom {
				error: msg.as_ref().to_owned(),
			}),
		)
	}
	pub fn complete_missing(mut self, p: &mut Parser, expected: ExpectedSyntax) -> CompletedMarker {
		self.complete_raw(
			p,
			SyntaxKind::ERROR_MISSING_TOKEN,
			Some(SyntaxError::Missing { expected }),
		)
	}
	pub fn complete_unexpected(
		mut self,
		p: &mut Parser,
		expected: ExpectedSyntax,
		found: SyntaxKind,
	) -> CompletedMarker {
		self.complete_raw(
			p,
			SyntaxKind::ERROR_UNEXPECTED_TOKEN,
			Some(SyntaxError::Unexpected { expected, found }),
		)
	}

	pub fn forget(mut self, p: &mut Parser) {
		self.bomb.defuse();
		let event_at_pos = &mut p.events[self.start_event_idx];
		assert!(matches!(event_at_pos, Event::Pending));

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
				*forward_parent = Some(
					NonZeroUsize::new(new_m.start_event_idx - self.start_event_idx).expect("!= 0"),
				);
			}
			_ => unreachable!(),
		}

		new_m
	}
	/// Create new node around existing marker, not counting anything that comes after it
	fn wrap_raw(
		self,
		p: &mut Parser,
		kind: SyntaxKind,
		error: Option<SyntaxError>,
	) -> CompletedMarker {
		let new_m = p.start();
		match &mut p.events[self.start_event_idx] {
			Event::Start { forward_parent, .. } => {
				*forward_parent = Some(
					NonZeroUsize::new(new_m.start_event_idx - self.start_event_idx).expect("!= 0"),
				);
			}
			_ => unreachable!(),
		}

		let completed = new_m.complete_raw(p, kind, error);

		match &mut p.events[self.finish_event_idx] {
			Event::Finish {
				wrapper,
				error: _error,
			} => {
				*wrapper = Some(
					NonZeroUsize::new(completed.finish_event_idx - self.finish_event_idx)
						.expect("!= 0"),
				);
			}
			_ => unreachable!(),
		}
		completed
	}
	pub fn wrap(self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
		self.wrap_raw(p, kind, None)
	}
	pub fn wrap_error(self, p: &mut Parser, msg: impl AsRef<str>) -> CompletedMarker {
		self.wrap_raw(
			p,
			SyntaxKind::ERROR_CUSTOM,
			Some(SyntaxError::Custom {
				error: msg.as_ref().to_owned(),
			}),
		)
	}
}
