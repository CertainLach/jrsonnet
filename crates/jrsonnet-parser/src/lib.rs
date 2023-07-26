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
pub use source::{Source, SourceDirectory, SourceFile, SourcePath, SourcePathT, SourceVirtual};

pub struct ParserSettings {
	pub source: Source,
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

		rule eof() = quiet!{![_]} / expected!("<eof>")
		rule eol() = "\n" / eof()

		/// Standard C-like comments
		rule comment()
			= "//" (!eol()[_])* eol()
			/ "/*" ("\\*/" / "\\\\" / (!("*/")[_]))* "*/"
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
		rule number() -> f64 = quiet!{a:$(uint_str() ("." uint_str())? (['e'|'E'] (s:['+'|'-'])? uint_str())?) {? a.parse().map_err(|_| "<number>") }} / expected!("<number>")

		/// Reserved word followed by any non-alphanumberic
		rule reserved() = ("assert" / "else" / "error" / "false" / "for" / "function" / "if" / "import" / "importstr" / "importbin" / "in" / "local" / "null" / "tailstrict" / "then" / "self" / "super" / "true") end_of_ident()
		rule id() -> IStr = v:$(quiet!{ !reserved() alpha() (alpha() / digit())*} / expected!("<identifier>")) { v.into() }

		rule keyword(id: &'static str) -> ()
			= ##parse_string_literal(id) end_of_ident()

		pub rule param(s: &ParserSettings) -> expr::Param = name:destruct(s) expr:(_ "=" _ expr:expr(s){expr})? { expr::Param(name, expr) }
		pub rule params(s: &ParserSettings) -> expr::ParamsDesc
			= params:param(s) ** comma() comma()? { expr::ParamsDesc(Rc::new(params)) }
			/ { expr::ParamsDesc(Rc::new(Vec::new())) }

		pub rule arg(s: &ParserSettings) -> (Option<IStr>, LocExpr)
			= name:(quiet! { (s:id() _ "=" !['='] _ {s})? } / expected!("<argument name>")) expr:expr(s) {(name, expr)}

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

		pub rule destruct_rest() -> expr::DestructRest
			= "..." into:(_ into:id() {into})? {if let Some(into) = into {
				expr::DestructRest::Keep(into)
			} else {expr::DestructRest::Drop}}
		pub rule destruct_array(s: &ParserSettings) -> expr::Destruct
			= "[" _ start:destruct(s)**comma() rest:(
				comma() _ rest:destruct_rest()? end:(
					comma() end:destruct(s)**comma() (_ comma())? {end}
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
		pub rule destruct_object(s: &ParserSettings) -> expr::Destruct
			= "{" _
				fields:(name:id() into:(_ ":" _ into:destruct(s) {into})? default:(_ "=" _ v:expr(s) {v})? {(name, into, default)})**comma()
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
		pub rule destruct(s: &ParserSettings) -> expr::Destruct
			= v:id() {expr::Destruct::Full(v)}
			/ "?" {?
				#[cfg(feature = "exp-destruct")] return Ok(expr::Destruct::Skip);
				#[cfg(not(feature = "exp-destruct"))] Err("!!!experimental destructuring was not enabled")
			}
			/ arr:destruct_array(s) {arr}
			/ obj:destruct_object(s) {obj}

		pub rule bind(s: &ParserSettings) -> expr::BindSpec
			= into:destruct(s) _ "=" _ expr:expr(s) {expr::BindSpec::Field{into, value: expr}}
			/ name:id() _ "(" _ params:params(s) _ ")" _ "=" _ expr:expr(s) {expr::BindSpec::Function{name, params, value: expr}}

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

		pub rule field_name(s: &ParserSettings) -> expr::FieldName
			= name:id() {expr::FieldName::Fixed(name)}
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
			= pre_locals:(b: obj_local(s) comma() {b})* &"[" field:field(s) post_locals:(comma() b:obj_local(s) {b})* _ ("," _)? forspec:forspec(s) others:(_ rest:compspec(s) {rest})? {
				let mut compspecs = vec![CompSpec::ForSpec(forspec)];
				compspecs.extend(others.unwrap_or_default());
				expr::ObjBody::ObjComp(expr::ObjComp{
					pre_locals,
					field,
					post_locals,
					compspecs,
				})
			}
			/ members:(member(s) ** comma()) comma()? {expr::ObjBody::MemberList(members)}
		pub rule ifspec(s: &ParserSettings) -> IfSpecData
			= keyword("if") _ expr:expr(s) {IfSpecData(expr)}
		pub rule forspec(s: &ParserSettings) -> ForSpecData
			= keyword("for") _ id:destruct(s) _ keyword("in") _ cond:expr(s) {ForSpecData(id, cond)}
		pub rule compspec(s: &ParserSettings) -> Vec<expr::CompSpec>
			= s:(i:ifspec(s) { expr::CompSpec::IfSpec(i) } / f:forspec(s) {expr::CompSpec::ForSpec(f)} ) ** _ {s}
		pub rule local_expr(s: &ParserSettings) -> Expr
			= keyword("local") _ binds:bind(s) ** comma() (_ ",")? _ ";" _ expr:expr(s) { Expr::LocalExpr(binds, expr) }
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
			= n:id() { expr::Expr::Var(n) }
		pub rule id_loc(s: &ParserSettings) -> LocExpr
			= a:position!() n:id() b:position!() { LocExpr(Rc::new(expr::Expr::Str(n)), ExprLocation(s.source.clone(), a as u32,b as u32)) }
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

			/ string_expr(s) / number_expr(s)
			/ array_expr(s)
			/ obj_expr(s)
			/ array_expr(s)
			/ array_comp_expr(s)

			/ keyword("importstr") _ path:expr(s) {Expr::ImportStr(path)}
			/ keyword("importbin") _ path:expr(s) {Expr::ImportBin(path)}
			/ keyword("import") _ path:expr(s) {Expr::Import(path)}

			/ var_expr(s)
			/ local_expr(s)
			/ if_then_else_expr(s)

			/ keyword("function") _ "(" _ params:params(s) _ ")" _ expr:expr(s) {Expr::Function(params, expr)}
			/ assertion:assertion(s) _ ";" _ expr:expr(s) { Expr::AssertExpr(assertion, expr) }

			/ keyword("error") _ expr:expr(s) { Expr::ErrorStmt(expr) }

		rule slice_part(s: &ParserSettings) -> Option<LocExpr>
			= _ e:(e:expr(s) _{e})? {e}
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

		rule ensure_null_coaelse()
			= "" {?
				#[cfg(not(feature = "exp-null-coaelse"))] return Err("!!!experimental null coaelscing was not enabled");
				#[cfg(feature = "exp-null-coaelse")] Ok(())
			}
		use BinaryOpType::*;
		use UnaryOpType::*;
		rule expr(s: &ParserSettings) -> LocExpr
			= precedence! {
				start:position!() v:@ end:position!() { LocExpr(Rc::new(v), ExprLocation(s.source.clone(), start as u32, end as u32)) }
				--
				a:(@) _ binop(<"||">) _ b:@ {expr_bin!(a Or b)}
				a:(@) _ binop(<"??">) _ ensure_null_coaelse() b:@ {
					#[cfg(feature = "exp-null-coaelse")] return expr_bin!(a NullCoaelse b);
					unreachable!("ensure_null_coaelse will fail if feature is not enabled")
				}
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
				indexable:(@) _ null_coaelse:("?" _ ensure_null_coaelse())? "."  _ index:id_loc(s) {Expr::Index{
					indexable, index,
					#[cfg(feature = "exp-null-coaelse")]
					null_coaelse: null_coaelse.is_some(),
				}}
				indexable:(@) _ null_coaelse:("?" _ "." _ ensure_null_coaelse())? "[" _ index:expr(s) _ "]" {Expr::Index{
					indexable, index,
					#[cfg(feature = "exp-null-coaelse")]
					null_coaelse: null_coaelse.is_some(),
				}}
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
/// Used for importstr values
pub fn string_to_expr(str: IStr, settings: &ParserSettings) -> LocExpr {
	let len = str.len();
	LocExpr(
		Rc::new(Expr::Str(str)),
		ExprLocation(settings.source.clone(), 0, len as u32),
	)
}

#[cfg(test)]
pub mod tests {
	use jrsonnet_interner::IStr;
	use BinaryOpType::*;

	use super::{expr::*, parse};
	use crate::{source::Source, ParserSettings};

	macro_rules! parse {
		($s:expr) => {
			parse(
				$s,
				&ParserSettings {
					source: Source::new_virtual("<test>".into(), IStr::empty()),
				},
			)
			.unwrap()
		};
	}

	macro_rules! el {
		($expr:expr, $from:expr, $to:expr$(,)?) => {
			LocExpr(
				std::rc::Rc::new($expr),
				ExprLocation(
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
						Expr::Parened(el!(
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
						)),
						2,
						9
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
				Expr::BinaryOp(
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

	/// Comments should be able to be escaped
	#[test]
	fn comment_escaping() {
		assert_eq!(
			parse!("2/*\\*/+*/ - 22"),
			el!(
				Expr::BinaryOp(el!(Expr::Num(2.0), 0, 1), Sub, el!(Expr::Num(22.0), 12, 14)),
				0,
				14
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
									index: el!(Str("deepJoin".into()), 5, 13),
									#[cfg(feature = "exp-null-coaelse")]
									null_coaelse: false,
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
		let expr = parse(
			"{} { local x = 1, x: x } + {}",
			&ParserSettings { source: file_name },
		)
		.unwrap();
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
