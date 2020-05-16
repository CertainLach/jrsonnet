use peg::parser;

mod expr;
pub use expr::*;

enum Suffix {
	String(String),
	Expression(Expr),
	Apply(expr::Args),
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
		rule id() -> String = quiet!{ !("local" / "super" / "self" / "true" / "false" / "null" / "$" / "if" / "then" / "else") s:$(alpha() (alpha() / digit())*) {s.to_owned()}} / expected!("<identifier>")

		pub rule positional_param() -> expr::Param = name:id() {expr::Param::Positional(name)}
		pub rule named_param() -> expr::Param = name:id() __() "=" __() expr:boxed_expr() {expr::Param::Named(name, expr)}
		pub rule params() -> expr::Params
			= positionals:(positional_param() ** delimiter()) delimiter() named:(named_param() ** delimiter()) {
				expr::Params([&positionals[..], &named[..]].concat())
			}
			/ named:(named_param() ** delimiter()) {expr::Params(named)}
			/ positionals:(positional_param() ** delimiter()) {expr::Params(positionals)}
			/ {expr::Params(Vec::new())}

		pub rule positional_arg() -> expr::Arg = quiet!{name:boxed_expr() {expr::Arg::Positional(name)}}/expected!("<positional arg>")
		pub rule named_arg() -> expr::Arg = quiet!{name:id() __() "=" __() expr:boxed_expr() {expr::Arg::Named(name, expr)}}/expected!("<named arg>")
		pub rule args() -> expr::Args
			= positionals:(positional_arg() ** delimiter()) delimiter() named:(named_arg() ** delimiter()) {
				expr::Args([&positionals[..], &named[..]].concat())
			}
			/ named:(named_arg() ** delimiter()) {expr::Args(named)}
			/ positionals:(positional_arg() ** delimiter()) {expr::Args(positionals)}
			/ {expr::Args(Vec::new())}

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
				e:expr_basic() {e}
				"(" __() e:boxed_expr() __() ")" {Expr::Parened(e)}
			} suffixes:(__() suffix:expr_suffix() {suffix})* {
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
			/ e:expr_basic() {e}

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

	mod expressions {
		use super::*;

		pub fn basic_math() -> Expr {
			Expr::BinaryOp(
				Box::new(Expr::Num(2.0)),
				BinaryOp::Add,
				Box::new(Expr::BinaryOp(
					Box::new(Expr::Num(2.0)),
					BinaryOp::Mul,
					Box::new(Expr::Num(2.0)),
				)),
			)
		}
	}

	#[test]
	fn empty_object() {
		assert_eq!(parse("{}").unwrap(), Expr::Obj(ObjBody::MemberList(vec![])));
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
	fn basic_math_with_indents() {
		assert_eq!(parse("2	+ 	  2	  *	2   	").unwrap(), expressions::basic_math());
	}

	#[test]
	fn basic_math_parened() {
		assert_eq!(
			parse("2+(2+2*2)").unwrap(),
			Expr::BinaryOp(
				Box::new(Expr::Num(2.0)),
				BinaryOp::Add,
				Box::new(Expr::Parened(Box::new(expressions::basic_math()))),
			)
		);
	}
}
