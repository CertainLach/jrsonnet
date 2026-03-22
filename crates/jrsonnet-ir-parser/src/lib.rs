use std::rc::Rc;

use insta::assert_snapshot;
use jrsonnet_gcmodule::Acyclic;
use jrsonnet_ir::{
	AssertExpr, AssertStmt, Expr, IfElse, IfSpecData, LiteralType, Slice, SliceDesc, Source,
	SourceVirtual, Span, Spanned,
};
use jrsonnet_lexer::{Lexeme, Lexer, SyntaxKind, T};

struct Parser<'a> {
	lexemes: Vec<Lexeme<'a>>,
	offset: usize,
	source: Source,
}

impl<'a> Parser<'a> {
	fn new(s: &'a str) -> Self {
		Self {
			lexemes: Lexer::new(s)
				.filter(|l| l.kind != SyntaxKind::WHITESPACE)
				.collect(),
			offset: 0,
			source: Source::new_virtual("<test>".into(), s.into()),
		}
	}
	fn peek(&self) -> SyntaxKind {
		self.lexemes[self.offset].kind
	}
	fn text(&self) -> &str {
		self.lexemes[self.offset].text
	}
	fn at(&self, kind: SyntaxKind) -> bool {
		!self.at_eof() && self.peek() == kind
	}
	fn eat_any(&mut self) {
		self.offset += 1
	}

	fn at_eof(&self) -> bool {
		self.offset == self.lexemes.len()
	}

	fn try_eat(&mut self, t: SyntaxKind) -> bool {
		if self.at(t) {
			self.eat_any();
			return true;
		}
		false
	}
	fn eat(&mut self, t: SyntaxKind) {
		assert_eq!(self.peek(), t);
		self.eat_any();
	}

	fn span_start(&self) -> u32 {
		self.lexemes[self.offset].range.0
	}
	fn span_end(&self) -> u32 {
		self.lexemes[self.offset - 1].range.1
	}
}

fn literal(p: &mut Parser<'_>) -> Option<LiteralType> {
	let t = match p.peek() {
		T![self] => LiteralType::This,
		T![super] => LiteralType::Super,
		T!['$'] => LiteralType::Dollar,
		T![null] => LiteralType::Null,
		T![true] => LiteralType::True,
		T![false] => LiteralType::False,
		_ => return None,
	};
	p.eat_any();
	Some(t)
}

fn spanned<T: Acyclic>(p: &mut Parser<'_>, cb: impl FnOnce(&mut Parser<'_>) -> T) -> Spanned<T> {
	let start = p.span_start();
	let v = cb(p);
	let end = p.span_end();

	Spanned::new(v, Span(p.source.clone(), start, end))
}

fn assert_stmt(p: &mut Parser<'_>) -> AssertStmt {
	p.eat(T![assert]);
	let cond = spanned(p, expr);
	dbg!(p.peek());
	let msg = if p.try_eat(T![:]) {
		Some(spanned(p, expr))
	} else {
		None
	};
	dbg!(AssertStmt(cond, msg))
}

fn if_spec_data(p: &mut Parser<'_>) -> IfSpecData {
	let v = spanned(p, |p| p.eat(T![if]));
	let cond = expr(p);
	IfSpecData { span: v.span, cond }
}

fn if_else(p: &mut Parser<'_>) -> IfElse {
	let cond = if_spec_data(p);
	p.eat(T![then]);
	let cond_then = expr(p);
	let cond_else = if p.at(T![else]) { Some(expr(p)) } else { None };
	IfElse {
		cond,
		cond_then,
		cond_else,
	}
}

fn slice_desc(p: &mut Parser<'_>, start: Option<Spanned<Expr>>) -> SliceDesc {
	// start
	p.eat(T![:]);
	let end = if !p.at(T![:]) && !p.at(T![']']) {
		Some(spanned(p, expr))
	} else {
		None
	};
	let step = if p.try_eat(T![:]) && !p.at(T![']']) {
		Some(spanned(p, expr))
	} else {
		None
	};
	SliceDesc { start, end, step }
}

fn expr_simple(p: &mut Parser<'_>) -> Expr {
	let mut e = if let Some(literal) = literal(p) {
		Expr::Literal(literal)
	} else if p.at(T![assert]) {
		let assert = assert_stmt(p);
		p.eat(T![;]);
		let rest = expr(p);
		Expr::AssertExpr(Rc::new(AssertExpr { assert, rest }))
	} else if p.at(T![if]) {
		Expr::IfElse(Box::new(if_else(p)))
	} else {
		panic!("unexpected token: {:?}", p.peek());
	};

	dbg!(&e);

	loop {
		if p.try_eat(T!['[']) {
			if p.at(T![:]) {
				let slice = slice_desc(p, None);
				e = Expr::Slice(Box::new(Slice { value: e, slice }));
				p.eat(T![']']);
				continue;
			}

			let idx = spanned(p, expr);
			if p.at(T![:]) {
				let slice = slice_desc(p, Some(idx));
				e = Expr::Slice(Box::new(Slice { value: e, slice }));
			} else {
			}
			p.eat(T![']']);
		} else {
			break;
		}
	}

	dbg!(e)
}

fn expr(p: &mut Parser<'_>) -> Expr {
	expr_simple(p)
}

#[test]
fn basic_test() {
	let mut parser = Parser::new(" assert true[false] : false ; true ");
	let e = expr(&mut parser);
	let l = &parser.lexemes;

	assert_snapshot!(format!("{l:#?}\n\n---\n\n{e:#?}"));
}
