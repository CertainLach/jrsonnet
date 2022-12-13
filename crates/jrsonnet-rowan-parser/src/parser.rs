use std::{cell::Cell, fmt, rc::Rc};

use miette::{LabeledSpan, SourceOffset, SourceSpan};
use rowan::{GreenNode, TextRange};

use crate::{
	event::Event,
	marker::{CompletedMarker, Marker, Ranger},
	nodes::{BinaryOperatorKind, Literal, Number, Text, UnaryOperatorKind},
	token_set::SyntaxKindSet,
	AstToken, SyntaxKind,
	SyntaxKind::*,
	SyntaxNode, T, TS,
};

pub struct Parse {
	pub green_node: GreenNode,
	pub errors: Vec<LocatedSyntaxError>,
}

pub struct Parser {
	// TODO: remove all trivia before feeding to parser?
	kinds: Vec<SyntaxKind>,
	pub offset: usize,
	pub events: Vec<Event>,
	pub entered: u32,
	pub hints: Vec<(u32, TextRange, String)>,
	pub last_error_token: usize,
	expected_syntax_tracking_state: Rc<Cell<ExpectedSyntax>>,
	steps: Cell<u64>,
}

#[derive(Clone, Debug)]
pub enum SyntaxError {
	Unexpected {
		expected: ExpectedSyntax,
		found: SyntaxKind,
	},
	Missing {
		expected: ExpectedSyntax,
	},
	Custom {
		error: String,
	},
	Hint {
		error: String,
	},
}

#[derive(Debug)]
pub struct LocatedSyntaxError {
	pub error: SyntaxError,
	pub range: TextRange,
}

impl From<LocatedSyntaxError> for LabeledSpan {
	fn from(val: LocatedSyntaxError) -> Self {
		let span = SourceSpan::new(
			SourceOffset::from(usize::from(val.range.start())),
			SourceOffset::from(usize::from(val.range.end() - val.range.start())),
		);
		dbg!(&val);
		match val.error {
			SyntaxError::Unexpected { expected, found } => LabeledSpan::new_with_span(
				Some(format!("expected {expected}, found {found:?}")),
				span,
			),
			SyntaxError::Missing { expected } => {
				LabeledSpan::new_with_span(Some(format!("missing {expected}")), span)
			}
			SyntaxError::Custom { error } | SyntaxError::Hint { error } => {
				LabeledSpan::new_with_span(Some(error), span)
			}
		}
	}
}

impl Parser {
	pub fn new(kinds: Vec<SyntaxKind>) -> Self {
		Self {
			kinds,
			offset: 0,
			events: vec![],
			entered: 0,
			last_error_token: 0,
			hints: vec![],
			expected_syntax_tracking_state: Rc::new(Cell::new(ExpectedSyntax::Unnamed(TS![]))),
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
		self.expected_syntax_tracking_state
			.set(ExpectedSyntax::Unnamed(TS![]));
	}
	pub fn start(&mut self) -> Marker {
		let start_event_idx = self.events.len();
		self.events.push(Event::Pending);
		self.entered += 1;
		Marker::new(start_event_idx)
	}
	pub fn start_ranger(&mut self) -> Ranger {
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
	pub fn error_with_no_skip(&mut self) -> CompletedMarker {
		self.error_with_recovery_set(SyntaxKindSet::ALL)
	}

	pub fn error_with_recovery_set(&mut self, recovery_set: SyntaxKindSet) -> CompletedMarker {
		let expected = self.expected_syntax_tracking_state.get();
		self.expected_syntax_tracking_state
			.set(ExpectedSyntax::Unnamed(TS![]));

		if self.at_end() || self.at_ts(recovery_set) {
			let m = self.start();
			return m.complete_missing(self, expected);
		}

		let current_token = self.current();

		self.last_error_token = self.offset;

		let m = self.start();
		self.bump();
		let m = m.complete_unexpected(self, expected, current_token);
		self.clear_expected_syntaxes();
		m
	}
	fn bump_assert(&mut self, kind: SyntaxKind) {
		assert!(self.at(kind), "expected {:?}", kind);
		self.bump_remap(self.current());
	}
	fn bump(&mut self) {
		self.bump_remap(self.current());
	}
	fn bump_remap(&mut self, kind: SyntaxKind) {
		assert_ne!(self.offset, self.kinds.len(), "already at end");
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
				for (i, tok) in self.kinds.iter().skip(self.offset).take(next).enumerate() {
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
			offset += 1;
		}
		self.kinds.get(offset).copied().unwrap_or(EOF)
	}
	fn current(&self) -> SyntaxKind {
		self.nth(0)
	}
	#[must_use]
	pub(crate) fn expected_syntax_name(&mut self, name: &'static str) -> ExpectedSyntaxGuard {
		self.expected_syntax_tracking_state
			.set(ExpectedSyntax::Named(name));

		ExpectedSyntaxGuard::new(Rc::clone(&self.expected_syntax_tracking_state))
	}
	pub fn at(&mut self, kind: SyntaxKind) -> bool {
		self.nth_at(0, kind)
	}
	pub fn nth_at(&mut self, n: usize, kind: SyntaxKind) -> bool {
		if n == 0 {
			if let ExpectedSyntax::Unnamed(kinds) = self.expected_syntax_tracking_state.get() {
				let kinds = kinds.with(kind);
				self.expected_syntax_tracking_state
					.set(ExpectedSyntax::Unnamed(kinds))
			}
		}
		self.nth(n) == kind
	}
	pub fn at_ts(&mut self, set: SyntaxKindSet) -> bool {
		if let ExpectedSyntax::Unnamed(kinds) = self.expected_syntax_tracking_state.get() {
			let kinds = kinds.union(set);
			self.expected_syntax_tracking_state
				.set(ExpectedSyntax::Unnamed(kinds))
		}
		set.contains(self.current())
	}
	pub fn at_end(&mut self) -> bool {
		self.at(EOF)
	}
}
pub(crate) struct ExpectedSyntaxGuard {
	expected_syntax_tracking_state: Rc<Cell<ExpectedSyntax>>,
}

impl ExpectedSyntaxGuard {
	fn new(expected_syntax_tracking_state: Rc<Cell<ExpectedSyntax>>) -> Self {
		Self {
			expected_syntax_tracking_state,
		}
	}
}

impl Drop for ExpectedSyntaxGuard {
	fn drop(&mut self) {
		self.expected_syntax_tracking_state
			.set(ExpectedSyntax::Unnamed(TS![]));
	}
}

#[derive(Clone, Debug, Copy)]
pub enum ExpectedSyntax {
	Named(&'static str),
	Unnamed(SyntaxKindSet),
}
impl fmt::Display for ExpectedSyntax {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ExpectedSyntax::Named(name) => write!(f, "{name}"),
			ExpectedSyntax::Unnamed(set) => write!(f, "{set}"),
		}
	}
}

fn expr(p: &mut Parser) -> CompletedMarker {
	match expr_binding_power(p, 0) {
		Ok(m) => m,
		Err(m) => m,
	}
}
fn expr_binding_power(
	p: &mut Parser,
	minimum_binding_power: u8,
) -> Result<CompletedMarker, CompletedMarker> {
	let mut lhs = lhs(p)?;

	while let Some(op) = BinaryOperatorKind::cast(p.current())
		.or_else(|| p.at(T!['{']).then_some(BinaryOperatorKind::MetaObjectApply))
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
		let parsed_rhs = expr_binding_power(p, right_binding_power).is_ok();
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
	Ok(lhs)
}

const COMPSPEC: SyntaxKindSet = TS![for if];
fn compspec(p: &mut Parser) -> CompletedMarker {
	assert!(p.at_ts(COMPSPEC));
	if p.at(T![for]) {
		let m = p.start();
		p.bump();
		name(p);
		p.expect(T![in]);
		expr(p);
		m.complete(p, FOR_SPEC)
	} else if p.at(T![if]) {
		let m = p.start();
		p.bump();
		expr(p);
		m.complete(p, IF_SPEC)
	} else {
		unreachable!()
	}
}

fn comma(p: &mut Parser) -> bool {
	comma_with_alternatives(p, TS![])
}
fn comma_with_alternatives(p: &mut Parser, set: SyntaxKindSet) -> bool {
	if p.at(T![,]) {
		p.bump();
		true
	} else if p.at_ts(set) {
		let _ex = p.expected_syntax_name("comma");
		p.expect_with_recovery_set(T![,], TS![]);
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
		m.forget(p);
		p.error_with_recovery_set(TS![; : :: ::: '(']);
	}
}
fn visibility(p: &mut Parser) {
	if p.at_ts(TS![: :: :::]) {
		p.bump()
	} else {
		p.error_with_recovery_set(TS![=]);
	}
}
fn assertion(p: &mut Parser) {
	let m = p.start();
	p.bump_assert(T![assert]);
	expr(p).wrap(p, LHS_EXPR);
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

	let mut elems = 0;
	let mut compspecs = Vec::new();
	loop {
		if p.at(T!['}']) {
			p.bump();
			break;
		}
		if p.at_ts(COMPSPEC) {
			if elems == 0 {
				let m = p.start();
				m.complete_missing(p, ExpectedSyntax::Named("field definition"));
			}
			while p.at_ts(COMPSPEC) {
				compspecs.push(compspec(p));
			}
			if comma_with_alternatives(p, TS![;]) {
				continue;
			}
			p.expect(R_BRACE);
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
			field_name(p);
			if p.at(T![+]) {
				p.bump();
			}
			let params = if p.at(T!['(']) {
				params_desc(p);
				visibility(p);
				expr(p);
				true
			} else if p.at_ts(TS![: :: :::]) && p.nth_at(1, T![function]) {
				visibility(p);
				p.bump_assert(T![function]);
				params_desc(p);
				expr(p);
				true
			} else {
				visibility(p);
				expr(p);
				false
			};

			if params {
				m.complete(p, MEMBER_FIELD_METHOD)
			} else {
				m.complete(p, MEMBER_FIELD_NORMAL)
			}
		};
		elems += 1;
		while p.at_ts(COMPSPEC) {
			compspecs.push(compspec(p));
		}
		if comma_with_alternatives(p, TS![;]) {
			continue;
		}
		p.expect(R_BRACE);
		break;
	}

	if elems > 1 && !compspecs.is_empty() {
		for errored in compspecs {
			errored.wrap_error(
				p,
				"compspec may only be used if there is only one array element",
			);
		}
		m.complete(p, OBJ_BODY_MEMBER_LIST);
	} else if !compspecs.is_empty() {
		m.complete(p, OBJ_BODY_COMP);
	} else {
		m.complete(p, OBJ_BODY_MEMBER_LIST);
	}
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
	let mut unnamed_after_named = Vec::new();

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
			let arg = m.complete(p, ARG);
			if started_named.get() {
				unnamed_after_named.push(arg)
			}
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

	for errored in unnamed_after_named {
		errored.wrap_error(p, "can't use positional arguments after named");
	}

	m.complete(p, ARGS_DESC);
}

fn array(p: &mut Parser) -> CompletedMarker {
	// Start the list node
	let m = p.start();
	p.bump_assert(T!['[']);

	let mut compspecs = Vec::new();
	let mut elems = 0;

	loop {
		if p.at(T![']']) {
			p.bump();
			break;
		}
		if elems != 0 && p.at_ts(COMPSPEC) {
			while p.at_ts(COMPSPEC) {
				compspecs.push(compspec(p));
			}
			if comma(p) {
				continue;
			}
			p.expect(T![']']);
			break;
		}
		elems += 1;
		expr(p);
		while p.at_ts(COMPSPEC) {
			compspecs.push(compspec(p));
		}
		if comma(p) {
			continue;
		}
		p.expect(T![']']);
		break;
	}

	if elems > 1 && !compspecs.is_empty() {
		for spec in compspecs {
			spec.wrap_error(
				p,
				"compspec may only be used if there is only one array element",
			);
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
			expr(p).wrap(p, SLICE_DESC_END);
		}
		if p.at(T![:]) {
			p.bump();
			// Step
			if !p.at(T![']']) {
				expr(p).wrap(p, SLICE_DESC_STEP);
			}
		}
	} else if p.at(T![::]) {
		p.bump();
		// End
		if !p.at(T![']']) {
			expr(p).wrap(p, SLICE_DESC_END);
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
fn lhs(p: &mut Parser) -> Result<CompletedMarker, CompletedMarker> {
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

	Ok(lhs)
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
	let _ex = p.expected_syntax_name("destruction specifier");
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
				// if had_rest {
				// 	p.custom_error(m_err.finish(p), "only one rest can be present in array");
				// }
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
				// if had_rest {
				// 	p.custom_error(m_err.finish(p), "only one rest can be present in object");
				// }
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
		m.forget(p);
		p.error_with_recovery_set(TS![; , '}', '(', :])
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
	} else if p.at(IDENT) && p.nth_at(1, T![=]) && p.nth_at(2, T![function]) {
		name(p);
		p.expect(T![=]);
		p.expect(T![function]);
		params_desc(p);
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
fn lhs_basic(p: &mut Parser) -> Result<CompletedMarker, CompletedMarker> {
	let _e = p.expected_syntax_name("expression");
	Ok(if Literal::can_cast(p.current()) {
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
		expr(p).wrap(p, TRUE_EXPR);
		if p.at(T![else]) {
			p.bump();
			expr(p).wrap(p, FALSE_EXPR);
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
		let _ = expr_binding_power(p, right_binding_power);
		m.complete(p, EXPR_UNARY)
	} else if p.at(T!['(']) {
		let m = p.start();
		p.bump();
		expr(p);
		p.expect(T![')']);
		m.complete(p, EXPR_PARENED)
	} else {
		return Err(p.error_with_no_skip());
	})
}

impl Parse {
	pub fn syntax(&self) -> SyntaxNode {
		SyntaxNode::new_root(self.green_node.clone())
	}
}
