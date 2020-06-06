#![feature(box_syntax)]
#![feature(test)]

extern crate test;

use peg::parser;
use std::{path::PathBuf, rc::Rc};
mod expr;
pub use expr::*;

enum Suffix {
	String(String),
	Slice(SliceDesc),
	Expression(LocExpr),
	Apply(expr::ArgsDesc),
	Extend(expr::ObjBody),
}
struct LocSuffix(Suffix, ExprLocation);

pub struct ParserSettings {
	pub loc_data: bool,
	pub file_name: PathBuf,
}

parser! {
	grammar jsonnet_parser() for str {
		use peg::ParseLiteral;

		/// Standard C-like comments
		rule comment()
			= "//" (!['\n'][_])* "\n"
			/ "/*" ((!("*/")[_][_])/("\\" "*/"))* "*/"
			/ "#" (!['\n'][_])* "\n"

		rule _() = ([' ' | '\n' | '\t'] / comment())*

		/// For comma-delimited elements
		rule comma() = quiet!{_ "," _} / expected!("<comma>")
		rule alpha() -> char = c:$(['_' | 'a'..='z' | 'A'..='Z']) {c.chars().next().unwrap()}
		rule digit() -> char = d:$(['0'..='9']) {d.chars().next().unwrap()}
		rule end_of_ident() = !['0'..='9' | '_' | 'a'..='z' | 'A'..='Z']
		/// Sequence of digits
		rule uint() -> u32 = a:$(digit()+) { a.parse().unwrap() }
		/// Number in scientific notation format
		rule number() -> f64 = quiet!{a:$(uint() ("." uint())? (['e'|'E'] (s:['+'|'-'])? uint())?) { a.parse().unwrap() }} / expected!("<number>")

		/// Reserved word followed by any non-alphanumberic
		rule reserved() = ("assert" / "else" / "error" / "false" / "for" / "function" / "if" / "import" / "importstr" / "in" / "local" / "null" / "tailstrict" / "then" / "self" / "super" / "true") end_of_ident()
		rule id() -> String = quiet!{ !reserved() s:$(alpha() (alpha() / digit())*) {s.to_owned()}} / expected!("<identifier>")

		rule keyword(id: &'static str)
			= ##parse_string_literal(id) end_of_ident()
		// Adds location data information to existing expression
		rule l(s: &ParserSettings, x: rule<Expr>) -> LocExpr
			= start:position!() v:x() end:position!() {loc_expr!(v, s.loc_data, (s.file_name.clone(), start, end))}

		pub rule param(s: &ParserSettings) -> expr::Param = name:id() expr:(_ "=" _ expr:expr(s){expr})? { expr::Param(name, expr) }
		pub rule params(s: &ParserSettings) -> expr::ParamsDesc
			= params:(param(s) ** comma()) {
				let mut defaults_started = false;
				for param in &params {
					defaults_started = defaults_started || param.1.is_some();
					assert_eq!(defaults_started, param.1.is_some(), "defauld parameters should be used after all positionals");
				}
				expr::ParamsDesc(params)
			}
			/ { expr::ParamsDesc(Vec::new()) }

		pub rule arg(s: &ParserSettings) -> expr::Arg
			= name:id() _ "=" _ expr:expr(s) {expr::Arg(Some(name), expr)}
			/ expr:expr(s) {expr::Arg(None, expr)}
		pub rule args(s: &ParserSettings) -> expr::ArgsDesc
			= args:arg(s) ** comma() comma()? {
				let mut named_started = false;
				for arg in &args {
					named_started = named_started || arg.0.is_some();
					assert_eq!(named_started, arg.0.is_some(), "named args should be used after all positionals");
				}
				expr::ArgsDesc(args)
			}
			/ { expr::ArgsDesc(Vec::new()) }

		pub rule bind(s: &ParserSettings) -> expr::BindSpec
			= name:id() _ "=" _ expr:expr(s) {expr::BindSpec{name, params: None, value: expr}}
			/ name:id() _ "(" _ params:params(s) _ ")" _ "=" _ expr:expr(s) {expr::BindSpec{name, params: Some(params), value: expr}}
		pub rule assertion(s: &ParserSettings) -> expr::AssertStmt
			= keyword("assert") _ cond:expr(s) msg:(_ ":" _ e:expr(s) {e})? { expr::AssertStmt(cond, msg) }
		pub rule string() -> String
			= v:("\"" str:$(("\\\"" / !['"'][_])*) "\"" {str.to_owned()}
			/ "'" str:$((!['\''][_])*) "'" {str.to_owned()}) {v.replace("\\n", "\n")}
		pub rule field_name(s: &ParserSettings) -> expr::FieldName
			= name:id() {expr::FieldName::Fixed(name)}
			/ name:string() {expr::FieldName::Fixed(name)}
			/ "[" _ expr:expr(s) _ "]" {expr::FieldName::Dyn(expr)}
		pub rule visibility() -> expr::Visibility
			= ":::" {expr::Visibility::Unhide}
			/ "::" {expr::Visibility::Hidden}
			/ ":" {expr::Visibility::Normal}
		pub rule field(s: &ParserSettings) -> expr::FieldMember
			= name:field_name(s) _ plus:"+"? _ visibility:visibility() _ value:expr(s) {expr::FieldMember{
				name,
				plus: plus.is_some(),
				params: None,
				visibility,
				value,
			}}
			/ name:field_name(s) _ "(" _ params:params(s) _ ")" _ visibility:visibility() _ value:expr(s) {expr::FieldMember{
				name,
				plus: false,
				params: Some(params),
				visibility,
				value,
			}}
		pub rule obj_local(s: &ParserSettings) -> BindSpec
			= keyword("local") _ bind:bind(s) {bind}
		pub rule member(s: &ParserSettings) -> expr::Member
			= bind:obj_local(s) {expr::Member::BindStmt(bind)}
			/ assertion:assertion(s) {expr::Member::AssertStmt(assertion)}
			/ field:field(s) {expr::Member::Field(field)}
		pub rule objinside(s: &ParserSettings) -> expr::ObjBody
			= pre_locals:(b: obj_local(s) comma() {b})* "[" _ key:expr(s) _ "]" _ ":" _ value:expr(s) post_locals:(comma() b:obj_local(s) {b})* _ forspec:forspec(s) others:(_ rest:compspec(s) {rest})? {
				expr::ObjBody::ObjComp {
					pre_locals,
					key,
					value,
					post_locals,
					rest: [vec![CompSpec::ForSpec(forspec)], others.unwrap_or_default()].concat(),
				}
			}
			/ members:(member(s) ** comma()) comma()? {expr::ObjBody::MemberList(members)}
		pub rule ifspec(s: &ParserSettings) -> IfSpecData
			= keyword("if") _ expr:expr(s) {IfSpecData(expr)}
		pub rule forspec(s: &ParserSettings) -> ForSpecData
			= keyword("for") _ id:id() _ keyword("in") _ cond:expr(s) {ForSpecData(id, cond)}
		pub rule compspec(s: &ParserSettings) -> Vec<expr::CompSpec>
			= s:(i:ifspec(s) { expr::CompSpec::IfSpec(i) } / f:forspec(s) {expr::CompSpec::ForSpec(f)} ) ** _ {s}
		pub rule local_expr(s: &ParserSettings) -> LocExpr
			= l(s,<keyword("local") _ binds:bind(s) ** comma() _ ";" _ expr:expr(s) { Expr::LocalExpr(binds, expr) }>)
		pub rule string_expr(s: &ParserSettings) -> LocExpr
			= l(s, <s:string() {Expr::Str(s)}>)
		pub rule obj_expr(s: &ParserSettings) -> LocExpr
			= l(s,<"{" _ body:objinside(s) _ "}" {Expr::Obj(body)}>)
		pub rule array_expr(s: &ParserSettings) -> LocExpr
			= l(s,<"[" _ elems:(expr(s) ** comma()) _ comma()? "]" {Expr::Arr(elems)}>)
		pub rule array_comp_expr(s: &ParserSettings) -> LocExpr
			= l(s,<"[" _ expr:expr(s) _ comma()? _ forspec:forspec(s) _ others:(others: compspec(s) _ {others})? "]" {Expr::ArrComp(expr, [vec![CompSpec::ForSpec(forspec)], others.unwrap_or_default()].concat())}>)
		pub rule number_expr(s: &ParserSettings) -> LocExpr
			= l(s,<n:number() { expr::Expr::Num(n) }>)
		pub rule var_expr(s: &ParserSettings) -> LocExpr
			= l(s,<n:id() { expr::Expr::Var(n) }>)
		pub rule if_then_else_expr(s: &ParserSettings) -> LocExpr
			= l(s,<cond:ifspec(s) _ keyword("then") _ cond_then:expr(s) cond_else:(_ keyword("else") _ e:expr(s) {e})? {Expr::IfElse{
				cond,
				cond_then,
				cond_else,
			}}>)

		pub rule literal(s: &ParserSettings) -> LocExpr
			= l(s,<v:(
				keyword("null") {LiteralType::Null}
				/ keyword("true") {LiteralType::True}
				/ keyword("false") {LiteralType::False}
				/ keyword("self") {LiteralType::This}
				/ keyword("$") {LiteralType::Dollar}
				/ keyword("super") {LiteralType::Super}
			) {Expr::Literal(v)}>)

		pub rule expr_basic(s: &ParserSettings) -> LocExpr
			= literal(s)

			/ string_expr(s) / number_expr(s)
			/ array_expr(s)
			/ obj_expr(s)
			/ array_expr(s)
			/ array_comp_expr(s)

			/ var_expr(s)
			/ local_expr(s)
			/ if_then_else_expr(s)

			/ l(s,<keyword("function") _ "(" _ params:params(s) _ ")" _ expr:expr(s) {Expr::Function(params, expr)}>)
			/ l(s,<assertion:assertion(s) _ ";" _ expr:expr(s) { Expr::AssertExpr(assertion, expr) }>)

			/ l(s,<keyword("error") _ expr:expr(s) { Expr::Error(expr) }>)

		rule expr_basic_with_suffix(s: &ParserSettings) -> LocExpr
			= a:expr_basic(s) suffixes:(_ suffix:l_expr_suffix(s) {suffix})* {
				let mut cur = a;
				for suffix in suffixes {
					let LocSuffix(suffix, location) = suffix;
					cur = LocExpr(Rc::new(match suffix {
						Suffix::String(index) => Expr::Index(cur, loc_expr!(Expr::Str(index), s.loc_data, (s.file_name.clone(), location.1, location.2))),
						Suffix::Slice(desc) => Expr::Slice(cur, desc),
						Suffix::Expression(index) => Expr::Index(cur, index),
						Suffix::Apply(args) => Expr::Apply(cur, args),
						Suffix::Extend(body) => Expr::ObjExtend(cur, body),
					}), if s.loc_data { Some(Rc::new(location)) } else { None })
				}
				cur
			}

		pub rule slice_desc(s: &ParserSettings) -> SliceDesc
			= start:expr(s)? _ ":" _ pair:(end:expr(s)? _ step:(":" _ e:expr(s) {e})? {(end, step)})? {
				if let Some((end, step)) = pair {
					SliceDesc { start, end, step }
				}else{
					SliceDesc { start, end: None, step: None }
				}
			}

		rule expr_suffix(s: &ParserSettings) -> Suffix
			= "." _ s:id() { Suffix::String(s) }
			/ "[" _ s:slice_desc(s) _ "]" { Suffix::Slice(s) }
			/ "[" _ s:expr(s) _ "]" { Suffix::Expression(s) }
			/ "(" _ args:args(s) _ ")" (_ keyword("tailstrict"))? { Suffix::Apply(args) }
			/ "{" _ body:objinside(s) _ "}" { Suffix::Extend(body) }
		rule l_expr_suffix(s: &ParserSettings) -> LocSuffix
			= start:position!() suffix:expr_suffix(s) end:position!() {LocSuffix(suffix, ExprLocation(s.file_name.clone(), start, end))}

		rule expr(s: &ParserSettings) -> LocExpr
			= start:position!() a:precedence! {
				a:(@) _ "||" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Or, b))}
				--
				a:(@) _ "&&" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::And, b))}
				--
				a:(@) _ "|" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::BitOr, b))}
				--
				a:@ _ "^" _ b:(@) {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::BitXor, b))}
				--
				a:(@) _ "&" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::BitAnd, b))}
				--
				a:(@) _ "==" _ b:@ {loc_expr_todo!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".to_owned())),
						el!(Expr::Str("equals".to_owned()))
					)), ArgsDesc(vec![Arg(None, a), Arg(None, b)])
				))}
				a:(@) _ "!=" _ b:@ {loc_expr_todo!(Expr::UnaryOp(UnaryOpType::Not, el!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".to_owned())),
						el!(Expr::Str("equals".to_owned()))
					)), ArgsDesc(vec![Arg(None, a), Arg(None, b)])
				))))}
				--
				a:(@) _ "<" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Lt, b))}
				a:(@) _ ">" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Gt, b))}
				a:(@) _ "<=" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Lte, b))}
				a:(@) _ ">=" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Gte, b))}
				--
				a:(@) _ "<<" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Lhs, b))}
				a:(@) _ ">>" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Rhs, b))}
				--
				a:(@) _ "+" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Add, b))}
				a:(@) _ "-" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Sub, b))}
				--
				a:(@) _ "*" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Mul, b))}
				a:(@) _ "/" _ b:@ {loc_expr_todo!(Expr::BinaryOp(a, BinaryOpType::Div, b))}
				a:(@) _ "%" _ b:@ {loc_expr_todo!(Expr::Apply(
					el!(Expr::Index(
						el!(Expr::Var("std".to_owned())),
						el!(Expr::Str("mod".to_owned()))
					)), ArgsDesc(vec![Arg(None, a), Arg(None, b)])
				))}
				--
						"-" _ b:@ {loc_expr_todo!(Expr::UnaryOp(UnaryOpType::Minus, b))}
						"!" _ b:@ {loc_expr_todo!(Expr::UnaryOp(UnaryOpType::Not, b))}
						"~" _ b:@ { loc_expr_todo!(Expr::UnaryOp(UnaryOpType::BitNot, b)) }
				--
				e:expr_basic_with_suffix(s) {e}
				"(" _ e:expr(s) _ ")" {loc_expr_todo!(Expr::Parened(e))}
			} end:position!() {
				let LocExpr(e, _) = a;
				LocExpr(e, if s.loc_data {
					Some(Rc::new(ExprLocation(s.file_name.to_owned(), start, end)))
				} else {
					None
				})
			}
			/ e:expr_basic_with_suffix(s) {e}

		pub rule jsonnet(s: &ParserSettings) -> LocExpr = _ e:expr(s) _ {e}
	}
}

pub fn parse(
	str: &str,
	settings: &ParserSettings,
) -> Result<LocExpr, peg::error::ParseError<peg::str::LineCol>> {
	jsonnet_parser::jsonnet(str, settings)
}

#[macro_export]
macro_rules! el {
	($expr:expr) => {
		LocExpr(std::rc::Rc::new($expr), None)
	};
}

#[cfg(test)]
pub mod tests {
	use super::{expr::*, parse};
	use crate::ParserSettings;
	use std::path::PathBuf;

	macro_rules! parse {
		($s:expr) => {
			parse(
				$s,
				&ParserSettings {
					loc_data: false,
					file_name: PathBuf::from("/test.jsonnet"),
					},
				)
			.unwrap()
		};
	}

	mod expressions {
		use super::*;

		pub fn basic_math() -> LocExpr {
			el!(Expr::BinaryOp(
				el!(Expr::Num(2.0)),
				BinaryOpType::Add,
				el!(Expr::BinaryOp(
					el!(Expr::Num(2.0)),
					BinaryOpType::Mul,
					el!(Expr::Num(2.0)),
				)),
			))
		}
	}

	#[test]
	fn empty_object() {
		assert_eq!(parse!("{}"), el!(Expr::Obj(ObjBody::MemberList(vec![]))));
	}

	#[test]
	fn basic_math() {
		assert_eq!(
			parse!("2+2*2"),
			el!(Expr::BinaryOp(
				el!(Expr::Num(2.0)),
				BinaryOpType::Add,
				el!(Expr::BinaryOp(
					el!(Expr::Num(2.0)),
					BinaryOpType::Mul,
					el!(Expr::Num(2.0))
				))
			))
		);
	}

	#[test]
	fn basic_math_with_indents() {
		assert_eq!(parse!("2	+ 	  2	  *	2   	"), expressions::basic_math());
	}

	#[test]
	fn basic_math_parened() {
		assert_eq!(
			parse!("2+(2+2*2)"),
			el!(Expr::BinaryOp(
				el!(Expr::Num(2.0)),
				BinaryOpType::Add,
				el!(Expr::Parened(expressions::basic_math())),
			))
		);
	}

	/// Comments should not affect parsing
	#[test]
	fn comments() {
		assert_eq!(
			parse!("2//comment\n+//comment\n3/*test*/*/*test*/4"),
			el!(Expr::BinaryOp(
				el!(Expr::Num(2.0)),
				BinaryOpType::Add,
				el!(Expr::BinaryOp(
					el!(Expr::Num(3.0)),
					BinaryOpType::Mul,
					el!(Expr::Num(4.0))
				))
			))
		);
	}

	/// Comments should be able to be escaped
	#[test]
	fn comment_escaping() {
		assert_eq!(
			parse!("2/*\\*/+*/ - 22"),
			el!(Expr::BinaryOp(
				el!(Expr::Num(2.0)),
				BinaryOpType::Sub,
				el!(Expr::Num(22.0))
			))
		);
	}

	#[test]
	fn array_comp() {
		use Expr::*;
		assert_eq!(
			parse!("[std.deepJoin(x) for x in arr]"),
			el!(ArrComp(
				el!(Apply(
					el!(Index(
						el!(Var("std".to_owned())),
						el!(Str("deepJoin".to_owned()))
					)),
					ArgsDesc(vec![Arg(None, el!(Var("x".to_owned())))])
				)),
				vec![CompSpec::ForSpec(ForSpecData(
					"x".to_owned(),
					el!(Var("arr".to_owned()))
				))]
			)),
		)
	}

	#[test]
	fn reserved() {
		use Expr::*;
		assert_eq!(parse!("null"), el!(Literal(LiteralType::Null)));
		assert_eq!(parse!("nulla"), el!(Var("nulla".to_owned())));
	}

	#[test]
	fn multiple_args_buf() {
		parse!("a(b, null_fields)");
	}

	#[test]
	fn infix_precedence() {
		use Expr::*;
		assert_eq!(
			parse!("!a && !b"),
			el!(BinaryOp(
				el!(UnaryOp(UnaryOpType::Not, el!(Var("a".to_owned())))),
				BinaryOpType::And,
				el!(UnaryOp(UnaryOpType::Not, el!(Var("b".to_owned()))))
			))
		);
	}

	#[test]
	fn infix_precedence_division() {
		use Expr::*;
		assert_eq!(
			parse!("!a / !b"),
			el!(BinaryOp(
				el!(UnaryOp(UnaryOpType::Not, el!(Var("a".to_owned())))),
				BinaryOpType::Div,
				el!(UnaryOp(UnaryOpType::Not, el!(Var("b".to_owned()))))
			))
		);
	}

	#[test]
	fn double_negation() {
		use Expr::*;
		assert_eq!(
			parse!("!!a"),
			el!(UnaryOp(
				UnaryOpType::Not,
				el!(UnaryOp(UnaryOpType::Not, el!(Var("a".to_owned()))))
			))
		)
	}

	#[test]
	fn array_test_error() {
		parse!("[a for a in b if c for e in f]");
		//                    ^^^^ failed code
	}

	#[test]
	fn can_parse_stdlib() {
		parse!(jsonnet_stdlib::STDLIB_STR);
	}

	use test::Bencher;

	// From source code
	#[bench]
	fn bench_parse_peg(b: &mut Bencher) {
		b.iter(|| parse!(jsonnet_stdlib::STDLIB_STR))
	}

	// From serialized blob
	#[bench]
	fn bench_parse_serde_bincode(b: &mut Bencher) {
		let serialized = bincode::serialize(&parse!(jsonnet_stdlib::STDLIB_STR)).unwrap();
		b.iter(|| bincode::deserialize::<LocExpr>(&serialized))
	}
}
