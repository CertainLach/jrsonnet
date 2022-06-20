use std::{cell::Cell, fmt::Display, rc::Rc};

use miette::{LabeledSpan, SourceOffset, SourceSpan};
use rowan::{GreenNode, TextRange, TextSize};

use crate::{
	event::Event,
	lex::Lexeme,
	marker::{AsRange, CompletedMarker, Marker, Ranger},
	nodes::{BinaryOperatorKind, Literal, Number, Text, Trivia, UnaryOperatorKind},
	token_set::SyntaxKindSet,
	AstToken, SyntaxKind,
	SyntaxKind::*,
	SyntaxNode, T, TS,
};

pub struct Parse {
	pub green_node: GreenNode,
	pub errors: Vec<SyntaxError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExpectedSyntax {
	Named(&'static str),
	Unnamed(SyntaxKind),
}
impl Display for ExpectedSyntax {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ExpectedSyntax::Named(n) => write!(f, "{}", n),
			ExpectedSyntax::Unnamed(u) => write!(f, "{:?}", u),
		}
	}
}

pub struct Parser<'i> {
	// TODO: remove all trivia before feeding to parser?
	lexemes: &'i [Lexeme<'i>],
	pub offset: usize,
	pub events: Vec<Event>,
	pub entered: u32,
	pub hints: Vec<(u32, TextRange, String)>,
	pub last_error_token: usize,
	expected_syntax: Option<ExpectedSyntax>,
	expected_syntax_tracking_state: Rc<Cell<ExpectedSyntaxTrackingState>>,
	steps: Cell<u64>,
}

const DEFAULT_RECOVERY_SET: SyntaxKindSet = TS![];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyntaxError {
	Unexpected {
		expected: ExpectedSyntax,
		found: SyntaxKind,
		range: TextRange,
	},
	Missing {
		expected: ExpectedSyntax,
		offset: TextSize,
	},
	Custom {
		error: String,
		range: TextRange,
	},
	Hint {
		error: String,
		range: TextRange,
	},
}

impl From<SyntaxError> for LabeledSpan {
	fn from(val: SyntaxError) -> Self {
		match val {
			SyntaxError::Unexpected {
				expected,
				found,
				range,
			} => LabeledSpan::new_with_span(
				Some(format!("expected {}, found {:?}", expected, found)),
				SourceSpan::new(
					SourceOffset::from(usize::from(range.start())),
					SourceOffset::from(usize::from(range.end() - range.start())),
				),
			),
			SyntaxError::Missing { expected, offset } => LabeledSpan::new_with_span(
				Some(format!("missing {}", expected)),
				SourceSpan::new(
					SourceOffset::from(usize::from(offset)),
					SourceOffset::from(0),
				),
			),
			SyntaxError::Custom { error, range } | SyntaxError::Hint { error, range } => {
				LabeledSpan::new_with_span(
					Some(error),
					SourceSpan::new(
						SourceOffset::from(usize::from(range.start())),
						SourceOffset::from(usize::from(range.end() - range.start())),
					),
				)
			}
		}
	}
}

impl<'i> Parser<'i> {
	pub fn new(lexemes: &'i [Lexeme<'i>]) -> Self {
		Self {
			lexemes,
			offset: 0,
			events: vec![],
			entered: 0,
			last_error_token: 0,
			hints: vec![],
			expected_syntax: None,
			expected_syntax_tracking_state: Rc::new(Cell::new(
				ExpectedSyntaxTrackingState::Unnamed,
			)),
			steps: Cell::new(0),
		}
	}
	pub fn clear_outdated_hints(&mut self) {
		let amount = self
			.hints
			.iter()
			.rev()
			.take_while(|h| h.0 > self.entered)
			.count();
		self.hints.truncate(self.hints.len() - amount)
	}
	fn clear_expected_syntaxes(&mut self) {
		self.expected_syntax = None;
		self.expected_syntax_tracking_state
			.set(ExpectedSyntaxTrackingState::Unnamed);
	}
	pub fn start(&mut self) -> Marker {
		self.skip_trivia();
		let start_event_idx = self.events.len();
		self.events.push(Event::Pending);
		self.entered += 1;
		Marker::new(start_event_idx)
	}
	pub fn start_ranger(&mut self) -> Ranger {
		self.skip_trivia();
		let pos = self.offset;
		Ranger { pos }
	}
	pub fn parse(mut self) -> Vec<Event> {
		let m = self.start();
		expr(&mut self);
		self.expect(EOF);
		m.complete(&mut self, SOURCE_FILE);

		self.events
	}

	pub(crate) fn expect(&mut self, kind: SyntaxKind) {
		self.expect_with_recovery_set(kind, TS![])
	}

	pub(crate) fn expect_with_recovery_set(
		&mut self,
		kind: SyntaxKind,
		recovery_set: SyntaxKindSet,
	) {
		if self.at(kind) {
			if kind != EOF {
				self.bump();
			}
		} else {
			self.error_with_recovery_set(recovery_set);
		}
	}

	pub(crate) fn expect_with_no_skip(&mut self, kind: SyntaxKind) {
		if self.at(kind) {
			self.bump();
		} else {
			self.error_with_no_skip();
		}
	}
	fn current_token(&self) -> Lexeme<'i> {
		self.lexemes[self.offset]
	}
	fn previous_token(&mut self) -> Option<Lexeme<'i>> {
		if self.offset == 0 {
			return None;
		}
		let mut previous_token_idx = self.offset - 1;
		while self
			.lexemes
			.get(previous_token_idx)
			.map_or(false, |l| Trivia::can_cast(l.kind))
			&& previous_token_idx != 0
		{
			previous_token_idx -= 1;
		}

		Some(self.lexemes[previous_token_idx])
	}
	pub fn start_of_token(&self, mut idx: usize) -> TextSize {
		while Trivia::can_cast(self.lexemes[idx].kind) {
			idx += 1;
		}
		self.lexemes[idx].range.start()
	}
	pub fn end_of_token(&self, mut idx: usize) -> TextSize {
		while Trivia::can_cast(self.lexemes[idx].kind) {
			idx -= 1;
		}
		self.lexemes[idx].range.end()
	}
	pub(crate) fn custom_error(&mut self, marker: impl AsRange, error: impl AsRef<str>) {
		self.last_error_token = marker.end_token();
		self.events.push(Event::Error(SyntaxError::Custom {
			error: error.as_ref().to_string(),
			range: marker.as_range(self),
		}));
	}
	pub(crate) fn error_with_recovery_set(
		&mut self,
		recovery_set: SyntaxKindSet,
	) -> Option<CompletedMarker> {
		self.error_with_recovery_set_no_default(recovery_set.union(DEFAULT_RECOVERY_SET))
	}
	pub fn error_with_no_skip(&mut self) -> Option<CompletedMarker> {
		self.error_with_recovery_set_no_default(SyntaxKindSet::ALL)
	}

	pub fn error_with_recovery_set_no_default(
		&mut self,
		recovery_set: SyntaxKindSet,
	) -> Option<CompletedMarker> {
		let expected_syntax = self
			.expected_syntax
			.take()
			.unwrap_or(ExpectedSyntax::Named("unknown"));
		self.expected_syntax_tracking_state
			.set(ExpectedSyntaxTrackingState::Unnamed);

		self.skip_trivia();
		if self.at_end() || self.at_ts(recovery_set) {
			let range = self
				.previous_token()
				.map(|t| t.range)
				.unwrap_or_else(|| TextRange::at(TextSize::from(0), TextSize::from(0)));

			self.events.push(Event::Error(SyntaxError::Missing {
				expected: expected_syntax,
				offset: range.end(),
			}));
			return None;
		}

		let current_token = self.current_token();

		self.events.push(Event::Error(SyntaxError::Unexpected {
			expected: expected_syntax,
			found: current_token.kind,
			range: current_token.range,
		}));
		self.clear_expected_syntaxes();
		self.last_error_token = self.offset;

		let m = self.start();
		self.bump();
		Some(m.complete(self, SyntaxKind::ERROR))
	}
	fn bump_assert(&mut self, kind: SyntaxKind) {
		self.skip_trivia();
		assert!(self.at(kind), "expected {:?}", kind);
		self.bump_remap(self.current());
	}
	fn bump(&mut self) {
		self.skip_trivia();
		self.bump_remap(self.current());
	}
	fn bump_remap(&mut self, kind: SyntaxKind) {
		self.skip_trivia();
		assert_ne!(self.offset, self.lexemes.len(), "already at end");
		self.events.push(Event::Token { kind });
		self.offset += 1;
		self.clear_expected_syntaxes();
	}
	fn step(&self) {
		use std::fmt::Write;
		let steps = self.steps.get();
		if steps >= 15000000 {
			let mut out = "seems like parsing is stuck".to_owned();
			{
				let last = 20;
				write!(out, "\n\nLast {} events:", last).unwrap();
				for (i, event) in self
					.events
					.iter()
					.skip(self.events.len().saturating_sub(last))
					.enumerate()
				{
					write!(out, "\n{i}. {event:?}").unwrap();
				}
			}
			{
				let next = 20;
				write!(out, "\n\nNext {next} tokens:").unwrap();
				for (i, tok) in self.lexemes.iter().skip(self.offset).take(next).enumerate() {
					write!(out, "\n{i}. {tok:?}").unwrap();
				}
			}
			panic!("{out}")
		}
		self.steps.set(steps + 1);
	}
	fn nth(&self, i: usize) -> SyntaxKind {
		self.step();
		let mut offset = self.offset;
		for _ in 0..i {
			while self
				.lexemes
				.get(offset)
				.map(|l| Trivia::can_cast(l.kind))
				.unwrap_or(false)
			{
				offset += 1;
			}
			offset += 1;
		}
		while self
			.lexemes
			.get(offset)
			.map(|l| Trivia::can_cast(l.kind))
			.unwrap_or(false)
		{
			offset += 1;
		}
		self.lexemes.get(offset).map(|l| l.kind).unwrap_or(EOF)
	}
	fn current(&self) -> SyntaxKind {
		self.nth(0)
	}
	fn skip_trivia(&mut self) {
		while Trivia::can_cast(self.peek_raw()) {
			self.offset += 1;
		}
	}
	fn peek_raw(&mut self) -> SyntaxKind {
		self.lexemes
			.get(self.offset)
			.map(|l| l.kind)
			.unwrap_or(SyntaxKind::EOF)
	}
	#[must_use]
	pub(crate) fn expected_syntax_name(&mut self, name: &'static str) -> ExpectedSyntaxGuard {
		self.expected_syntax_tracking_state
			.set(ExpectedSyntaxTrackingState::Named);
		self.expected_syntax = Some(ExpectedSyntax::Named(name));

		ExpectedSyntaxGuard::new(Rc::clone(&self.expected_syntax_tracking_state))
	}
	pub fn at(&mut self, kind: SyntaxKind) -> bool {
		self.nth_at(0, kind)
	}
	pub fn nth_at(&mut self, n: usize, kind: SyntaxKind) -> bool {
		if let ExpectedSyntaxTrackingState::Unnamed = self.expected_syntax_tracking_state.get() {
			self.expected_syntax = Some(ExpectedSyntax::Unnamed(kind));
		}
		self.nth(n) == kind
	}
	pub fn at_ts(&mut self, set: SyntaxKindSet) -> bool {
		set.contains(self.current())
	}
	pub fn at_end(&mut self) -> bool {
		self.at(EOF)
	}
}
pub(crate) struct ExpectedSyntaxGuard {
	expected_syntax_tracking_state: Rc<Cell<ExpectedSyntaxTrackingState>>,
}

impl ExpectedSyntaxGuard {
	fn new(expected_syntax_tracking_state: Rc<Cell<ExpectedSyntaxTrackingState>>) -> Self {
		Self {
			expected_syntax_tracking_state,
		}
	}
}

impl Drop for ExpectedSyntaxGuard {
	fn drop(&mut self) {
		self.expected_syntax_tracking_state
			.set(ExpectedSyntaxTrackingState::Unnamed);
	}
}

#[derive(Debug, Clone, Copy)]
enum ExpectedSyntaxTrackingState {
	Named,
	Unnamed,
}

fn expr(p: &mut Parser) -> Option<CompletedMarker> {
	expr_binding_power(p, 0)
}
fn expr_binding_power(p: &mut Parser, minimum_binding_power: u8) -> Option<CompletedMarker> {
	let mut lhs = lhs(p)?;

	while let Some(op) = BinaryOperatorKind::cast(p.current())
		.or_else(|| p.at(T!['{']).then(|| BinaryOperatorKind::MetaObjectApply))
	{
		let (left_binding_power, right_binding_power) = op.binding_power();
		if left_binding_power < minimum_binding_power {
			break;
		}

		// Object apply is not a real operator, we dont have something to bump
		if op != BinaryOperatorKind::MetaObjectApply {
			p.bump();
		}

		let m = lhs.wrap(p, LHS_EXPR).precede(p);
		let parsed_rhs = expr_binding_power(p, right_binding_power).is_some();
		lhs = m.complete(
			p,
			if op == BinaryOperatorKind::MetaObjectApply {
				EXPR_OBJ_EXTEND
			} else {
				EXPR_BINARY
			},
		);

		if !parsed_rhs {
			break;
		}
	}
	Some(lhs)
}
fn compspec(p: &mut Parser) {
	assert!(p.at(T![for]) || p.at(T![if]));
	if p.at(T![for]) {
		let m = p.start();
		p.bump();
		name(p);
		p.expect(T![in]);
		expr(p);
		m.complete(p, FOR_SPEC);
	} else if p.at(T![if]) {
		let m = p.start();
		p.bump();
		expr(p);
		m.complete(p, IF_SPEC);
	} else {
		unreachable!()
	}
}
fn comma(p: &mut Parser) -> bool {
	if p.at(T![,]) {
		p.bump();
		true
	} else {
		false
	}
}
fn comma_with_alternatives(p: &mut Parser, set: SyntaxKindSet) -> bool {
	if p.at(T![,]) {
		p.bump();
		true
	} else if p.at_ts(set) {
		p.expect_with_no_skip(T![,]);
		p.bump();
		true
	} else {
		false
	}
}
fn field_name(p: &mut Parser) {
	let _e = p.expected_syntax_name("field name");
	let m = p.start();
	if p.at(T!['[']) {
		p.bump();
		expr(p);
		p.expect(T![']']);
		m.complete(p, FIELD_NAME_DYNAMIC);
	} else if p.at(IDENT) {
		name(p);
		m.complete(p, FIELD_NAME_FIXED);
	} else if Text::can_cast(p.current()) {
		text(p);
		m.complete(p, FIELD_NAME_FIXED);
	} else {
		p.error_with_recovery_set(TS![;]);
	}
}
fn visibility(p: &mut Parser) {
	if p.at_ts(TS![: :: :::]) {
		p.bump()
	} else {
		p.error_with_recovery_set(TS![]);
	}
}
fn field(p: &mut Parser) {
	let m = p.start();
	field_name(p);
	let plus = if p.at(T![+]) {
		let r = p.start_ranger();
		p.bump();
		Some(r.finish(p))
	} else {
		None
	};
	let params = if p.at(T!['(']) {
		if let Some(plus) = plus {
			p.custom_error(plus, "can't extend with method");
		}
		params_desc(p);
		if p.at(T![+]) {
			let r = p.start_ranger();
			p.bump();
			p.custom_error(r.finish(p), "can't extend with method");
		}
		true
	} else {
		false
	};
	visibility(p);
	expr(p);

	if params {
		m.complete(p, FIELD_METHOD)
	} else {
		m.complete(p, FIELD_NORMAL)
	};
}
fn assertion(p: &mut Parser) {
	let m = p.start();
	p.bump_assert(T![assert]);
	expr(p).map(|c| c.wrap(p, LHS_EXPR));
	if p.at(T![:]) {
		p.bump();
		expr(p);
	}
	m.complete(p, ASSERTION);
}
fn object(p: &mut Parser) -> CompletedMarker {
	let m_t = p.start();
	let m = p.start();
	p.bump_assert(T!['{']);

	loop {
		if p.at(T!['}']) {
			p.bump();
			break;
		}
		let m = p.start();
		if p.at(T![local]) {
			obj_local(p);
			m.complete(p, MEMBER_BIND_STMT)
		} else if p.at(T![assert]) {
			assertion(p);
			m.complete(p, MEMBER_ASSERT_STMT)
		} else {
			field(p);
			while p.at(T![for]) || p.at(T![if]) {
				compspec(p)
			}
			m.complete(p, MEMBER_FIELD)
		};
		if comma_with_alternatives(p, SyntaxKindSet::new(&[T![=]])) {
			continue;
		}
		p.expect(R_BRACE);
		break;
	}

	m.complete(p, OBJ_BODY_MEMBER_LIST);
	m_t.complete(p, EXPR_OBJECT)
}
fn param(p: &mut Parser) {
	let m = p.start();
	destruct(p);
	if p.at(T![=]) {
		p.bump();
		expr(p);
	}
	m.complete(p, PARAM);
}
fn params_desc(p: &mut Parser) -> CompletedMarker {
	let m = p.start();
	p.bump_assert(T!['(']);

	loop {
		if p.at(T![')']) {
			p.bump();
			break;
		}
		param(p);
		if comma(p) {
			continue;
		}
		p.expect(T![')']);
		break;
	}

	m.complete(p, PARAMS_DESC)
}
fn args_desc(p: &mut Parser) {
	let m = p.start();
	p.bump_assert(T!['(']);

	let started_named = Cell::new(false);

	loop {
		if p.at(T![')']) {
			break;
		}

		let m = p.start();
		if p.at(IDENT) && p.nth_at(1, T![=]) {
			name(p);
			p.bump();
			expr(p);
			m.complete(p, ARG);
			started_named.set(true);
		} else {
			expr(p);
			m.complete(p, ARG);
		}
		if comma(p) {
			continue;
		}
		break;
	}
	p.expect(T![')']);
	if p.at(T![tailstrict]) {
		p.bump()
	}
	m.complete(p, ARGS_DESC);
}

fn array(p: &mut Parser) -> CompletedMarker {
	// Start the list node
	let m = p.start();
	p.bump_assert(T!['[']);

	// This vec will have at most one element in case of correct input
	let mut compspecs = Vec::with_capacity(1);
	let mut elems = 0;

	loop {
		if p.at(T![']']) {
			p.bump();
			break;
		}
		elems += 1;
		expr(p);
		let c = p.start_ranger();
		let mut had_spec = false;
		while p.at(T![for]) || p.at(T![if]) {
			had_spec = true;
			compspec(p)
		}
		if had_spec {
			compspecs.push(c.finish(p));
		}
		if comma(p) {
			continue;
		}
		p.expect(T![']']);
		break;
	}

	if elems > 1 && !compspecs.is_empty() {
		for spec in compspecs {
			p.custom_error(
				spec,
				"compspec may only be used if there is only one array element",
			)
		}

		m.complete(p, EXPR_ARRAY)
	} else if !compspecs.is_empty() {
		m.complete(p, EXPR_ARRAY_COMP)
	} else {
		m.complete(p, EXPR_ARRAY)
	}
}
/// Returns true if it was slice, false if just index
#[must_use]
fn slice_desc_or_index(p: &mut Parser) -> bool {
	let m = p.start();
	p.bump();
	// TODO: do not treat :, ::, ::: as full tokens?
	// Start
	if !p.at(T![:]) && !p.at(T![::]) {
		expr(p);
	}
	if p.at(T![:]) {
		p.bump();
		// End
		if !p.at(T![']']) {
			expr(p).map(|c| c.wrap(p, SLICE_DESC_END));
		}
		if p.at(T![:]) {
			p.bump();
			// Step
			if !p.at(T![']']) {
				expr(p).map(|c| c.wrap(p, SLICE_DESC_STEP));
			}
		}
	} else if p.at(T![::]) {
		p.bump();
		// End
		if !p.at(T![']']) {
			expr(p).map(|c| c.wrap(p, SLICE_DESC_END));
		}
	} else {
		// It was not a slice
		p.expect(T![']']);
		m.forget(p);
		return false;
	}
	p.expect(T![']']);
	m.complete(p, SLICE_DESC);
	true
}
fn lhs(p: &mut Parser) -> Option<CompletedMarker> {
	let mut lhs = lhs_basic(p)?;

	loop {
		if p.at(T![.]) {
			let m = lhs.precede(p);
			p.bump();
			name(p);
			lhs = m.complete(p, EXPR_INDEX);
		} else if p.at(T!['[']) {
			if slice_desc_or_index(p) {
				lhs = lhs.precede(p).complete(p, EXPR_SLICE);
			} else {
				lhs = lhs
					.wrap(p, LHS_EXPR)
					.precede(p)
					.complete(p, EXPR_INDEX_EXPR);
			}
		} else if p.at(T!['(']) {
			let m = lhs.precede(p);
			args_desc(p);
			lhs = m.complete(p, EXPR_APPLY);
		} else {
			break;
		}
	}

	Some(lhs)
}
fn name(p: &mut Parser) {
	let m = p.start();
	p.expect(IDENT);
	m.complete(p, NAME);
}
fn destruct_rest(p: &mut Parser) {
	let m = p.start();
	p.bump_assert(T![...]);
	if p.at(IDENT) {
		p.bump()
	}
	m.complete(p, DESTRUCT_REST);
}
fn destruct_object_field(p: &mut Parser) {
	let m = p.start();
	name(p);
	if p.at(T![:]) {
		p.bump();
		destruct(p);
	};
	if p.at(T![=]) {
		p.bump();
		expr(p);
	}
	m.complete(p, DESTRUCT_OBJECT_FIELD);
}
fn obj_local(p: &mut Parser) {
	let m = p.start();
	p.bump_assert(T![local]);
	bind(p);
	m.complete(p, OBJ_LOCAL);
}
fn destruct(p: &mut Parser) -> CompletedMarker {
	let m = p.start();
	if p.at(T![?]) {
		p.bump();
		m.complete(p, DESTRUCT_SKIP)
	} else if p.at(T!['[']) {
		p.bump();
		let mut had_rest = false;
		loop {
			if p.at(T![']']) {
				p.bump();
				break;
			} else if p.at(T![...]) {
				let m_err = p.start_ranger();
				destruct_rest(p);
				if had_rest {
					p.custom_error(m_err.finish(p), "only one rest can be present in array");
				}
				had_rest = true;
			} else {
				destruct(p);
			}
			if p.at(T![,]) {
				p.bump();
				continue;
			}
			p.expect(T![']']);
			break;
		}
		m.complete(p, DESTRUCT_ARRAY)
	} else if p.at(T!['{']) {
		p.bump();
		let mut had_rest = false;
		loop {
			if p.at(T!['}']) {
				p.bump();
				break;
			} else if p.at(T![...]) {
				let m_err = p.start_ranger();
				destruct_rest(p);
				if had_rest {
					p.custom_error(m_err.finish(p), "only one rest can be present in object");
				}
				had_rest = true;
			} else {
				if had_rest {
					p.error_with_recovery_set(TS![]);
				}
				destruct_object_field(p);
			}
			if p.at(T![,]) {
				p.bump();
				continue;
			}
			p.expect(T!['}']);
			break;
		}
		m.complete(p, DESTRUCT_OBJECT)
	} else if p.at(IDENT) {
		name(p);
		m.complete(p, DESTRUCT_FULL)
	} else {
		m.complete(p, ERROR)
	}
}
fn bind(p: &mut Parser) {
	let m = p.start();
	if p.at(IDENT) && p.nth_at(1, T!['(']) {
		name(p);
		params_desc(p);
		p.expect(T![=]);
		expr(p);
		m.complete(p, BIND_FUNCTION)
	} else {
		destruct(p);
		p.expect(T![=]);
		expr(p);
		m.complete(p, BIND_DESTRUCT)
	};
}
fn text(p: &mut Parser) {
	assert!(Text::can_cast(p.current()));
	p.bump();
}
fn number(p: &mut Parser) {
	assert!(Number::can_cast(p.current()));
	p.bump();
}
fn literal(p: &mut Parser) {
	assert!(Literal::can_cast(p.current()));
	p.bump();
}
fn lhs_basic(p: &mut Parser) -> Option<CompletedMarker> {
	let _e = p.expected_syntax_name("value");
	Some(if Literal::can_cast(p.current()) {
		let m = p.start();
		literal(p);
		m.complete(p, EXPR_LITERAL)
	} else if Text::can_cast(p.current()) {
		let m = p.start();
		text(p);
		m.complete(p, EXPR_STRING)
	} else if Number::can_cast(p.current()) {
		let m = p.start();
		number(p);
		m.complete(p, EXPR_NUMBER)
	} else if p.at(IDENT) {
		let m = p.start();
		name(p);
		m.complete(p, EXPR_VAR)
	} else if p.at(INTRINSIC_THIS_FILE) {
		let m = p.start();
		p.bump();
		m.complete(p, EXPR_INTRINSIC_THIS_FILE)
	} else if p.at(INTRINSIC_ID) {
		let m = p.start();
		p.bump();
		m.complete(p, EXPR_INTRINSIC_ID)
	} else if p.at(INTRINSIC) {
		let m = p.start();
		p.bump();
		p.expect(T!['(']);
		name(p);
		p.expect(T![')']);
		m.complete(p, EXPR_INTRINSIC)
	} else if p.at(T![if]) {
		let m = p.start();
		p.bump();
		expr(p);
		p.expect(T![then]);
		expr(p).map(|c| c.wrap(p, TRUE_EXPR));
		if p.at(T![else]) {
			p.bump();
			expr(p).map(|c| c.wrap(p, FALSE_EXPR));
		}
		m.complete(p, EXPR_IF_THEN_ELSE)
	} else if p.at(T!['[']) {
		array(p)
	} else if p.at(T!['{']) {
		object(p)
	} else if p.at(T![local]) {
		let m = p.start();
		p.bump();
		loop {
			if p.at(T![;]) {
				p.bump();
				break;
			}
			bind(p);

			if p.at(T![,]) {
				p.bump();
				continue;
			}
			p.expect(T![;]);
			break;
		}
		expr(p);
		m.complete(p, EXPR_LOCAL)
	} else if p.at(T![function]) {
		let m = p.start();
		p.bump();
		params_desc(p);
		expr(p);
		m.complete(p, EXPR_FUNCTION)
	} else if p.at(T![error]) {
		let m = p.start();
		p.bump();
		expr(p);
		m.complete(p, EXPR_ERROR)
	} else if p.at(T![assert]) {
		let m = p.start();
		assertion(p);
		p.expect(T![;]);
		expr(p);
		m.complete(p, EXPR_ASSERT)
	} else if p.at(T![import]) || p.at(T![importstr]) || p.at(T![importbin]) {
		let m = p.start();
		p.bump();
		text(p);
		m.complete(p, EXPR_IMPORT)
	} else if let Some(op) = UnaryOperatorKind::cast(p.current()) {
		let ((), right_binding_power) = op.binding_power();

		let m = p.start();
		p.bump();
		expr_binding_power(p, right_binding_power);
		m.complete(p, EXPR_UNARY)
	} else if p.at(T!['(']) {
		let m = p.start();
		p.bump();
		expr(p);
		p.expect(T![')']);
		m.complete(p, EXPR_PARENED)
	} else {
		p.error_with_recovery_set(TS![]);
		return None;
	})
}

impl Parse {
	pub fn syntax(&self) -> SyntaxNode {
		SyntaxNode::new_root(self.green_node.clone())
	}
}
