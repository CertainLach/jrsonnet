use std::cell::Cell;
use std::fmt::Display;
use std::rc::Rc;

use miette::Diagnostic;
use miette::LabeledSpan;
use miette::SourceOffset;
use miette::SourceSpan;
use rowan::GreenNode;

use rowan::TextRange;
use rowan::TextSize;
use thiserror::Error;

use crate::binary::BinaryOperator;
use crate::event::Event;
use crate::event::Sink;
use crate::lex::lex;
use crate::lex::Lang;
use crate::lex::Lexeme;
use crate::lex::SyntaxKind;
use crate::lex::SyntaxKind::*;
use crate::marker::AsRange;
use crate::marker::CompletedMarker;
use crate::marker::FinishedRanger;
use crate::marker::Marker;
use crate::marker::Ranger;
use crate::token_set::TokenSet;
use crate::unary::UnaryOperator;

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
	lexemes: &'i [Lexeme<'i>],
	pub offset: usize,
	pub events: Vec<Event>,
	pub entered: u32,
	pub hints: Vec<(u32, TextRange, String)>,
	pub last_error_token: usize,
	expected_syntax: Option<ExpectedSyntax>,
	expected_syntax_tracking_state: Rc<Cell<ExpectedSyntaxTrackingState>>,
}

const DEFAULT_RECOVERY_SET: TokenSet = TokenSet::new(&[
	SymbolSemi,
	RParen,
	SymbolRightBracket,
	SymbolRightBrace,
	KeywordLocal,
]);

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

impl Into<LabeledSpan> for SyntaxError {
	fn into(self) -> LabeledSpan {
		match self {
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
					Some(format!("{}", error)),
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
	fn new(lexemes: &'i [Lexeme<'i>]) -> Self {
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
		let start_event_idx = self.events.len();
		self.events.push(Event::Placeholder);
		self.entered += 1;
		Marker::new(start_event_idx, self.offset)
	}
	pub fn start_ranger(&mut self) -> Ranger {
		let pos = self.offset;
		Ranger { pos }
	}
	fn parse(mut self) -> Vec<Event> {
		let m = self.start();
		expr(&mut self);
		if !self.at_end() {
			let ranger = self.start_ranger();

			while self.peek().is_some() {
				self.bump()
			}
			let end = ranger.finish(&self);
			self.custom_error(end, "unexpected input after expression");
		}
		m.complete(&mut self, Root);

		self.events
	}

	pub(crate) fn expect(&mut self, kind: SyntaxKind) {
		self.expect_with_recovery_set(kind, TokenSet::default())
	}

	pub(crate) fn expect_with_recovery_set(&mut self, kind: SyntaxKind, recovery_set: TokenSet) {
		if self.at(kind) {
			self.bump();
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
	pub(crate) fn last_token_range(&self) -> Option<TextRange> {
		self.lexemes.last().map(|Lexeme { range, .. }| *range)
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
			.map_or(false, |l| l.kind.is_trivia())
			&& previous_token_idx != 0
		{
			previous_token_idx -= 1;
		}

		Some(self.lexemes[previous_token_idx])
	}
	pub fn start_of_token(&self, mut idx: usize) -> TextSize {
		while self.lexemes[idx].kind.is_trivia() {
			idx += 1;
		}
		self.lexemes[idx].range.start()
	}
	pub fn end_of_token(&self, mut idx: usize) -> TextSize {
		while self.lexemes[idx].kind.is_trivia() {
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
		recovery_set: TokenSet,
	) -> Option<CompletedMarker> {
		self.error_with_recovery_set_no_default(recovery_set.union(DEFAULT_RECOVERY_SET))
	}
	pub fn error_with_no_skip(&mut self) -> Option<CompletedMarker> {
		self.error_with_recovery_set_no_default(TokenSet::ALL)
	}

	pub fn error_with_recovery_set_no_default(
		&mut self,
		recovery_set: TokenSet,
	) -> Option<CompletedMarker> {
		let expected_syntax = self.expected_syntax.take().unwrap();
		self.expected_syntax_tracking_state
			.set(ExpectedSyntaxTrackingState::Unnamed);

		if self.at_end() || self.at_set(recovery_set) {
			let range = self
				.previous_token()
				.map(|t| t.range)
				.unwrap_or(TextRange::at(TextSize::from(0), TextSize::from(0)));

			self.events.push(Event::Error(SyntaxError::Missing {
				expected: expected_syntax,
				offset: range.end(),
			}));
			return None;
		}

		let current_token = self.current_token();

		self.events.push(Event::Error(SyntaxError::Unexpected {
			expected: expected_syntax.clone(),
			found: current_token.kind,
			range: current_token.range,
		}));
		self.clear_expected_syntaxes();
		self.last_error_token = self.offset;

		let m = self.start();
		self.bump();
		Some(m.complete(self, SyntaxKind::Error))
	}

	fn bump(&mut self) {
		self.skip_trivia();
		assert_ne!(self.offset, self.lexemes.len(), "already at end");
		self.events.push(Event::Token);
		self.offset += 1;
		self.clear_expected_syntaxes();
	}
	fn peek(&mut self) -> Option<SyntaxKind> {
		self.skip_trivia();
		self.peek_raw()
	}
	pub fn peek_token(&mut self) -> Option<&Lexeme<'i>> {
		self.skip_trivia();
		self.peek_token_raw()
	}
	fn skip_trivia(&mut self) {
		while self.peek_raw().map(|c| c.is_trivia()).unwrap_or(false) {
			self.offset += 1;
		}
	}
	fn peek_raw(&mut self) -> Option<SyntaxKind> {
		self.lexemes.get(self.offset).map(|l| l.kind)
	}
	fn peek_token_raw(&mut self) -> Option<&Lexeme<'i>> {
		self.lexemes.get(self.offset)
	}
	#[must_use]
	pub(crate) fn expected_syntax_name(&mut self, name: &'static str) -> ExpectedSyntaxGuard {
		self.expected_syntax_tracking_state
			.set(ExpectedSyntaxTrackingState::Named);
		self.expected_syntax = Some(ExpectedSyntax::Named(name));

		ExpectedSyntaxGuard::new(Rc::clone(&self.expected_syntax_tracking_state))
	}
	pub fn at(&mut self, kind: SyntaxKind) -> bool {
		if let ExpectedSyntaxTrackingState::Unnamed = self.expected_syntax_tracking_state.get() {
			self.expected_syntax = Some(ExpectedSyntax::Unnamed(kind));
		}
		self.peek() == Some(kind)
	}
	pub fn at_set(&mut self, set: TokenSet) -> bool {
		self.peek().map_or(false, |k| set.contains(k))
	}
	pub fn at_end(&mut self) -> bool {
		self.peek().is_none()
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
macro_rules! at_match {
	($p:ident {
		$($r:ident => $e:expr,)*
		_ => $else:expr $(,)?
	}) => {{
		$(
			if $p.at($r) {$e} else
		)* {
			$else
		}
	}}
}

fn expr(p: &mut Parser) {
	expr_binding_power(p, 0);
}
fn expr_binding_power(p: &mut Parser, minimum_binding_power: u8) -> Option<CompletedMarker> {
	let mut lhs = lhs(p)?;

	loop {
		let op = at_match!(p {
			OpMul => BinaryOperator::Mul,
			OpDiv => BinaryOperator::Div,
			OpMod => BinaryOperator::Mod,
			OpPlus => BinaryOperator::Plus,
			OpMinus => BinaryOperator::Minus,
			OpShiftLeft => BinaryOperator::ShiftLeft,
			OpShiftRight => BinaryOperator::ShiftRight,
			OpLessThan => BinaryOperator::LessThan,
			OpGreaterThan => BinaryOperator::GreaterThan,
			OpLessThanOrEqual => BinaryOperator::LessThanOrEqual,
			OpGreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
			OpEqual => BinaryOperator::Equal,
			OpNotEqual => BinaryOperator::NotEqual,
			OpBitAnd => BinaryOperator::BitAnd,
			OpBitXor => BinaryOperator::BitXor,
			OpBitOr => BinaryOperator::BitOr,
			OpAnd => BinaryOperator::And,
			OpOr => BinaryOperator::Or,
			OpIn => BinaryOperator::In,
			SymbolLeftBrace => BinaryOperator::ObjectApply,
			_ => break,
		});
		let (left_binding_power, right_binding_power) = op.binding_power();
		if left_binding_power < minimum_binding_power {
			break;
		}

		// Object apply is not a real operator, we dont have something to bump
		if op != BinaryOperator::ObjectApply {
			p.bump();
		}

		let m = lhs.precede(p);
		let parsed_rhs = expr_binding_power(p, right_binding_power).is_some();
		lhs = m.complete(
			p,
			if op == BinaryOperator::ObjectApply {
				ObjectApply
			} else {
				BinOp
			},
		);

		if !parsed_rhs {
			break;
		}
	}
	Some(lhs)
}
fn compspec(p: &mut Parser) {
	assert!(p.at(KeywordFor) || p.at(KeywordIf));
	if p.at(KeywordFor) {
		let m = p.start();
		p.bump();
		p.expect(Ident);
		p.expect(OpIn);
		expr(p);
		m.complete(p, CompspecFor);
	} else if p.at(KeywordIf) {
		let m = p.start();
		p.bump();
		expr(p);
		m.complete(p, CompspecIf);
	} else {
		unreachable!()
	}
}
fn comma(p: &mut Parser) -> bool {
	if p.at(SymbolComma) {
		p.bump();
		true
	} else {
		false
	}
}
fn comma_with_alternatives(p: &mut Parser, set: TokenSet) -> bool {
	if p.at(SymbolComma) {
		p.bump();
		true
	} else if p.at_set(set) {
		p.expect_with_no_skip(SymbolComma);
		p.bump();
		true
	} else {
		false
	}
}
fn field_name(p: &mut Parser) {
	let _e = p.expected_syntax_name("field name");
	if p.at(SymbolLeftBracket) {
		p.bump();
		expr(p);
		p.expect(SymbolRightBracket);
	} else if p.at(Ident) {
		p.bump()
	} else {
		p.error_with_recovery_set(TokenSet::new(&[SymbolSemi]));
	}
}
fn object(p: &mut Parser) -> CompletedMarker {
	assert!(p.at(SymbolLeftBrace));
	let m = p.start();
	p.bump();

	loop {
		if p.at(SymbolRightBrace) {
			p.bump();
			break;
		}
		let m = p.start();
		field_name(p);
		p.expect(SymbolColon);
		expr(p);
		while p.at(KeywordFor) || p.at(KeywordIf) {
			compspec(p)
		}
		m.complete(p, Field);
		if comma_with_alternatives(p, TokenSet::new(&[SymbolAssign])) {
			continue;
		}
		p.expect(SymbolRightBrace);
		break;
	}

	m.complete(p, Object)
}

fn params(p: &mut Parser) -> CompletedMarker {
	assert!(p.at(LParen));
	let m = p.start();
	p.bump();

	loop {
		if p.at(RParen) {
			p.bump();
			break;
		}
		let m = p.start();
		p.expect(Ident);
		if p.at(SymbolAssign) {
			p.bump();
			expr(p);
		}
		m.complete(p, DefParam);
		if comma(p) {
			continue;
		}
		p.expect(RParen);
		break;
	}

	m.complete(p, DefParams)
}
fn args(p: &mut Parser) {
	assert!(p.at(LParen));
	p.bump();

	let mut error_positional_start = None::<Marker>;
	let mut started_named = Cell::new(false);
	let mut on_positional = |p: &mut Parser, m: Marker| {
		let c = m.complete(p, DefPositionalArg);
		if started_named.get() && error_positional_start.is_none() {
			error_positional_start = Some(c.precede(p));
		}
	};
	loop {
		if p.at(RParen) {
			break;
		}

		let m = p.start();
		if p.at(Ident) {
			p.bump();
			if p.at(SymbolAssign) {
				p.bump();
				expr(p);
				m.complete(p, DefNamedArg);
				started_named.set(true);
			} else {
				on_positional(p, m);
			}
		} else {
			expr(p);
			on_positional(p, m);
		}
		if comma(p) {
			continue;
		}
		break;
	}
	if let Some(error_positional_start) = error_positional_start {
		let c = error_positional_start.complete(p, ErrorPositionalAfterNamed);
		p.custom_error(c, "positional arguments can't be placed after named")
	}
	p.expect(RParen);
}

fn array(p: &mut Parser) -> CompletedMarker {
	assert!(p.at(SymbolLeftBracket));
	// Start the list node
	let m = p.start();
	p.bump(); // '['

	// This vec will have at most one element in case of correct input
	let mut compspecs = Vec::with_capacity(1);
	let mut elems = 0;

	loop {
		if p.at(SymbolRightBracket) {
			p.bump();
			break;
		}
		elems += 1;
		let m = p.start();
		{
			let m = p.start();
			expr(p);
			m.complete(p, BodyDef);
		}
		let c = p.start_ranger();
		let mut had_spec = false;
		while p.at(KeywordFor) || p.at(KeywordIf) {
			had_spec = true;
			compspec(p)
		}
		if had_spec {
			compspecs.push(c.finish(p));
		}
		m.complete(p, ArrayElem);
		if comma(p) {
			continue;
		}
		p.expect(SymbolRightBracket);
		break;
	}

	if elems > 1 && !compspecs.is_empty() {
		for spec in compspecs {
			p.custom_error(
				spec,
				"compspec may only be used if there is only one array element",
			)
		}
	}

	m.complete(p, Array)
}

fn lhs(p: &mut Parser) -> Option<CompletedMarker> {
	let mut lhs = lhs_basic(p)?;

	loop {
		if p.at(SymbolDot) {
			let m = lhs.precede(p);
			p.bump();
			p.expect(Ident);
			lhs = m.complete(p, FieldAccess);
		} else if p.at(SymbolLeftBracket) {
			let m = lhs.precede(p);
			p.bump();
			// Start
			if !p.at(SymbolColon) {
				expr(p);
			}
			if p.at(SymbolColon) {
				p.bump();
				// End
				if !p.at(SymbolRightBracket) && !p.at(SymbolColon) {
					expr(p);
				}
				if p.at(SymbolColon) {
					p.bump();
					// Step
					if !p.at(SymbolRightBracket) {
						expr(p);
					}
				}
			}
			p.expect(SymbolRightBracket);
			lhs = m.complete(p, Slice);
		} else if p.at(LParen) {
			let m = lhs.precede(p);
			args(p);
			lhs = m.complete(p, FunctionCall);
		} else {
			break;
		}
	}

	Some(lhs)
}

fn lhs_basic(p: &mut Parser) -> Option<CompletedMarker> {
	let _e = p.expected_syntax_name("value");
	Some(
		if p.at(Number)
			|| p.at(StringSingleQuoted)
			|| p.at(StringDoubleQuoted)
			|| p.at(StringSingleVerbatim)
			|| p.at(StringDoubleVerbatim)
			|| p.at(StringBlock)
			|| p.at(KeywordNull)
			|| p.at(SymbolDollar)
			|| p.at(KeywordSuper)
			|| p.at(KeywordSelf)
		{
			let m = p.start();
			p.bump();
			m.complete(p, Literal)
		} else if p.at(Ident) {
			let m = p.start();
			p.bump();
			m.complete(p, Ident)
		} else if p.at(SymbolLeftBracket) {
			array(p)
		} else if p.at(SymbolLeftBrace) {
			object(p)
		} else if p.at(KeywordLocal) {
			let m = p.start();
			p.bump();
			let mut sus_local = None;
			loop {
				p.expect_with_recovery_set(
					Ident,
					TokenSet::new(&[SymbolAssign, SymbolSemi, KeywordLocal]),
				);
				if p.at(LParen) {
					params(p);
				}

				let sus_local_candidate = p.start_ranger();
				p.expect_with_recovery_set(
					SymbolAssign,
					TokenSet::new(&[SymbolSemi, KeywordLocal]),
				);

				sus_local = p.at(KeywordLocal).then(|| sus_local_candidate.finish(p));
				expr(p);

				if !comma(p) {
					break;
				}
			}
			p.expect(SymbolSemi);
			if let Some(sus_local) = sus_local {
				if sus_local.had_error_since(p) {
					p.custom_error(sus_local, "unusal local placement, missing ';' ?")
				}
			}
			{
				let m = p.start();
				expr(p);
				m.complete(p, BodyDef);
			}
			m.complete(p, Local)
		} else if p.at(KeywordFunction) {
			let m = p.start();
			p.bump();
			args(p);
			{
				let m = p.start();
				expr(p);
				m.complete(p, BodyDef);
			}
			m.complete(p, FunctionDef)
		} else if p.at(KeywordError) {
			let m = p.start();
			p.bump();
			expr(p);
			m.complete(p, ExprError)
		} else if p.at(KeywordAssert) {
			let m = p.start();
			p.bump();
			expr(p);
			if p.at(SymbolColon) {
				p.bump();
				expr(p);
			}
			m.complete(p, ExprAssert)
		} else if p.at(KeywordImport) || p.at(KeywordImportStr) {
			let m = p.start();
			p.bump();
			expr(p);
			m.complete(p, ExprImport)
		} else if p.at(OpMinus) || p.at(OpNot) || p.at(OpBitNegate) {
			let op = match p.peek().unwrap() {
				OpMinus => UnaryOperator::Minus,
				OpNot => UnaryOperator::Not,
				OpBitNegate => UnaryOperator::BitNegate,
				_ => unreachable!(),
			};
			let ((), right_binding_power) = op.binding_power();

			let m = p.start();
			p.bump();
			expr_binding_power(p, right_binding_power);
			m.complete(p, UnaryOp)
		} else if p.at(LParen) {
			let m = p.start();
			p.bump();
			expr(p);
			assert!(p.at(RParen));
			p.bump();
			m.complete(p, Parened)
		} else {
			p.error_with_no_skip();
			return None;
		},
	)
}

type SyntaxNode = rowan::SyntaxNode<Lang>;
#[allow(unused)]
type SyntaxToken = rowan::SyntaxToken<Lang>;
#[allow(unused)]
type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

impl Parse {
	pub fn syntax(&self) -> SyntaxNode {
		SyntaxNode::new_root(self.green_node.clone())
	}
}

pub fn parse(input: &str) -> Parse {
	let lexemes = lex(input);
	let parser = Parser::new(&lexemes);
	let events = parser.parse();
	dbg!(&events);
	let sink = Sink::new(events, &lexemes);

	sink.finish()
}
