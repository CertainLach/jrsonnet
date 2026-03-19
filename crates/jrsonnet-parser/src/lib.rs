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
	Source, SourceDefaultIgnoreJpath, SourceDirectory, SourceFifo, SourceFile, SourcePath,
	SourcePathT, SourceVirtual,
};

pub struct ParserSettings {
	pub source: Source,
}

macro_rules! expr_bin {
	($a:ident $op:ident $b:ident) => {
		Expr::BinaryOp(Box::new(BinaryOp {
			lhs: $a,
			op: $op,
			rhs: $b,
		}))
	};
}
macro_rules! expr_un {
	($op:ident $a:ident) => {
		Expr::UnaryOp($op, Box::new($a))
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
		rule uint_str() -> &'input str = a:$(digit()+ ("_" digit()+)*) { a }
		/// Number in scientific notation format
		rule number() -> f64 = quiet!{a:$(uint_str() ("." uint_str())? (['e'|'E'] (s:['+'|'-'])? uint_str())?) {? a.replace("_","").parse().map_err(|_| "<number>") }} / expected!("<number>")

		/// Reserved word followed by any non-alphanumberic
		rule reserved() = ("assert" / "else" / "error" / "false" / "for" / "function" / "if" / "import" / "importstr" / "importbin" / "in" / "local" / "null" / "tailstrict" / "then" / "self" / "super" / "true") end_of_ident()
		rule id() -> IStr = v:$(quiet!{ !reserved() alpha() (alpha() / digit())*} / expected!("<identifier>")) { v.into() }

		rule keyword(id: &'static str) -> ()
			= ##parse_string_literal(id) end_of_ident()

		pub rule param(s: &ParserSettings) -> expr::Param = name:destruct(s) expr:(_ "=" _ expr:expr(s){expr})? { expr::Param(name, expr.map(Rc::new)) }
		pub rule params(s: &ParserSettings) -> expr::ParamsDesc
			= params:param(s) ** comma() comma()? { expr::ParamsDesc(Rc::new(params)) }
			/ { expr::ParamsDesc(Rc::new(Vec::new())) }

		pub rule arg(s: &ParserSettings) -> (Option<IStr>, Rc<Spanned<Expr>>)
			= name:(quiet! { (s:id() _ "=" !['='] _ {s})? } / expected!("<argument name>")) expr:expr(s) {(name, Rc::new(expr))}

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
			= into:destruct(s) _ "=" _ expr:expr(s) {expr::BindSpec::Field{into, value: Rc::new(expr)}}
			/ name:id() _ "(" _ params:params(s) _ ")" _ "=" _ expr:expr(s) {expr::BindSpec::Function{name, params, value: Rc::new(expr)}}

		pub rule assertion(s: &ParserSettings) -> expr::AssertStmt
			= keyword("assert") _ cond:expr(s) msg:(_ ":" _ e:expr(s) {e})? { expr::AssertStmt(cond, msg) }

		pub rule whole_line() -> &'input str
			= str:$((!['\n'][_])* "\n") {str}
		pub rule string_block() -> String
			= "|||" chomped:"-"? (!['\n']single_whitespace())* "\n"
			empty_lines:$(['\n']*)
			prefix:[' ' | '\t']+ first_line:whole_line()
			lines:("\n" {"\n"} / [' ' | '\t']*<{prefix.len()}> s:whole_line() {s})*
			[' ' | '\t']*<, {prefix.len() - 1}> "|||"
			{
				let mut l = empty_lines.to_owned();
				l.push_str(first_line);
				l.extend(lines);
				if chomped.is_some() {
					debug_assert!(l.ends_with('\n'));
					l.truncate(l.len() - 1);
				}
				l
			}

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
				value: Rc::new(value),
			}}
			/ name:field_name(s) _ "(" _ params:params(s) _ ")" _ visibility:visibility() _ value:expr(s) {expr::FieldMember{
				name,
				plus: false,
				params: Some(params),
				visibility,
				value: Rc::new(value),
			}}
		pub rule obj_local(s: &ParserSettings) -> BindSpec
			= keyword("local") _ bind:bind(s) {bind}
		pub rule member(s: &ParserSettings) -> expr::Member
			= bind:obj_local(s) {expr::Member::BindStmt(bind)}
			/ assertion:assertion(s) {expr::Member::AssertStmt(Rc::new(assertion))}
			/ field:field(s) {expr::Member::Field(field)}
		pub rule objinside(s: &ParserSettings) -> expr::ObjBody
			= pre_locals:(b: obj_local(s) comma() {b})* &"[" field:field(s) post_locals:(comma() b:obj_local(s) {b})* _ ("," _)? forspec:forspec(s) others:(_ rest:compspec(s) {rest})? {
				let mut compspecs = vec![CompSpec::ForSpec(forspec)];
				compspecs.extend(others.unwrap_or_default());
				expr::ObjBody::ObjComp(expr::ObjComp{
					pre_locals,
					field: Rc::new(field),
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
			= keyword("local") _ binds:bind(s) ** comma() (_ ",")? _ ";" _ expr:expr(s) { Expr::LocalExpr(binds, Box::new(expr)) }
		pub rule string_expr(s: &ParserSettings) -> Expr
			= s:string() {Expr::Str(s.into())}
		pub rule obj_expr(s: &ParserSettings) -> Expr
			= "{" _ body:objinside(s) _ "}" {Expr::Obj(body)}
		pub rule array_expr(s: &ParserSettings) -> Expr
			= "[" _ elems:(expr(s) ** comma()) _ comma()? "]" {Expr::Arr(Rc::new(elems))}
		pub rule array_comp_expr(s: &ParserSettings) -> Expr
			= "[" _ expr:expr(s) _ comma()? _ forspec:forspec(s) _ others:(others: compspec(s) _ {others})? "]" {
				let mut specs = vec![CompSpec::ForSpec(forspec)];
				specs.extend(others.unwrap_or_default());
				Expr::ArrComp(Rc::new(expr), specs)
			}
		pub rule number_expr(s: &ParserSettings) -> Expr
			= n:number() {? if n.is_finite() {
				Ok(expr::Expr::Num(n))
			} else {
				Err("!!!numbers are finite")
			}}
		pub rule var_expr(s: &ParserSettings) -> Expr
			= n:id() { expr::Expr::Var(n) }
		pub rule id_loc(s: &ParserSettings) -> Spanned<Expr>
			= a:position!() n:id() b:position!() { Spanned::new(expr::Expr::Str(n), Span(s.source.clone(), a as u32,b as u32)) }
		pub rule if_then_else_expr(s: &ParserSettings) -> Expr
			= cond:ifspec(s) _ keyword("then") _ cond_then:expr(s) cond_else:(_ keyword("else") _ e:expr(s) {e})? {Expr::IfElse(Box::new(IfElse{
				cond,
				cond_then,
				cond_else,
			}))}

		pub rule literal(s: &ParserSettings) -> Expr
			= v:(
				keyword("null") {LiteralType::Null}
				/ keyword("true") {LiteralType::True}
				/ keyword("false") {LiteralType::False}
				/ keyword("self") {LiteralType::This}
				/ keyword("$") {LiteralType::Dollar}
				/ keyword("super") {LiteralType::Super}
			) {Expr::Literal(v)}

		rule import_kind() -> ImportKind
			= keyword("importstr") { ImportKind::Str }
			/ keyword("importbin") { ImportKind::Bin }
			/ keyword("import") { ImportKind::Normal }

		pub rule expr_basic(s: &ParserSettings) -> Expr
			= literal(s)

			/ string_expr(s) / number_expr(s)
			/ array_expr(s)
			/ obj_expr(s)
			/ array_expr(s)
			/ array_comp_expr(s)

			/ kind:import_kind() _ path:expr(s) {Expr::Import(kind, Box::new(path))}

			/ var_expr(s)
			/ local_expr(s)
			/ if_then_else_expr(s)

			/ keyword("function") _ "(" _ params:params(s) _ ")" _ expr:expr(s) {Expr::Function(params, Rc::new(expr))}
			/ assert:assertion(s) _ ";" _ rest:expr(s) { Expr::AssertExpr(Rc::new(AssertExpr{
				assert, rest
			})) }

			/ keyword("error") _ expr:expr(s) { Expr::ErrorStmt(Box::new(expr)) }

		rule slice_part(s: &ParserSettings) -> Option<Spanned<Expr>>
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
		rule expr(s: &ParserSettings) -> Spanned<Expr>
			= precedence! {
				"(" _ e:expr(s) _ ")" {e}
				start:position!() v:@ end:position!() { Spanned::new(v, Span(s.source.clone(), start as u32, end as u32)) }
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
						unaryop(<"+">) _ b:@ {expr_un!(Plus b)}
						unaryop(<"-">) _ b:@ {expr_un!(Minus b)}
						unaryop(<"!">) _ b:@ {expr_un!(Not b)}
						unaryop(<"~">) _ b:@ {expr_un!(BitNot b)}
				--
				value:(@) _ "[" _ slice:slice_desc(s) _ "]" {Expr::Slice(Box::new(Slice{value, slice}))}
				indexable:(@) _ parts:index_part(s)+ {Expr::Index{indexable: Box::new(indexable), parts}}
				a:(@) _ "(" _ args:args(s) _ ")" ts:(_ keyword("tailstrict"))? {Expr::Apply(Box::new(a), args, ts.is_some())}
				a:(@) _ "{" _ body:objinside(s) _ "}" {Expr::ObjExtend(Rc::new(a), body)}
				--
				e:expr_basic(s) {e}
			}
		pub rule index_part(s: &ParserSettings) -> IndexPart
		= n:("?" _ ensure_null_coaelse())? "." _ value:id_loc(s) {IndexPart {
			value,
			#[cfg(feature = "exp-null-coaelse")]
			null_coaelse: n.is_some(),
		}}
		/ n:("?" _ "." _ ensure_null_coaelse())? "[" _ value:expr(s) _ "]" {IndexPart {
			value,
			#[cfg(feature = "exp-null-coaelse")]
			null_coaelse: n.is_some(),
		}}

		pub rule jsonnet(s: &ParserSettings) -> Spanned<Expr> = _ e:expr(s) _ {e}
	}
}

pub type ParseError = peg::error::ParseError<peg::str::LineCol>;
pub fn parse(str: &str, settings: &ParserSettings) -> Result<Spanned<Expr>, ParseError> {
	jsonnet_parser::jsonnet(str, settings)
}
/// Used for importstr values
pub fn string_to_expr(str: IStr, settings: &ParserSettings) -> Spanned<Expr> {
	let len = str.len();
	Spanned::new(Expr::Str(str), Span(settings.source.clone(), 0, len as u32))
}

#[cfg(test)]
pub mod tests {
	use insta::assert_snapshot;
	use jrsonnet_interner::IStr;

	use super::parse;
	use crate::{source::Source, ParserSettings};

	fn parsep(s: &str) -> String {
		let v = parse(
			s,
			&ParserSettings {
				source: Source::new_virtual("<test>".into(), IStr::empty()),
			},
		)
		.unwrap();
		format!("{v:#?}")
	}

	macro_rules! parse {
		($s:expr) => {
			assert_snapshot!(parsep($s));
		};
	}

	#[test]
	fn multiline_string() {
		parse!("|||\n    Hello world!\n     a\n|||");
		parse!("|||\n  Hello world!\n   a\n|||");
		parse!("|||\n\t\tHello world!\n\t\t\ta\n|||");
		parse!("|||\n   Hello world!\n    a\n |||");
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
		parse!(r#""Hello, \"world\"!""#);
		parse!(r#"'Hello \'world\'!'"#);
		parse!(r#"'\\\\'"#);
	}

	#[test]
	fn string_unescaping() {
		parse!(r#""Hello\nWorld""#);
	}

	#[test]
	fn string_verbantim() {
		parse!(r#"@"Hello\n""World""""#);
	}

	#[test]
	fn imports() {
		parse!("import \"hello\"");
		parse!("importstr \"garnish.txt\"");
		parse!("importbin \"garnish.bin\"");
	}

	#[test]
	fn empty_object() {
		parse!("{}");
	}

	#[test]
	fn basic_math() {
		parse!("2+2*2");
		parse!("2	+ 	  2	  *	2   	");
		parse!("2+(2+2*2)");
		parse!("2//comment\n+//comment\n3/*test*/*/*test*/4");
	}

	#[test]
	fn suffix() {
		parse!("std.test");
		parse!("std(2)");
		parse!("std.test(2)");
		parse!("a[b]");
	}

	#[test]
	fn array_comp() {
		parse!("[std.deepJoin(x) for x in arr]");
	}

	#[test]
	fn reserved() {
		parse!("null");
		parse!("nulla");
	}

	#[test]
	fn multiple_args_buf() {
		parse!("a(b, null_fields)");
	}

	#[test]
	fn infix_precedence() {
		parse!("!a && !b");
		parse!("!a / !b");
	}

	#[test]
	fn double_negation() {
		parse!("!!a");
	}

	#[test]
	fn array_test_error() {
		parse!("[a for a in b if c for e in f]");
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
		parse!("{} { local x = 1, x: x } + {}");
	}
}
