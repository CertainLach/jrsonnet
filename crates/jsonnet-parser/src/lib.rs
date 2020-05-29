#![feature(box_syntax)]

use peg::parser;

mod expr;
pub use expr::*;

enum Suffix {
	String(String),
	Slice(SliceDesc),
	Expression(Expr),
	Apply(expr::ArgsDesc),
	Extend(expr::ObjBody),
}

parser! {
	grammar jsonnet_parser() for str {
		use peg::ParseLiteral;

		/// Standard C-like comments
		rule comment() = "//" (!['\n'][_])* "\n" / "/*" ((!("*/")[_][_])/("\\" "*/"))* "*/"
		rule _() = ([' ' | '\n' | '\t'] / comment())*

		/// For comma-delimited elements
		rule comma() = quiet!{_ "," _} / expected!("<comma>")
		rule alpha() -> char = c:$(['_' | 'a'..='z' | 'A'..='Z']) {c.chars().nth(0).unwrap()}
		rule digit() -> char = d:$(['0'..='9']) {d.chars().nth(0).unwrap()}
		rule end_of_ident() = !['0'..='9' | '_' | 'a'..='z' | 'A'..='Z']
		/// Sequence of digits
		rule uint() -> u32 = a:$(digit()+) { a.parse().unwrap() }
		/// Number in scientific notation format
		rule number() -> f64 = quiet!{a:$(uint() ("." uint())? (['e'|'E'] (s:['+'|'-'])? uint())?) { a.parse().unwrap() }} / expected!("<number>")

		/// Reserved word followed by any non-alphanumberic
		rule reserved() = ("assert" / "else" / "error" / "false" / "for" / "function" / "if" / "import" / "importstr" / "in" / "local" / "null" / "tailstrict" / "then" / "self" / "super" / "true") end_of_ident()
		rule id() -> String = quiet!{ !reserved() s:$(alpha() (alpha() / digit())*) {s.to_owned()}} / expected!("<identifier>")
		rule keyword(id: &'static str) = ##parse_string_literal(id) end_of_ident()

		pub rule param() -> expr::Param = name:id() expr:(_ "=" _ expr:boxed_expr(){expr})? { expr::Param(name, expr) }
		pub rule params() -> expr::ParamsDesc
			= params:(param() ** comma()) {
				let mut defaults_started = false;
				for param in &params {
					defaults_started = defaults_started || param.1.is_some();
					assert_eq!(defaults_started, param.1.is_some(), "defauld parameters should be used after all positionals");
				}
				expr::ParamsDesc(params)
			}
			/ { expr::ParamsDesc(Vec::new()) }

		pub rule arg() -> expr::Arg
			= name:id() _ "=" _ expr:boxed_expr() {expr::Arg(Some(name), expr)}
			/ expr:boxed_expr() {expr::Arg(None, expr)}
		pub rule args() -> expr::ArgsDesc
			= args:arg() ** comma() comma()? {
				let mut named_started = false;
				for arg in &args {
					named_started = named_started || arg.0.is_some();
					assert_eq!(named_started, arg.0.is_some(), "named args should be used after all positionals");
				}
				expr::ArgsDesc(args)
			}
			/ { expr::ArgsDesc(Vec::new()) }

		pub rule bind() -> expr::BindSpec
			= name:id() _ "=" _ expr:boxed_expr() {expr::BindSpec{name, params: None, value: expr}}
			/ name:id() _ "(" _ params:params() _ ")" _ "=" _ expr:boxed_expr() {expr::BindSpec{name, params: Some(params), value: expr}}
		pub rule assertion() -> expr::AssertStmt = keyword("assert") _ cond:boxed_expr() msg:(_ ":" _ e:boxed_expr() {e})? { expr::AssertStmt(cond, msg) }
		pub rule string() -> String
			= "\"" str:$(("\\\"" / !['"'][_])*) "\"" {str.to_owned()}
			/ "'" str:$((!['\''][_])*) "'" {str.to_owned()}
		pub rule field_name() -> expr::FieldName
			= name:id() {expr::FieldName::Fixed(name)}
			/ name:string() {expr::FieldName::Fixed(name)}
			/ "[" _ expr:boxed_expr() _ "]" {expr::FieldName::Dyn(expr)}
		pub rule visibility() -> expr::Visibility
			= ":::" {expr::Visibility::Unhide}
			/ "::" {expr::Visibility::Hidden}
			/ ":" {expr::Visibility::Normal}
		pub rule field() -> expr::FieldMember
			= name:field_name() _ plus:"+"? _ visibility:visibility() _ value:expr() {expr::FieldMember{
				name,
				plus: plus.is_some(),
				params: None,
				visibility,
				value,
			}}
			/ name:field_name() _ "(" _ params:params() _ ")" _ visibility:visibility() _ value:expr() {expr::FieldMember{
				name,
				plus: false,
				params: Some(params),
				visibility,
				value,
			}}
		pub rule obj_local() -> BindSpec
			= keyword("local") _ bind:bind() {bind}
		pub rule member() -> expr::Member
			= bind:obj_local() {expr::Member::BindStmt(bind)}
			/ assertion:assertion() {expr::Member::AssertStmt(assertion)}
			/ field:field() {expr::Member::Field(field)}
		pub rule objinside() -> expr::ObjBody
			= pre_locals:(b: obj_local() comma() {b})* "[" _ key:boxed_expr() _ "]" _ ":" _ value:boxed_expr() post_locals:(comma() b:obj_local() {b})* _ first:forspec() rest:(_ rest:compspec() {rest})? {
				expr::ObjBody::ObjComp {
					pre_locals,
					key,
					value,
					post_locals,
					first,
					rest: rest.unwrap_or(Vec::new()),
				}
			}
			/ members:(member() ** comma()) comma()? {expr::ObjBody::MemberList(members)}
		pub rule ifspec() -> IfSpecData = keyword("if") _ expr:boxed_expr() {IfSpecData(expr)}
		pub rule forspec() -> ForSpecData = keyword("for") _ id:id() _ keyword("in") _ cond:boxed_expr() {ForSpecData(id, cond)}
		pub rule compspec() -> Vec<expr::CompSpec> = s:(i:ifspec() { expr::CompSpec::IfSpec(i) } / f:forspec() {expr::CompSpec::ForSpec(f)} )+ {s}
		pub rule bind_expr() -> Expr = bind:bind() {Expr::Bind(bind)}
		pub rule local_expr() -> Expr = keyword("local") _ binds:bind() ** comma() _ ";" _ expr:boxed_expr() { Expr::LocalExpr(binds, expr) }
		pub rule string_expr() -> Expr = s:string() {Expr::Str(s)}
		pub rule parened_expr() -> Expr = "(" e:boxed_expr() ")" {Expr::Parened(e)}
		pub rule obj_expr() -> Expr = "{" _ body:objinside() _ "}" {Expr::Obj(body)}
		pub rule array_expr() -> Expr = "[" _ elems:(expr() ** comma()) _ comma()? "]" {Expr::Arr(elems)}
		pub rule array_comp_expr() -> Expr = "[" _ expr:boxed_expr() _ comma()? _ forspec:forspec() _ others:(others: compspec() _ {others})? "]" {Expr::ArrComp(expr, forspec, others.unwrap_or(vec![]))}
		pub rule index_expr() -> Expr
			= val:boxed_expr() "." idx:id() {Expr::Index(val, Box::new(Expr::Str(idx)))}
			/ val:boxed_expr() "[" key:boxed_expr() "]" {Expr::Index(val, key)}
		pub rule number_expr() -> Expr = n:number() { expr::Expr::Num(n) }
		pub rule var_expr() -> Expr = n:id() { expr::Expr::Var(n) }
		pub rule if_then_else_expr() -> Expr = cond:ifspec() _ keyword("then") _ cond_then:boxed_expr() cond_else:(_ keyword("else") _ e:boxed_expr() {e})? {Expr::IfElse{
			cond,
			cond_then,
			cond_else,
		}}

		pub rule literal() -> Expr
			= v:(
				keyword("null") {LiteralType::Null}
				/ keyword("true") {LiteralType::True}
				/ keyword("false") {LiteralType::False}
				/ keyword("self") {LiteralType::This}
				/ keyword("$") {LiteralType::Dollar}
				/ keyword("super") {LiteralType::Super}
			) {Expr::Literal(v)}

		pub rule expr_basic() -> Expr
			= literal()

			/ string_expr() / number_expr()
			/ array_expr()
			/ obj_expr()
			/ array_expr()
			/ array_comp_expr()

			/ var_expr()
			/ local_expr()
			/ if_then_else_expr()
			/ "-" _ expr:boxed_expr() { Expr::UnaryOp(UnaryOpType::Minus, expr) }
			/ "!" _ expr:boxed_expr() { Expr::UnaryOp(UnaryOpType::Not, expr) }

			/ keyword("function") _ "(" _ params:params() _ ")" _ expr:boxed_expr() {Expr::Function(params, expr)}
			/ assertion:assertion() _ ";" _ expr:boxed_expr() { Expr::AssertExpr(assertion, expr) }

			/ keyword("error") _ expr:boxed_expr() { Expr::Error(expr) }

		rule expr_basic_with_suffix() -> Expr
			= a:expr_basic() suffixes:(_ suffix:expr_suffix() {suffix})* {
				let mut cur = a;
				for suffix in suffixes {
					cur = match suffix {
						Suffix::String(index) => Expr::Index(Box::new(cur), Box::new(Expr::Str(index))),
						Suffix::Slice(desc) => Expr::Slice(Box::new(cur), desc),
						Suffix::Expression(index) => Expr::Index(Box::new(cur), Box::new(index)),
						Suffix::Apply(args) => Expr::Apply(Box::new(cur), args),
						Suffix::Extend(body) => Expr::ObjExtend(box cur, body),
					}
				}
				cur
			}

		pub rule slice_desc() -> SliceDesc
			= start:boxed_expr()? _ ":" _ pair:(end:boxed_expr()? _ step:(":" _ e:boxed_expr() {e})? {(end, step)})? {
				if let Some((end, step)) = pair {
					SliceDesc { start, end, step }
				}else{
					SliceDesc { start, end: None, step: None }
				}
			}

		rule expr_suffix() -> Suffix
			= "." _ s:id() { Suffix::String(s) }
			/ "[" _ s:slice_desc() _ "]" { Suffix::Slice(s) }
			/ "[" _ s:expr() _ "]" { Suffix::Expression(s) }
			/ "(" _ args:args() _ ")" (_ keyword("tailstrict"))? { Suffix::Apply(args) }
			/ "{" _ body:objinside() _ "}" { Suffix::Extend(body) }

		rule expr() -> Expr
			= a:precedence! {
				a:(@) _ "||" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Or, Box::new(b))}
				--
				a:(@) _ "&&" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::And, Box::new(b))}
				--
				a:(@) _ "|" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::BitOr, Box::new(b))}
				--
				a:@ _ "^" _ b:(@) {Expr::BinaryOp(Box::new(a), BinaryOpType::BitXor, Box::new(b))}
				--
				a:(@) _ "&" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::BitAnd, Box::new(b))}
				--
				a:(@) _ "==" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Eq, Box::new(b))}
				a:(@) _ "!=" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Ne, Box::new(b))}
				--
				a:(@) _ "<" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Lt, Box::new(b))}
				a:(@) _ ">" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Gt, Box::new(b))}
				a:(@) _ "<=" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Lte, Box::new(b))}
				a:(@) _ ">=" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Gte, Box::new(b))}
				--
				a:(@) _ "<<" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Lhs, Box::new(b))}
				a:(@) _ ">>" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Rhs, Box::new(b))}
				--
				a:(@) _ "+" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Add, Box::new(b))}
				a:(@) _ "-" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Sub, Box::new(b))}
				--
				a:(@) _ "*" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Mul, Box::new(b))}
				a:(@) _ "/" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Div, Box::new(b))}
				a:(@) _ "%" _ b:@ {Expr::BinaryOp(Box::new(a), BinaryOpType::Mod, Box::new(b))}
				--
				e:expr_basic_with_suffix() {e}
				"(" _ e:boxed_expr() _ ")" {Expr::Parened(e)}
			}
			/ e:expr_basic_with_suffix() {e}

		pub rule boxed_expr() -> Box<Expr> = e:expr() {Box::new(e)}
		pub rule jsonnet() -> Expr = _ e:expr() _ {e}
	}
}

// TODO: impl FromStr from Expr
pub fn parse(str: &str) -> Result<Expr, peg::error::ParseError<peg::str::LineCol>> {
	jsonnet_parser::jsonnet(str)
}

#[cfg(test)]
pub mod tests {
	use super::{expr::*, parse};
	#[test]
	fn empty_object() {
		assert_eq!(parse("{}").unwrap(), Expr::Obj(ObjBody::MemberList(vec![])),);
	}
	#[test]
	fn basic_math() {
		assert_eq!(
			parse("2+2*2").unwrap(),
			Expr::BinaryOp(
				Box::new(Expr::Num(2.0)),
				BinaryOpType::Add,
				Box::new(Expr::BinaryOp(
					Box::new(Expr::Num(2.0)),
					BinaryOpType::Mul,
					Box::new(Expr::Num(2.0))
				))
			)
		);
	}

	/// Comments should not affect parsing
	#[test]
	fn comments() {
		assert_eq!(
			parse("2//comment\n+//comment\n3/*test*/*/*test*/4").unwrap(),
			Expr::BinaryOp(
				box Expr::Num(2.0),
				BinaryOpType::Add,
				box Expr::BinaryOp(box Expr::Num(3.0), BinaryOpType::Mul, box Expr::Num(4.0))
			)
		);
	}

	/// Comments should be able to be escaped
	#[test]
	fn comment_escaping() {
		assert_eq!(
			parse("2/*\\*/+*/ - 22").unwrap(),
			Expr::BinaryOp(box Expr::Num(2.0), BinaryOpType::Sub, box Expr::Num(22.0))
		);
	}

	#[test]
	fn suffix_comparsion() {
		use Expr::*;
		assert_eq!(
			parse("std.type(a) == \"string\"").unwrap(),
			BinaryOp(
				box Apply(
					box Index(box Var("std".to_owned()), box Str("type".to_owned())),
					ArgsDesc(vec![Arg(None, box Var("a".to_owned()))])
				),
				BinaryOpType::Eq,
				box Str("string".to_owned())
			)
		);
	}

	#[test]
	fn array_comp() {
		use Expr::*;
		assert_eq!(
			parse("[std.deepJoin(x) for x in arr]").unwrap(),
			ArrComp(
				box Apply(
					box Index(box Var("std".to_owned()), box Str("deepJoin".to_owned())),
					ArgsDesc(vec![Arg(None, box Var("x".to_owned()))])
				),
				ForSpecData("x".to_owned(), box Var("arr".to_owned())),
				vec![]
			),
		)
	}

	#[test]
	fn array_comp_with_ifs() {
		use Expr::*;
		assert_eq!(
			parse("[k for k in std.objectFields(patch) if patch[k] == null]").unwrap(),
			ArrComp(
				box Var("k".to_owned()),
				ForSpecData(
					"k".to_owned(),
					box Apply(
						box Index(
							box Var("std".to_owned()),
							box Str("objectFields".to_owned())
						),
						ArgsDesc(vec![Arg(None, box Var("patch".to_owned()))])
					)
				),
				vec![CompSpec::IfSpec(IfSpecData(box BinaryOp(
					box Index(box Var("patch".to_owned()), box Var("k".to_owned())),
					BinaryOpType::Eq,
					box Literal(LiteralType::Null)
				)))]
			),
		);
	}

	#[test]
	fn reserved() {
		use Expr::*;
		assert_eq!(parse("null").unwrap(), Literal(LiteralType::Null));
		assert_eq!(parse("nulla").unwrap(), Var("nulla".to_owned()));
	}

	#[test]
	fn multiple_args_buf() {
		parse("a(b, null_fields)").unwrap();
	}

	#[test]
	fn can_parse_stdlib() {
		parse(jsonnet_stdlib::STDLIB_STR).unwrap();
	}
}
