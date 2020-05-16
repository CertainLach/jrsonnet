#![feature(box_syntax)]

use peg::parser;

mod expr;
pub use expr::*;

enum Suffix {
	String(String),
	Expression(Expr),
	Apply(expr::ArgsDesc),
}

parser! {
	grammar jsonnet_parser() for str {
		rule delimiter() = quiet!{__() "," __()} / expected!("<elements delimiter>")
		rule _() = quiet!{[' ' | '\n' | '\t']+} / expected!("<whitespace>")
		rule __() = quiet!{[' ' | '\n' | '\t']*}
		rule alpha() -> char = c:$(['_' | 'a'..='z' | 'A'..='Z']) {c.chars().nth(0).unwrap()}
		rule digit() -> char = d:$(['0'..='9']) {d.chars().nth(0).unwrap()}
		rule int() -> u32 = a:$(digit()+) { a.parse().unwrap() }
		rule number() -> f64 = quiet!{a:$((['-'|'+'])? int() ("." int())? (['e'|'E'] (s:['+'|'-'])? int())?) { a.parse().unwrap() }} / expected!("<number>")
		rule id() -> String = quiet!{ !("local" / "super" / "self" / "true" / "false" / "null" / "$" / "if" / "then" / "else" / "function") s:$(alpha() (alpha() / digit())*) {s.to_owned()}} / expected!("<identifier>")

		pub rule positional_param() -> expr::Param = name:id() {expr::Param::Positional(name)}
		pub rule named_param() -> expr::Param = name:id() __() "=" __() expr:boxed_expr() {expr::Param::Named(name, expr)}
		pub rule params() -> expr::ParamsDesc
			= positionals:(positional_param() ** delimiter()) named: (delimiter() named:(named_param() ** delimiter()) {named})? {
				if named.is_some() {
					expr::ParamsDesc([&positionals[..], &named.unwrap()[..]].concat())
				} else {
					expr::ParamsDesc(positionals)
				}
			}
			/ named:(named_param() ** delimiter()) {expr::ParamsDesc(named)}
			/ {expr::ParamsDesc(Vec::new())}

		pub rule positional_arg() -> expr::Arg = quiet!{name:boxed_expr() {expr::Arg::Positional(name)}}/expected!("<positional arg>")
		pub rule named_arg() -> expr::Arg = quiet!{name:id() __() "=" __() expr:boxed_expr() {expr::Arg::Named(name, expr)}}/expected!("<named arg>")
		pub rule args() -> expr::ArgsDesc
			= positionals:(positional_arg() ** delimiter()) named: (delimiter() named:(named_arg() ** delimiter()) {named})? {
				if named.is_some() {
					expr::ArgsDesc([&positionals[..], &named.unwrap()[..]].concat())
				} else {
					expr::ArgsDesc(positionals)
				}
			}
			/ named:(named_arg() ** delimiter()) {expr::ArgsDesc(named)}
			/ {expr::ArgsDesc(Vec::new())}

		pub rule bind() -> expr::Bind
			= name:id() __() "=" __() expr:boxed_expr() {expr::Bind::Value(name, expr)}
			/ name:id() __() "(" __() params:params() __() ")" __() "=" __() expr:boxed_expr() {expr::Bind::Function(name, params, expr)}
		pub rule assertion() -> expr::AssertStmt = "assert" _() cond:boxed_expr() msg:(__() ":" __() e:boxed_expr() {e})? { expr::AssertStmt(cond, msg) }
		pub rule string() -> String
			= "\"" str:$((!['"'][_])+) "\"" {str.to_owned()}
			/ "'" str:$((!['\''][_])+) "'" {str.to_owned()}
		pub rule field_name() -> expr::FieldName
			= name:id() {expr::FieldName::Fixed(name)}
			/ name:string() {expr::FieldName::Fixed(name)}
			/ "[" __() expr:boxed_expr() __() "]" {expr::FieldName::Dyn(expr)}
		pub rule visibility() -> expr::Visibility
			= ":::" {expr::Visibility::Unhide}
			/ "::" {expr::Visibility::Hidden}
			/ ":" {expr::Visibility::Normal}
		pub rule field() -> expr::FieldMember
			= name:field_name() __() plus:"+"? __() visibility:visibility() __() value:expr() {expr::FieldMember::Value{
				name,
				plus: plus.is_some(),
				visibility,
				value,
			}}
			/ name:field_name() __() "(" __() params:params() __() ")" __() visibility:visibility() __() value:expr() {expr::FieldMember::Function{
				name,
				params,
				visibility,
				value,
			}}
		pub rule member() -> expr::Member
			= "local" _() bind:bind() {expr::Member::BindStmt(bind)}
			/ assertion:assertion() {expr::Member::AssertStmt(assertion)}
			/ field:field() {expr::Member::Field(field)}
		pub rule obj_body() -> expr::ObjBody = members:(member() ** delimiter()) delimiter()? {expr::ObjBody::MemberList(members)}
		pub rule ifspec() -> expr::IfSpec = "if" _() expr:boxed_expr() {expr::IfSpec(expr)}
		pub rule forspec() -> expr::ForSpec = "for" _() id:id() _() "in" _() ifs:ifspec()* {expr::ForSpec(id, ifs)}
		pub rule bind_expr() -> Expr = bind:bind() {Expr::Bind(bind)}
		pub rule local_expr() -> Expr = "local" _() binds:(bind() ** delimiter()) __() ";" __() expr:boxed_expr() { Expr::LocalExpr(binds, expr) }
		pub rule string_expr() -> Expr = s:string() {Expr::Str(s)}
		pub rule parened_expr() -> Expr = "(" e:boxed_expr() ")" {Expr::Parened(e)}
		pub rule obj_expr() -> Expr = "{" __() body:obj_body() __() "}" {Expr::Obj(body)}
		pub rule array_expr() -> Expr = "[" __() elems:(expr() ** delimiter()) __() delimiter()? "]" {Expr::Arr(elems)}
		pub rule array_comp_expr() -> Expr = "[" __() expr:boxed_expr() delimiter()? fors:forspec()+ __() "]" {Expr::ArrComp(expr, fors)}
		pub rule index_expr() -> Expr
			= val:boxed_expr() "." idx:id() {Expr::Index(val, Box::new(Expr::Str(idx)))}
			/ val:boxed_expr() "[" key:boxed_expr() "]" {Expr::Index(val, key)}
		pub rule slice_expr() -> Expr
			= value:boxed_expr() "[" start:boxed_expr()? ":" pair:(end:boxed_expr()? step:(":" e:boxed_expr() {e})? {(end, step)})? "]" {
			if let Some((end, step)) = pair {
				Expr::Slice { value, start, end, step }
			}else{
				Expr::Slice{ value, start, end: None, step: None }
			}
		}
		pub rule number_expr() -> Expr = n:number() { expr::Expr::Num(n) }
		pub rule var_expr() -> Expr = n:id() { expr::Expr::Var(n) }
		pub rule if_then_else_expr() -> Expr = cond:ifspec() _() "then" _() cond_then:boxed_expr() cond_else:(_() "else" _() e:boxed_expr() {e})? {Expr::IfElse{
			cond,
			cond_then,
			cond_else,
		}}
		pub rule expr_basic() -> Expr
			= "null" {Expr::Value(ValueType::Null)}
			/ "true" {Expr::Value(ValueType::True)} / "false" {Expr::Value(ValueType::False)}

			/ "self" {Expr::Literal(LiteralType::This)} / "$" {Expr::Literal(LiteralType::Dollar)}
			/ "super" {Expr::Literal(LiteralType::Super)}

			/ string_expr() / number_expr()
			/ array_expr()
			/ array_comp_expr()
			/ obj_expr()
			/ array_expr()
			/ array_comp_expr()

			/ var_expr()
			/ if_then_else_expr()
			/ local_expr()

			/ "function" __() "(" __() params:params() __() ")" __() expr:boxed_expr() {Expr::Function(params, expr)}

		rule expr_basic_with_suffix() -> Expr
			= a:expr_basic() suffixes:(__() suffix:expr_suffix() {suffix})* {
				let mut cur = a;
				for suffix in suffixes {
					match suffix {
						Suffix::String(index) => {
							cur = Expr::Index(Box::new(cur), Box::new(Expr::Str(index)))
						},
						Suffix::Expression(index) => {
							cur = Expr::Index(Box::new(cur), Box::new(index))
						},
						Suffix::Apply(args) => {
							cur = Expr::Apply(Box::new(cur), args)
						}
					}
				}
				cur
			}

		rule expr_suffix() -> Suffix
			= "." __() s:id() { Suffix::String(s) }
			/ "[" __() s:expr() __() "]" { Suffix::Expression(s) }
			/ "(" __() args:args() __() ")" { Suffix::Apply(args) }

		rule expr() -> Expr
			= a:precedence! {
				a:(@) __() "||" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Or, Box::new(b))}
				--
				a:(@) __() "&&" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::And, Box::new(b))}
				--
				a:(@) __() "|" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::BitOr, Box::new(b))}
				--
				a:@ __() "^" __() b:(@) {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::BitXor, Box::new(b))}
				--
				a:(@) __() "&" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::BitAnd, Box::new(b))}
				--
				a:(@) __() "==" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Eq, Box::new(b))}
				a:(@) __() "!=" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Ne, Box::new(b))}
				--
				a:(@) __() "<" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Lt, Box::new(b))}
				a:(@) __() ">" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Gt, Box::new(b))}
				a:(@) __() "<=" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Lte, Box::new(b))}
				a:(@) __() ">=" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Gte, Box::new(b))}
				--
				a:(@) __() "<<" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Lhs, Box::new(b))}
				a:(@) __() ">>" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Rhs, Box::new(b))}
				--
				a:(@) __() "+" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Add, Box::new(b))}
				a:(@) __() "-" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Sub, Box::new(b))}
				--
				a:(@) __() "*" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Mul, Box::new(b))}
				a:(@) __() "/" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Div, Box::new(b))}
				a:(@) __() "%" __() b:@ {Expr::BinaryOp(Box::new(a), expr::BinaryOpType::Mod, Box::new(b))}
				--
				e:expr_basic_with_suffix() {e}
				"(" __() e:boxed_expr() __() ")" {Expr::Parened(e)}
			}
			/ e:expr_basic_with_suffix() {e}

		pub rule boxed_expr() -> Box<Expr> = e:expr() {Box::new(e)}
		pub rule jsonnet() -> Expr = __() e:expr() __() {e}
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

	#[test]
	fn suffix_comparsion() {
		use Expr::*;
		assert_eq!(
			parse("std.type(a) == \"string\"").unwrap(),
			BinaryOp(
				box Apply(
					box Index(box Var("std".to_owned()), box Str("type".to_owned())),
					ArgsDesc(vec![Arg::Positional(box Var("a".to_owned()))])
				),
				BinaryOpType::Eq,
				box Str("string".to_owned())
			)
		);
	}
}
