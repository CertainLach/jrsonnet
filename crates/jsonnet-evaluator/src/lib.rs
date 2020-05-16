#![feature(box_syntax, box_patterns)]

use jsonnet_parser::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
	Str(String),
	Num(f64),
}

pub fn evaluate(expr: &Expr) -> Val {
	use Expr::*;
	match expr {
		Parened(e) => evaluate(e),
		Str(v) => Val::Str(v.clone()),
		Num(v) => Val::Num(*v),
		BinaryOp(v1, o, v2) => match (evaluate(v1), o, evaluate(v2)) {
			(Val::Str(v1), BinaryOpType::Add, Val::Str(v2)) => Val::Str(v1 + &v2),
			(Val::Str(v1), BinaryOpType::Mul, Val::Num(v2)) => {
				Val::Str(v1.repeat(v2 as usize))
			},
			(Val::Num(v1), BinaryOpType::Add, Val::Num(v2)) => Val::Num(v1 + v2),
			(Val::Num(v1), BinaryOpType::Mul, Val::Num(v2)) => Val::Num(v1 * v2),
			_ => panic!("Can't evaluate binary op: {:?} {:?} {:?}", v1, o, v2),
		},
		_ => panic!("Can't evaluate: {:?}", expr),
	}
}

#[cfg(test)]
pub mod tests {
	use super::{evaluate, Val};
	use jsonnet_parser::parse;
	#[test]
	fn math_evaluation() {
		assert_eq!(evaluate(&parse("2+2*2").unwrap()), Val::Num(6.0));
	}

	#[test]
	fn math_evaluation_with_parened() {
		assert_eq!(evaluate(&parse("3+(2+2*2)").unwrap()), Val::Num(9.0));
	}

	#[test]
	fn string_concat() {
		assert_eq!(
			evaluate(&parse("\"Hello\"+\"World\"").unwrap()),
			Val::Str("HelloWorld".to_owned()),
		);
	}

	#[test]
	fn string_repeat() {
		assert_eq!(
			evaluate(&parse("\"Hello\"*3").unwrap()),
			Val::Str("HelloHelloHello".to_owned()),
		);
	}
}
