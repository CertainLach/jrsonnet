#![allow(clippy::redundant_closure_call)]

use peg::parser;
use std::{
	path::{Path, PathBuf},
	rc::Rc,
};
mod expr;
pub use expr::*;
pub use jrsonnet_interner::IStr;
pub use peg;

pub struct ParserSettings {
	pub loc_data: bool,
	pub file_name: Rc<Path>,
}

macro_rules! expr_bin {
	($a:ident $op:ident $b:ident) => {
		Expr::BinaryOp($a, $op, $b)
	};
}
macro_rules! expr_un {
	($op:ident $a:ident) => {
		Expr::UnaryOp($op, $a)
	};
}

parser! {
	grammar jsonnet_parser() for str {
		use peg::ParseLiteral;

		/// Standard C-like comments
		rule comment()
			= "//" (!['\n'][_])* "\n"
			/ "/*" ("\\*/" / "\\\\" / (!("*/")[_]))* "*/"
			/ "#" (!['\n'][_])* "\n"

		rule single_whitespace() = quiet!{([' ' | '\r' | '\n' | '\t'] / comment())} / expected!("<whitespace>")
		rule _() = single_whitespace()*

		/// For comma-delimited elements
		rule comma() = quiet!{_ "," _} / expected!("<comma>")
		rule alpha() -> char = c:$(['_' | 'a'..='z' | 'A'..='Z']) {c.chars().next().unwrap()}
		rule digit() -> char = d:$(['0'..='9']) {d.chars().next().unwrap()}
		rule end_of_ident() = !['0'..='9' | '_' | 'a'..='z' | 'A'..='Z']
		/// Sequence of digits
		rule uint_str() -> &'input str = a:$(digit()+) { a }
		/// Number in scientific notation format
		rule number() -> f64 = quiet!{a:$(uint_str() ("." uint_str())? (['e'|'E'] (s:['+'|'-'])? uint_str())?) {? a.parse().map_err(|_| "<number>") }} / expected!("<number>")

		/// Reserved word followed by any non-alphanumberic
		rule reserved() = ("assert" / "else" / "error" / "false" / "for" / "function" / "if" / "import" / "importstr" / "in" / "local" / "null" / "tailstrict" / "then" / "self" / "super" / "true") end_of_ident()
		rule id() = quiet!{ !reserved() alpha() (alpha() / digit())*} / expected!("<identifier>")

		rule keyword(id: &'static str) -> ()
			= ##parse_string_literal(id) end_of_ident()

		pub rule param(s: &ParserSettings) -> expr::Param = name:$(id()) expr:(_ "=" _ expr:expr(s){expr})? { expr::Param(name.into(), expr) }
		pub rule params(s: &ParserSettings) -> expr::ParamsDesc
			= params:param(s) ** comma() comma()? {
				let mut defaults_started = false;
				for param in &params {
					defaults_started = defaults_started || param.1.is_some();
					assert_eq!(defaults_started, param.1.is_some(), "defauld parameters should be used after all positionals");
				}
				expr::ParamsDesc(Rc::new(params))
			}
			/ { expr::ParamsDesc(Rc::new(Vec::new())) }

		pub rule arg(s: &ParserSettings) -> (Option<IStr>, LocExpr)
			= quiet! { name:(s:$(id()) _ "=" _ {s})? expr:expr(s) {(name.map(Into::into), expr)} }
			/ expected!("<argument>")

		pub rule args(s: &ParserSettings) -> expr::ArgsDesc
			= args:arg(s)**comma() comma()? {?
				let unnamed_count = args.iter().take_while(|(n, _)| n.is_none()).count();
				let mut unnamed = Vec::with_capacity(unnamed_count);
				let mut named = Vec::with_capacity(args.len() - unnamed_count);
				let mut named_started = false;
				for (name, value) in args {
					if let Some(name) = name {
						named_started = true;
						named.push((name, value));
					} else {
						if named_started {
							return Err("<named argument>")
						}
						unnamed.push(value);
					}
				}
				Ok(expr::ArgsDesc::new(unnamed, named))
			}

		pub rule bind(s: &ParserSettings) -> expr::BindSpec
			= name:$(id()) _ "=" _ expr:expr(s) {expr::BindSpec{name:name.into(), params: None, value: expr}}
			/ name:$(id()) _ "(" _ params:params(s) _ ")" _ "=" _ expr:expr(s) {expr::BindSpec{name:name.into(), params: Some(params), value: expr}}
		pub rule assertion(s: &ParserSettings) -> expr::AssertStmt
			= keyword("assert") _ cond:expr(s) msg:(_ ":" _ e:expr(s) {e})? { expr::AssertStmt(cond, msg) }

		pub rule whole_line() -> &'input str
			= str:$((!['\n'][_])* "\n") {str}
		pub rule string_block() -> String
			= "|||" (!['\n']single_whitespace())* "\n"
			  empty_lines:$(['\n']*)
			  prefix:[' ' | '\t']+ first_line:whole_line()
			  lines:("\n" {"\n"} / [' ' | '\t']*<{prefix.len()}> s:whole_line() {s})*
			  [' ' | '\t']*<, {prefix.len() - 1}> "|||"
			  {let mut l = empty_lines.to_owned(); l.push_str(first_line); l.extend(lines); l}
		pub rule string() -> String
			= quiet!{ "\"" str:$(("\\\"" / "\\\\" / (!['"'][_]))*) "\"" {unescape::unescape(str).unwrap()}
			/ "'" str:$(("\\'" / "\\\\" / (!['\''][_]))*) "'" {unescape::unescape(str).unwrap()}
			/ "@'" str:$(("''" / (!['\''][_]))*) "'" {str.replace("''", "'")}
			/ "@\"" str:$(("\"\"" / (!['"'][_]))*) "\"" {str.replace("\"\"", "\"")}
			/ string_block() } / expected!("<string>")

		pub rule field_name(s: &ParserSettings) -> expr::FieldName
			= name:$(id()) {expr::FieldName::Fixed(name.into())}
			/ name:string() {expr::FieldName::Fixed(name.into())}
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
			= pre_locals:(b: obj_local(s) comma() {b})* "[" _ key:expr(s) _ "]" _ plus:"+"? _ ":" _ value:expr(s) post_locals:(comma() b:obj_local(s) {b})* _ forspec:forspec(s) others:(_ rest:compspec(s) {rest})? {
				let mut compspecs = vec![CompSpec::ForSpec(forspec)];
				compspecs.extend(others.unwrap_or_default());
				expr::ObjBody::ObjComp(expr::ObjComp{
					pre_locals,
					key,
					plus: plus.is_some(),
					value,
					post_locals,
					compspecs,
				})
			}
			/ members:(member(s) ** comma()) comma()? {expr::ObjBody::MemberList(members)}
		pub rule ifspec(s: &ParserSettings) -> IfSpecData
			= keyword("if") _ expr:expr(s) {IfSpecData(expr)}
		pub rule forspec(s: &ParserSettings) -> ForSpecData
			= keyword("for") _ id:$(id()) _ keyword("in") _ cond:expr(s) {ForSpecData(id.into(), cond)}
		pub rule compspec(s: &ParserSettings) -> Vec<expr::CompSpec>
			= s:(i:ifspec(s) { expr::CompSpec::IfSpec(i) } / f:forspec(s) {expr::CompSpec::ForSpec(f)} ) ** _ {s}
		pub rule local_expr(s: &ParserSettings) -> Expr
			= keyword("local") _ binds:bind(s) ** comma() _ ";" _ expr:expr(s) { Expr::LocalExpr(binds, expr) }
		pub rule string_expr(s: &ParserSettings) -> Expr
			= s:string() {Expr::Str(s.into())}
		pub rule obj_expr(s: &ParserSettings) -> Expr
			= "{" _ body:objinside(s) _ "}" {Expr::Obj(body)}
		pub rule array_expr(s: &ParserSettings) -> Expr
			= "[" _ elems:(expr(s) ** comma()) _ comma()? "]" {Expr::Arr(elems)}
		pub rule array_comp_expr(s: &ParserSettings) -> Expr
			= "[" _ expr:expr(s) _ comma()? _ forspec:forspec(s) _ others:(others: compspec(s) _ {others})? "]" {
				let mut specs = vec![CompSpec::ForSpec(forspec)];
				specs.extend(others.unwrap_or_default());
				Expr::ArrComp(expr, specs)
			}
		pub rule number_expr(s: &ParserSettings) -> Expr
			= n:number() { expr::Expr::Num(n) }
		pub rule var_expr(s: &ParserSettings) -> Expr
			= n:$(id()) { expr::Expr::Var(n.into()) }
		pub rule if_then_else_expr(s: &ParserSettings) -> Expr
			= cond:ifspec(s) _ keyword("then") _ cond_then:expr(s) cond_else:(_ keyword("else") _ e:expr(s) {e})? {Expr::IfElse{
				cond,
				cond_then,
				cond_else,
			}}

		pub rule literal(s: &ParserSettings) -> Expr
			= v:(
				keyword("null") {LiteralType::Null}
				/ keyword("true") {LiteralType::True}
				/ keyword("false") {LiteralType::False}
				/ keyword("self") {LiteralType::This}
				/ keyword("$") {LiteralType::Dollar}
				/ keyword("super") {LiteralType::Super}
			) {Expr::Literal(v)}

		pub rule expr_basic(s: &ParserSettings) -> Expr
			= literal(s)

			/ quiet!{"$intrinsic(" name:$(id()) ")" {Expr::Intrinsic(name.into())}}

			/ string_expr(s) / number_expr(s)
			/ array_expr(s)
			/ obj_expr(s)
			/ array_expr(s)
			/ array_comp_expr(s)

			/ keyword("importstr") _ path:string() {Expr::ImportStr(PathBuf::from(path))}
			/ keyword("import") _ path:string() {Expr::Import(PathBuf::from(path))}

			/ var_expr(s)
			/ local_expr(s)
			/ if_then_else_expr(s)

			/ keyword("function") _ "(" _ params:params(s) _ ")" _ expr:expr(s) {Expr::Function(params, expr)}
			/ assertion:assertion(s) _ ";" _ expr:expr(s) { Expr::AssertExpr(assertion, expr) }

			/ keyword("error") _ expr:expr(s) { Expr::ErrorStmt(expr) }

		rule slice_part(s: &ParserSettings) -> Option<LocExpr>
			= e:(_ e:expr(s) _{e})? {e}
		pub rule slice_desc(s: &ParserSettings) -> SliceDesc
			= start:slice_part(s) ":" pair:(end:slice_part(s) step:(":" e:slice_part(s){e})? {(end, step.flatten())})? {
				let (end, step) = if let Some((end, step)) = pair {
					(end, step)
				}else{
					(None, None)
				};

				SliceDesc { start, end, step }
			}

		rule binop(x: rule<()>) -> ()
			= quiet!{ x() } / expected!("<binary op>")
		rule unaryop(x: rule<()>) -> ()
			= quiet!{ x() } / expected!("<unary op>")


		use BinaryOpType::*;
		use UnaryOpType::*;
		rule expr(s: &ParserSettings) -> LocExpr
			= precedence! {
				start:position!() v:@ end:position!() { loc_expr!(v, s.loc_data, (s.file_name.clone(), start, end)) }
				--
				a:(@) _ binop(<"||">) _ b:@ {expr_bin!(a Or b)}
				--
				a:(@) _ binop(<"&&">) _ b:@ {expr_bin!(a And b)}
				--
				a:(@) _ binop(<"|">) _ b:@ {expr_bin!(a BitOr b)}
				--
				a:@ _ binop(<"^">) _ b:(@) {expr_bin!(a BitXor b)}
				--
				a:(@) _ binop(<"&">) _ b:@ {expr_bin!(a BitAnd b)}
				--
				a:(@) _ binop(<"==">) _ b:@ {expr_bin!(a Eq b)}
				a:(@) _ binop(<"!=">) _ b:@ {expr_bin!(a Neq b)}
				--
				a:(@) _ binop(<"<">) _ b:@ {expr_bin!(a Lt b)}
				a:(@) _ binop(<">">) _ b:@ {expr_bin!(a Gt b)}
				a:(@) _ binop(<"<=">) _ b:@ {expr_bin!(a Lte b)}
				a:(@) _ binop(<">=">) _ b:@ {expr_bin!(a Gte b)}
				a:(@) _ binop(<keyword("in")>) _ b:@ {expr_bin!(a In b)}
				--
				a:(@) _ binop(<"<<">) _ b:@ {expr_bin!(a Lhs b)}
				a:(@) _ binop(<">>">) _ b:@ {expr_bin!(a Rhs b)}
				--
				a:(@) _ binop(<"+">) _ b:@ {expr_bin!(a Add b)}
				a:(@) _ binop(<"-">) _ b:@ {expr_bin!(a Sub b)}
				--
				a:(@) _ binop(<"*">) _ b:@ {expr_bin!(a Mul b)}
				a:(@) _ binop(<"/">) _ b:@ {expr_bin!(a Div b)}
				a:(@) _ binop(<"%">) _ b:@ {expr_bin!(a Mod b)}
				--
						unaryop(<"-">) _ b:@ {expr_un!(Minus b)}
						unaryop(<"!">) _ b:@ {expr_un!(Not b)}
						unaryop(<"~">) _ b:@ {expr_un!(BitNot b)}
				--
				a:(@) _ "[" _ e:slice_desc(s) _ "]" {Expr::Slice(a, e)}
				a:(@) _ "." _ e:$(id()) {Expr::Index(a, el!(Expr::Str(e.into())))}
				a:(@) _ "[" _ e:expr(s) _ "]" {Expr::Index(a, e)}
				a:(@) _ "(" _ args:args(s) _ ")" ts:(_ keyword("tailstrict"))? {Expr::Apply(a, args, ts.is_some())}
				a:(@) _ "{" _ body:objinside(s) _ "}" {Expr::ObjExtend(a, body)}
				--
				e:expr_basic(s) {e}
				"(" _ e:expr(s) _ ")" {Expr::Parened(e)}
			}

		pub rule jsonnet(s: &ParserSettings) -> LocExpr = _ e:expr(s) _ {e}
	}
}

pub type ParseError = peg::error::ParseError<peg::str::LineCol>;
pub fn parse(str: &str, settings: &ParserSettings) -> Result<LocExpr, ParseError> {
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
	use BinaryOpType::*;

	macro_rules! parse {
		($s:expr) => {
			parse(
				$s,
				&ParserSettings {
					loc_data: false,
					file_name: PathBuf::from("/test.jsonnet").into(),
				},
			)
			.unwrap()
		};
	}

	macro_rules! el_loc {
		($expr:expr, $loc:expr$(,)?) => {
			LocExpr(std::rc::Rc::new($expr), Some($loc))
		};
	}

	mod expressions {
		use super::*;

		pub fn basic_math() -> LocExpr {
			el!(Expr::BinaryOp(
				el!(Expr::Num(2.0)),
				Add,
				el!(Expr::BinaryOp(
					el!(Expr::Num(2.0)),
					Mul,
					el!(Expr::Num(2.0)),
				)),
			))
		}
	}

	#[test]
	fn multiline_string() {
		assert_eq!(
			parse!("|||\n    Hello world!\n     a\n|||"),
			el!(Expr::Str("Hello world!\n a\n".into())),
		);
		assert_eq!(
			parse!("|||\n  Hello world!\n   a\n|||"),
			el!(Expr::Str("Hello world!\n a\n".into())),
		);
		assert_eq!(
			parse!("|||\n\t\tHello world!\n\t\t\ta\n|||"),
			el!(Expr::Str("Hello world!\n\ta\n".into())),
		);
		assert_eq!(
			parse!("|||\n   Hello world!\n    a\n |||"),
			el!(Expr::Str("Hello world!\n a\n".into())),
		);
	}

	#[test]
	fn slice() {
		parse!("a[1:]");
		parse!("a[1::]");
		parse!("a[:1:]");
		parse!("a[::1]");
		parse!("str[:len - 1]");
	}

	#[test]
	fn string_escaping() {
		assert_eq!(
			parse!(r#""Hello, \"world\"!""#),
			el!(Expr::Str(r#"Hello, "world"!"#.into())),
		);
		assert_eq!(
			parse!(r#"'Hello \'world\'!'"#),
			el!(Expr::Str("Hello 'world'!".into())),
		);
		assert_eq!(parse!(r#"'\\\\'"#), el!(Expr::Str("\\\\".into())),);
	}

	#[test]
	fn string_unescaping() {
		assert_eq!(
			parse!(r#""Hello\nWorld""#),
			el!(Expr::Str("Hello\nWorld".into())),
		);
	}

	#[test]
	fn string_verbantim() {
		assert_eq!(
			parse!(r#"@"Hello\n""World""""#),
			el!(Expr::Str("Hello\\n\"World\"".into())),
		);
	}

	#[test]
	fn imports() {
		assert_eq!(
			parse!("import \"hello\""),
			el!(Expr::Import(PathBuf::from("hello"))),
		);
		assert_eq!(
			parse!("importstr \"garnish.txt\""),
			el!(Expr::ImportStr(PathBuf::from("garnish.txt")))
		);
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
				Add,
				el!(Expr::BinaryOp(
					el!(Expr::Num(2.0)),
					Mul,
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
				Add,
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
				Add,
				el!(Expr::BinaryOp(
					el!(Expr::Num(3.0)),
					Mul,
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
				Sub,
				el!(Expr::Num(22.0))
			))
		);
	}

	#[test]
	fn suffix() {
		// assert_eq!(parse!("std.test"), el!(Expr::Num(2.2)));
		// assert_eq!(parse!("std(2)"), el!(Expr::Num(2.2)));
		// assert_eq!(parse!("std.test(2)"), el!(Expr::Num(2.2)));
		// assert_eq!(parse!("a[b]"), el!(Expr::Num(2.2)))
	}

	#[test]
	fn array_comp() {
		use Expr::*;
		assert_eq!(
			parse!("[std.deepJoin(x) for x in arr]"),
			el!(ArrComp(
				el!(Apply(
					el!(Index(el!(Var("std".into())), el!(Str("deepJoin".into())))),
					ArgsDesc::new(vec![el!(Var("x".into()))], vec![]),
					false,
				)),
				vec![CompSpec::ForSpec(ForSpecData(
					"x".into(),
					el!(Var("arr".into()))
				))]
			)),
		)
	}

	#[test]
	fn reserved() {
		use Expr::*;
		assert_eq!(parse!("null"), el!(Literal(LiteralType::Null)));
		assert_eq!(parse!("nulla"), el!(Var("nulla".into())));
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
				el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into())))),
				And,
				el!(UnaryOp(UnaryOpType::Not, el!(Var("b".into()))))
			))
		);
	}

	#[test]
	fn infix_precedence_division() {
		use Expr::*;
		assert_eq!(
			parse!("!a / !b"),
			el!(BinaryOp(
				el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into())))),
				Div,
				el!(UnaryOp(UnaryOpType::Not, el!(Var("b".into()))))
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
				el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()))))
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
		parse!(jrsonnet_stdlib::STDLIB_STR);
	}

	#[test]
	fn add_location_info_to_all_sub_expressions() {
		use Expr::*;

		let file_name: std::rc::Rc<std::path::Path> = PathBuf::from("/test.jsonnet").into();
		let expr = parse(
			"{} { local x = 1, x: x } + {}",
			&ParserSettings {
				loc_data: true,
				file_name: file_name.clone(),
			},
		)
		.unwrap();
		assert_eq!(
			expr,
			el_loc!(
				BinaryOp(
					el_loc!(
						ObjExtend(
							el_loc!(
								Obj(ObjBody::MemberList(vec![])),
								ExprLocation(file_name.clone(), 0, 2)
							),
							ObjBody::MemberList(vec![
								Member::BindStmt(BindSpec {
									name: "x".into(),
									params: None,
									value: el_loc!(
										Num(1.0),
										ExprLocation(file_name.clone(), 15, 16)
									)
								}),
								Member::Field(FieldMember {
									name: FieldName::Fixed("x".into()),
									plus: false,
									params: None,
									visibility: Visibility::Normal,
									value: el_loc!(
										Var("x".into()),
										ExprLocation(file_name.clone(), 21, 22)
									),
								})
							])
						),
						ExprLocation(file_name.clone(), 0, 24)
					),
					BinaryOpType::Add,
					el_loc!(
						Obj(ObjBody::MemberList(vec![])),
						ExprLocation(file_name.clone(), 27, 29)
					),
				),
				ExprLocation(file_name.clone(), 0, 29),
			),
		);
	}
	// From source code
	/*
	#[bench]
	fn bench_parse_peg(b: &mut Bencher) {
		b.iter(|| parse!(jrsonnet_stdlib::STDLIB_STR))
	}
	*/
}
