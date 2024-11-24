#![allow(clippy::redundant_closure_call, clippy::derive_partial_eq_without_eq)]

use std::rc::Rc;

use peg::parser;
mod expr;
pub use expr::*;
pub use jrsonnet_interner::IStr;
pub use peg;
mod location;
mod source;
mod unescape;
pub use location::CodeLocation;
pub use source::{
	Source, SourceDirectory, SourceFifo, SourceFile, SourcePath, SourcePathT, SourceVirtual,
};

fn expr_bin(a: Expr, op: BinaryOpType, b: Expr) -> Expr {
	Expr::BinaryOp {
		op,
		ab: Box::new((a, b)),
	}
}

fn expr_un(op: UnaryOpType, v: Expr) -> Expr {
	Expr::UnaryOp(op, Box::new(v))
}

parser! {
	grammar jsonnet_parser() for str {
		use peg::ParseLiteral;

		rule eof() = quiet!{![_]} / expected!("<eof>")
		rule eol() = "\n" / eof()

		/// Standard C-like comments
		rule comment()
			= "//" (!eol()[_])* eol()
			/ "/*" (!("*/")[_])* "*/"
			/ "#" (!eol()[_])* eol()

		rule single_whitespace() = quiet!{([' ' | '\r' | '\n' | '\t'] / comment())} / expected!("<whitespace>")
		rule _() = quiet!{([' ' | '\r' | '\n' | '\t']+) / comment()}* / expected!("<whitespace>")

		/// For comma-delimited elements
		rule comma() = quiet!{_ "," _} / expected!("<comma>")
		rule alpha() -> char = c:$(['_' | 'a'..='z' | 'A'..='Z']) {c.chars().next().unwrap()}
		rule digit() -> char = d:$(['0'..='9']) {d.chars().next().unwrap()}
		rule end_of_ident() = !['0'..='9' | '_' | 'a'..='z' | 'A'..='Z']
		/// Sequence of digits
		rule uint_str() -> &'input str = a:$(digit()+) { a }
		/// Number in scientific notation format
		rule number() -> f64 = quiet!{a:$(("-" / "+")? uint_str() ("." uint_str())? (['e'|'E'] (s:['+'|'-'])? uint_str())?) {? a.parse().map_err(|_| "<number>") }} / expected!("<number>")

		/// Reserved word followed by any non-alphanumberic
		rule reserved() = ("assert" / "else" / "error" / "false" / "for" / "function" / "if" / "import" / "importstr" / "importbin" / "in" / "local" / "null" / "tailstrict" / "then" / "self" / "super" / "true") end_of_ident()
		rule id() -> IStr = v:$(quiet!{ !reserved() alpha() (alpha() / digit())*} / expected!("<identifier>")) { v.into() }

		rule keyword(id: &'static str) -> ()
			= ##parse_string_literal(id) end_of_ident()

		pub rule param() -> expr::Param = name:destruct() expr:(_ "=" _ expr:spanned(<expr()>){expr})? { expr::Param(name, expr) }
		pub rule params() -> expr::ParamsDesc
			= params:param() ** comma() comma()? { expr::ParamsDesc(rc_vec(params)) }
			/ { expr::ParamsDesc(rc_vec(Vec::new())) }

		pub rule arg() -> (Option<IStr>, SpannedExpr)
			= name:(quiet! { (s:id() _ "=" !['='] _ {s})? } / expected!("<argument name>")) expr:spanned(<expr()>) {(name, expr)}

		pub rule args() -> expr::ArgsDesc
			= args:arg()**comma() comma()? {?
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

		pub rule destruct_rest() -> expr::DestructRest
			= "..." into:(_ into:id() {into})? {if let Some(into) = into {
				expr::DestructRest::Keep(into)
			} else {expr::DestructRest::Drop}}
		pub rule destruct_array() -> expr::Destruct
			= "[" _ start:destruct()**comma() rest:(
				comma() _ rest:destruct_rest()? end:(
					comma() end:destruct()**comma() (_ comma())? {end}
					/ comma()? {Vec::new()}
				) {(rest, end)}
				/ comma()? {(None, Vec::new())}
			) _ "]" {?
				#[cfg(feature = "exp-destruct")] return Ok(expr::Destruct::Array {
					start,
					rest: rest.0,
					end: rest.1,
				});
				#[cfg(not(feature = "exp-destruct"))] Err("!!!experimental destructuring was not enabled")
			}
		pub rule destruct_object() -> expr::Destruct
			= "{" _
				fields:(name:id() into:(_ ":" _ into:destruct() {into})? default:(_ "=" _ v:expr() {v})? {(name, into, default)})**comma()
				rest:(
					comma() rest:destruct_rest()? {rest}
					/ comma()? {None}
				)
			_ "}" {?
				#[cfg(feature = "exp-destruct")] return Ok(expr::Destruct::Object {
					fields,
					rest,
				});
				#[cfg(not(feature = "exp-destruct"))] Err("!!!experimental destructuring was not enabled")
			}
		pub rule destruct() -> expr::Destruct
			= v:id() {expr::Destruct::Full(v)}
			/ "?" {?
				#[cfg(feature = "exp-destruct")] return Ok(expr::Destruct::Skip);
				#[cfg(not(feature = "exp-destruct"))] Err("!!!experimental destructuring was not enabled")
			}
			/ arr:destruct_array() {arr}
			/ obj:destruct_object() {obj}

		pub rule bind() -> expr::BindSpec
			= into:destruct() _ params:(beg:spanned(<"(">) _ params:params() _ ")" _ {(beg, params)})? "=" _ value:spanned(<expr()>) {if let Some((beg, params)) = params {
				let fn_span = Span::encompassing(beg.span(), value.span());
				expr::BindSpec { into, value: Spanned(Expr::Function(params, Rc::new(value)), fn_span) }
			} else {
				expr::BindSpec { into, value }
			}}

		pub rule assertion() -> expr::AssertStmt
			= keyword("assert") _ cond:spanned(<expr()>) msg:(_ ":" _ e:expr() {e})? { expr::AssertStmt(cond, msg) }

		pub rule whole_line() -> &'input str
			= str:$((!['\n'][_])* "\n") {str}
		pub rule string_block() -> String
			= "|||" (!['\n']single_whitespace())* "\n"
			  empty_lines:$(['\n']*)
			  prefix:[' ' | '\t']+ first_line:whole_line()
			  lines:("\n" {"\n"} / [' ' | '\t']*<{prefix.len()}> s:whole_line() {s})*
			  [' ' | '\t']*<, {prefix.len() - 1}> "|||"
			  {let mut l = empty_lines.to_owned(); l.push_str(first_line); l.extend(lines); l}

		rule hex_char()
			= quiet! { ['0'..='9' | 'a'..='f' | 'A'..='F'] } / expected!("<hex char>")

		rule string_char(c: rule<()>)
			= (!['\\']!c()[_])+
			/ "\\\\"
			/ "\\u" hex_char() hex_char() hex_char() hex_char()
			/ "\\x" hex_char() hex_char()
			/ ['\\'] (quiet! { ['b' | 'f' | 'n' | 'r' | 't' | '"' | '\''] } / expected!("<escape character>"))
		pub rule string() -> String
			= ['"'] str:$(string_char(<"\"">)*) ['"'] {? unescape::unescape(str).ok_or("<escaped string>")}
			/ ['\''] str:$(string_char(<"\'">)*) ['\''] {? unescape::unescape(str).ok_or("<escaped string>")}
			/ quiet!{ "@'" str:$(("''" / (!['\''][_]))*) "'" {str.replace("''", "'")}
			/ "@\"" str:$(("\"\"" / (!['"'][_]))*) "\"" {str.replace("\"\"", "\"")}
			/ string_block() } / expected!("<string>")

		pub rule field_name() -> expr::FieldName
			= name:id() {expr::FieldName::Fixed(name)}
			/ name:string() {expr::FieldName::Fixed(name.into())}
			/ "[" _ expr:spanned(<expr()>) _ "]" {expr::FieldName::Dyn(expr)}
		pub rule visibility() -> expr::Visibility
			= ":::" {expr::Visibility::Unhide}
			/ "::" {expr::Visibility::Hidden}
			/ ":" {expr::Visibility::Normal}
		pub rule field() -> expr::FieldMember
			= name:field_name() _ plus:"+"? _ visibility:visibility() _ value:spanned(<expr()>) {expr::FieldMember{
				name,
				plus: plus.is_some(),
				params: None,
				visibility,
				value: Rc::new(value),
			}}
			/ name:field_name() _ "(" _ params:params() _ ")" _ visibility:visibility() _ value:spanned(<expr()>) {expr::FieldMember{
				name,
				plus: false,
				params: Some(params),
				visibility,
				value: Rc::new(value),
			}}
		pub rule obj_local() -> BindSpec
			= keyword("local") _ bind:bind() {bind}
		pub rule member() -> expr::Member
			= bind:obj_local() {expr::Member::BindStmt(bind)}
			/ assertion:assertion() {expr::Member::AssertStmt(assertion)}
			/ field:field() {expr::Member::Field(field)}

		pub rule objinside() -> expr::ObjInner
			= members:(member() ** comma()) comma()? _ specs:compspec()? {?
				ObjUnknown::new(members, specs).classify()
			}
		pub rule ifspec() -> IfSpecData
			= keyword("if") _ expr:spanned(<expr()>) {IfSpecData(expr)}
		pub rule forspec() -> ForSpecData
			= keyword("for") _ id:destruct() _ keyword("in") _ cond:spanned(<expr()>) {ForSpecData(id, cond)}
		pub rule compspec() -> Vec<expr::CompSpec>
			= &keyword("for") s:(i:ifspec() { expr::CompSpec::IfSpec(i) } / f:forspec() {expr::CompSpec::ForSpec(f)} ) ** _ {s}

		pub rule local_expr() -> Expr
			= keyword("local") _ binds:bind() ** comma() (_ ",")? _ ";" _ expr:expr() {
				// TODO: Non-Rc based BindSpecs?
				Expr::LocalExpr(rc_vec(binds), Box::new(expr))
			}
		pub rule string_expr() -> Expr
			= s:string() {Expr::Str(s.into())}
		pub rule obj_expr() -> Expr
			= "{" _ body:objinside() _ "}" {match body {
				ObjInner::Members(members) => Expr::ObjMembers(Box::new(members)),
				ObjInner::Comp(members) => Expr::ObjComp(Box::new(members)),
			}}
		pub rule array_expr() -> Expr
			= "[" _ elems:(expr() ** comma()) comma()? _ specs:compspec()? _ "]" {? ArrUnknown::new(elems, specs).classify()}
		pub rule number_expr() -> Expr
			= n:number() { expr::Expr::Num(n) }
		pub rule var_expr() -> Expr
			= n:spanned(<id()>) { expr::Expr::Var(n) }
		pub rule if_then_else_expr() -> Expr
			= condition:ifspec() _
				keyword("then") _ then:spanned(<expr()>)
				else_:(_ keyword("else") _ e:spanned(<expr()>) {e})?
			{Expr::IfElse(Box::new(IfElseBody{
				condition,
				then,
				else_,
			}))}

		pub rule literal() -> Expr
			= v:(
				keyword("null") {LiteralType::Null}
				/ keyword("true") {LiteralType::True}
				/ keyword("false") {LiteralType::False}
				/ keyword("self") {LiteralType::This}
				/ keyword("$") {LiteralType::Dollar}
				/ keyword("super") {LiteralType::Super}
			) {Expr::Literal(v)}

		pub rule import_kind() -> ImportKind
			= keyword("importstr") {ImportKind::String}
			/ keyword("importbin") {ImportKind::Binary}
			/ keyword("import") {ImportKind::Normal}

		pub rule expr_basic() -> Expr
			= literal()

			/ string_expr() / number_expr()
			/ array_expr()
			/ obj_expr()
			/ array_expr()

			/ v:spanned(<k:import_kind() _ path:expr() {(k, path)}>) {Expr::Import(Box::new(v))}

			/ var_expr()
			/ local_expr()
			/ if_then_else_expr()

			/ keyword("function") _ "(" _ params:params() _ ")" _ expr:spanned(<expr()>) {Expr::Function(params, Rc::new(expr))}
			/ assertion:assertion() _ ";" _ expr:expr() { Expr::AssertExpr(Box::new((assertion, expr))) }

			/ keyword("error") _ expr:spanned(<expr()>) { Expr::ErrorStmt(Box::new(expr)) }

		rule slice_part() -> Option<SpannedExpr>
			= _ e:(e:spanned(<expr()>) _{e})? {e}
		pub rule slice_desc() -> SliceDesc
			= start:slice_part() ":" pair:(end:slice_part() step:(":" e:slice_part(){e})? {(end, step.flatten())})? {
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
		rule spanned<T: jrsonnet_gcmodule::Trace>(x: rule<T>) -> Spanned<T>
			= start:position!() v:x() end:position!() { Spanned(v, Span(current_source(), start as u32, end as u32)) }

		rule ensure_null_coaelse()
			= "" {?
				#[cfg(not(feature = "exp-null-coaelse"))] return Err("!!!experimental null coaelscing was not enabled");
				#[cfg(feature = "exp-null-coaelse")] Ok(())
			}

		use BinaryOpType::*;
		use UnaryOpType::*;
		rule expr() -> Expr
			= precedence! {
				a:(@) _ binop(<"||">) _ b:@ {expr_bin(a, Or, b)}
				a:(@) _ binop(<"??">) _ ensure_null_coaelse() b:@ {
					#[cfg(feature = "exp-null-coaelse")] return expr_bin!(a NullCoaelse b);
					unreachable!("ensure_null_coaelse will fail if feature is not enabled")
				}
				--
				a:(@) _ binop(<"&&">) _ b:@ {expr_bin(a, And, b)}
				--
				a:(@) _ binop(<"|">) _ b:@ {expr_bin(a, BitOr, b)}
				--
				a:@ _ binop(<"^">) _ b:(@) {expr_bin(a, BitXor, b)}
				--
				a:(@) _ binop(<"&">) _ b:@ {expr_bin(a, BitAnd, b)}
				--
				a:(@) _ binop(<"==">) _ b:@ {expr_bin(a, Eq, b)}
				a:(@) _ binop(<"!=">) _ b:@ {expr_bin(a, Neq, b)}
				--
				a:(@) _ binop(<"<">) _ b:@ {expr_bin(a, Lt, b)}
				a:(@) _ binop(<">">) _ b:@ {expr_bin(a, Gt, b)}
				a:(@) _ binop(<"<=">) _ b:@ {expr_bin(a, Lte, b)}
				a:(@) _ binop(<">=">) _ b:@ {expr_bin(a, Gte, b)}
				a:(@) _ binop(<keyword("in")>) _ b:@ {expr_bin(a, In, b)}
				--
				a:(@) _ binop(<"<<">) _ b:@ {expr_bin(a, Lhs, b)}
				a:(@) _ binop(<">>">) _ b:@ {expr_bin(a, Rhs, b)}
				--
				a:(@) _ binop(<"+">) _ b:@ {expr_bin(a, Add, b)}
				a:(@) _ binop(<"-">) _ b:@ {expr_bin(a, Sub, b)}
				--
				a:(@) _ binop(<"*">) _ b:@ {expr_bin(a, Mul, b)}
				a:(@) _ binop(<"/">) _ b:@ {expr_bin(a, Div, b)}
				a:(@) _ binop(<"%">) _ b:@ {expr_bin(a, Mod, b)}
				--
						unaryop(<"+">) _ b:@ {expr_un(Plus, b)}
						unaryop(<"-">) _ b:@ {expr_un(Minus, b)}
						unaryop(<"!">) _ b:@ {expr_un(Not, b)}
						unaryop(<"~">) _ b:@ {expr_un(BitNot, b)}
				--
				a:(@) _ "[" _ e:slice_desc() _ "]" {Expr::Slice(Box::new((a, e)))}
				indexable:(@) _ parts:index_part()+ {Expr::Index{indexable: Box::new(indexable), parts}}
				a:(@) _ "(" _ args:spanned(<args()>) _ ")" ts:(_ keyword("tailstrict"))? {Expr::Apply(Box::new(ApplyBody {lhs: a, args, tailstrict: ts.is_some()}))}
				a:(@) _ "{" _ body:objinside() _ "}" {match body {
					ObjInner::Members(body) => Expr::ObjExtendMembers(Box::new((a, body))),
					ObjInner::Comp(body) => Expr::ObjExtendComp(Box::new((a, body))),
				}}
				--
				e:expr_basic() {e}
				"(" _ e:expr() _ ")" {e}
			}
		pub rule index_part() -> IndexPart
		= n:("?" _ ensure_null_coaelse())? "." _ value:spanned(<v:id() {Expr::Str(v)}>) {IndexPart {
			value,
			#[cfg(feature = "exp-null-coaelse")]
			null_coaelse: n.is_some(),
		}}
		/ n:("?" _ "." _ ensure_null_coaelse())? "[" _ value:spanned(<expr()>) _ "]" {IndexPart {
			value,
			#[cfg(feature = "exp-null-coaelse")]
			null_coaelse: n.is_some(),
		}}

		pub rule jsonnet() -> Expr = _ e:expr() _ {e}
	}
}

pub type ParseError = peg::error::ParseError<peg::str::LineCol>;
pub fn parse(str: &str, source: Source) -> Result<Expr, ParseError> {
	with_current_source(source, || jsonnet_parser::jsonnet(str))
}

#[cfg(test)]
pub mod tests {
	use jrsonnet_interner::IStr;
	use BinaryOpType::*;

	use super::{expr::*, parse};
	use crate::source::Source;

	macro_rules! parse {
		($s:expr) => {
			parse($s, Source::new_virtual("<test>".into(), IStr::empty())).unwrap()
		};
	}

	macro_rules! el {
		($expr:expr, $from:expr, $to:expr$(,)?) => {
			Spanned(
				$expr,
				Span(
					Source::new_virtual("<test>".into(), IStr::empty()),
					$from,
					$to,
				),
			)
		};
	}

	#[test]
	fn multiline_string() {
		assert_eq!(
			parse!("|||\n    Hello world!\n     a\n|||"),
			el!(Expr::Str("Hello world!\n a\n".into()), 0, 31),
		);
		assert_eq!(
			parse!("|||\n  Hello world!\n   a\n|||"),
			el!(Expr::Str("Hello world!\n a\n".into()), 0, 27),
		);
		assert_eq!(
			parse!("|||\n\t\tHello world!\n\t\t\ta\n|||"),
			el!(Expr::Str("Hello world!\n\ta\n".into()), 0, 27),
		);
		assert_eq!(
			parse!("|||\n   Hello world!\n    a\n |||"),
			el!(Expr::Str("Hello world!\n a\n".into()), 0, 30),
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
			el!(Expr::Str(r#"Hello, "world"!"#.into()), 0, 19),
		);
		assert_eq!(
			parse!(r#"'Hello \'world\'!'"#),
			el!(Expr::Str("Hello 'world'!".into()), 0, 18),
		);
		assert_eq!(parse!(r#"'\\\\'"#), el!(Expr::Str("\\\\".into()), 0, 6));
	}

	#[test]
	fn string_unescaping() {
		assert_eq!(
			parse!(r#""Hello\nWorld""#),
			el!(Expr::Str("Hello\nWorld".into()), 0, 14),
		);
	}

	#[test]
	fn string_verbantim() {
		assert_eq!(
			parse!(r#"@"Hello\n""World""""#),
			el!(Expr::Str("Hello\\n\"World\"".into()), 0, 19),
		);
	}

	#[test]
	fn imports() {
		assert_eq!(
			parse!("import \"hello\""),
			el!(Expr::Import(el!(Expr::Str("hello".into()), 7, 14)), 0, 14),
		);
		assert_eq!(
			parse!("importstr \"garnish.txt\""),
			el!(
				Expr::ImportStr(el!(Expr::Str("garnish.txt".into()), 10, 23)),
				0,
				23
			)
		);
		assert_eq!(
			parse!("importbin \"garnish.bin\""),
			el!(
				Expr::ImportBin(el!(Expr::Str("garnish.bin".into()), 10, 23)),
				0,
				23
			)
		);
	}

	#[test]
	fn empty_object() {
		assert_eq!(
			parse!("{}"),
			el!(Expr::Obj(ObjBody::MemberList(vec![])), 0, 2)
		);
	}

	#[test]
	fn basic_math() {
		assert_eq!(
			parse!("2+2*2"),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(el!(Expr::Num(2.0), 2, 3), Mul, el!(Expr::Num(2.0), 4, 5)),
						2,
						5
					)
				),
				0,
				5
			)
		);
	}

	#[test]
	fn basic_math_with_indents() {
		assert_eq!(
			parse!("2	+ 	  2	  *	2   	"),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(el!(Expr::Num(2.0), 7, 8), Mul, el!(Expr::Num(2.0), 13, 14),),
						7,
						14
					),
				),
				0,
				14
			)
		);
	}

	#[test]
	fn basic_math_parened() {
		assert_eq!(
			parse!("2+(2+2*2)"),
			el!(
				Expr::BinaryOp(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(
							el!(Expr::Num(2.0), 3, 4),
							Add,
							el!(
								Expr::BinaryOp(
									el!(Expr::Num(2.0), 5, 6),
									Mul,
									el!(Expr::Num(2.0), 7, 8),
								),
								5,
								8
							),
						),
						3,
						8
					),
				),
				0,
				9
			)
		);
	}

	/// Comments should not affect parsing
	#[test]
	fn comments() {
		assert_eq!(
			parse!("2//comment\n+//comment\n3/*test*/*/*test*/4"),
			el!(
				expr_bin!(
					el!(Expr::Num(2.0), 0, 1),
					Add,
					el!(
						Expr::BinaryOp(
							el!(Expr::Num(3.0), 22, 23),
							Mul,
							el!(Expr::Num(4.0), 40, 41)
						),
						22,
						41
					)
				),
				0,
				41
			)
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
		/*
		`ArrComp(Apply(Index(Var("std") from "test.jsonnet":1-4, Var("deepJoin") from "test.jsonnet":5-13) from "test.jsonnet":1-13, ArgsDesc { unnamed: [Var("x") from "test.jsonnet":14-15], named: [] }, false) from "test.jsonnet":1-16, [ForSpec(ForSpecData("x", Var("arr") from "test.jsonnet":26-29))]) from "test.jsonnet":0-30`,
		`ArrComp(Apply(Index(Var("std") from "test.jsonnet":1-4, Str("deepJoin") from "test.jsonnet":5-13) from "test.jsonnet":1-13, ArgsDesc { unnamed: [Var("x") from "test.jsonnet":14-15], named: [] }, false) from "test.jsonnet":1-16, [ForSpec(ForSpecData("x", Var("arr") from "test.jsonnet":26-29))]) from "test.jsonnet":0-30`
				*/
		assert_eq!(
			parse!("[std.deepJoin(x) for x in arr]"),
			el!(
				ArrComp(
					el!(
						Apply(
							el!(
								Index {
									indexable: el!(Var("std".into()), 1, 4),
									parts: vec![IndexPart {
										value: el!(Str("deepJoin".into()), 5, 13),
										#[cfg(feature = "exp-null-coaelse")]
										null_coaelse: false,
									}],
								},
								1,
								13
							),
							ArgsDesc::new(vec![el!(Var("x".into()), 14, 15)], vec![]),
							false,
						),
						1,
						16
					),
					vec![CompSpec::ForSpec(ForSpecData(
						Destruct::Full("x".into()),
						el!(Var("arr".into()), 26, 29)
					))]
				),
				0,
				30
			),
		)
	}

	#[test]
	fn reserved() {
		use Expr::*;
		assert_eq!(parse!("null"), el!(Literal(LiteralType::Null), 0, 4));
		assert_eq!(parse!("nulla"), el!(Var("nulla".into()), 0, 5));
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
			el!(
				BinaryOp(
					el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()), 1, 2)), 0, 2),
					And,
					el!(UnaryOp(UnaryOpType::Not, el!(Var("b".into()), 7, 8)), 6, 8)
				),
				0,
				8
			)
		);
	}

	#[test]
	fn infix_precedence_division() {
		use Expr::*;
		assert_eq!(
			parse!("!a / !b"),
			el!(
				BinaryOp(
					el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()), 1, 2)), 0, 2),
					Div,
					el!(UnaryOp(UnaryOpType::Not, el!(Var("b".into()), 6, 7)), 5, 7)
				),
				0,
				7
			)
		);
	}

	#[test]
	fn double_negation() {
		use Expr::*;
		assert_eq!(
			parse!("!!a"),
			el!(
				UnaryOp(
					UnaryOpType::Not,
					el!(UnaryOp(UnaryOpType::Not, el!(Var("a".into()), 2, 3)), 1, 3)
				),
				0,
				3
			)
		)
	}

	#[test]
	fn array_test_error() {
		parse!("[a for a in b if c for e in f]");
		//                    ^^^^ failed code
	}

	#[test]
	fn missing_newline_between_comment_and_eof() {
		parse!(
			"{a:1}

			//+213"
		);
	}

	#[test]
	fn default_param_before_nondefault() {
		parse!("local x(foo = 'foo', bar) = null; null");
	}

	#[test]
	fn add_location_info_to_all_sub_expressions() {
		use Expr::*;

		let file_name = Source::new_virtual("<test>".into(), IStr::empty());
		let expr = parse("{} { local x = 1, x: x } + {}", file_name).unwrap();
		assert_eq!(
			expr,
			el!(
				BinaryOp(
					el!(
						ObjExtend(
							el!(Obj(ObjBody::MemberList(vec![])), 0, 2),
							ObjBody::MemberList(vec![
								Member::BindStmt(BindSpec::Field {
									into: Destruct::Full("x".into()),
									value: el!(Num(1.0), 15, 16)
								}),
								Member::Field(FieldMember {
									name: FieldName::Fixed("x".into()),
									plus: false,
									params: None,
									visibility: Visibility::Normal,
									value: el!(Var("x".into()), 21, 22),
								})
							])
						),
						0,
						24
					),
					BinaryOpType::Add,
					el!(Obj(ObjBody::MemberList(vec![])), 27, 29),
				),
				0,
				29
			),
		);
	}
}
