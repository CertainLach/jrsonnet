#![feature(box_syntax, box_patterns)]
#![feature(type_alias_impl_trait)]
#![feature(debug_non_exhaustive)]

mod binding;
mod ctx;
mod dynamic;
mod evaluate;
mod obj;
mod val;

pub use binding::*;
pub use ctx::*;
pub use dynamic::*;
pub use evaluate::*;
use jsonnet_parser::*;
pub use obj::*;
use std::fmt::Debug;
use std::rc::Rc;
pub use val::*;

pub trait FunctionRhs: Debug {
	fn evaluate(&self, ctx: Context) -> Val;
}
dynamic_wrapper!(FunctionRhs, BoxedFunctionRhs);

pub trait FunctionDefault: Debug {
	fn default(&self, ctx: Context, expr: Expr) -> Val;
}
dynamic_wrapper!(FunctionDefault, BoxedFunctionDefault);

#[cfg(test)]
pub mod tests {
	use super::{evaluate, Context, Val};
	use jsonnet_parser::*;

	// macro_rules! eval {
	// 	($str: expr) => {
	// 		evaluate(Context::new(), &parse($str).unwrap())
	// 	};
	// }
	macro_rules! assert_eval {
		($str: expr) => {
			assert_eq!(
				evaluate(Context::new(), &parse($str).unwrap()),
				Val::Literal(LiteralType::True)
				)
		};
	}
	macro_rules! assert_json {
		($str: expr, $out: expr) => {
			assert_eq!(
				format!("{}", evaluate(Context::new(), &parse($str).unwrap())),
				$out
				)
		};
	}
	macro_rules! assert_eval_neg {
		($str: expr) => {
			assert_eq!(
				evaluate(Context::new(), &parse($str).unwrap()),
				Val::Literal(LiteralType::False)
				)
		};
	}

	/// Sanity checking, before trusting to another tests
	#[test]
	fn equality_operator() {
		assert_eval!("2 == 2");
		assert_eval_neg!("2 != 2");
		assert_eval!("2 != 3");
		assert_eval_neg!("2 == 3");
		assert_eval!("'Hello' == 'Hello'");
		assert_eval_neg!("'Hello' != 'Hello'");
		assert_eval!("'Hello' != 'World'");
		assert_eval_neg!("'Hello' == 'World'");
	}

	#[test]
	fn math_evaluation() {
		assert_eval!("2 + 2 * 2 == 6");
		assert_eval!("3 + (2 + 2 * 2) == 9");
	}

	#[test]
	fn string_concat() {
		assert_eval!("'Hello' + 'World' == 'HelloWorld'");
		assert_eval!("'Hello' * 3 == 'HelloHelloHello'");
		assert_eval!("'Hello' + 'World' * 3 == 'HelloWorldWorldWorld'");
	}

	#[test]
	fn local() {
		assert_eval!("local a = 2; local b = 3; a + b == 5");
		assert_eval!("local a = 1, b = a + 1; a + b == 3");
		assert_eval!("local a = 1; local a = 2; a == 2");
	}

	#[test]
	fn object_lazyness() {
		assert_json!("local a = {a:error 'test'}; {}", r#"{}"#);
	}

	#[test]
	fn object_inheritance() {
		assert_json!("{a:self.b} + {b:3}", r#"{"a":3,"b":3}"#);
	}

	#[test]
	fn test_object() {
		assert_json!("{a:2}", r#"{"a":2}"#);
		assert_json!("{a:2+2}", r#"{"a":4}"#);
		assert_json!("{a:2}+{b:2}", r#"{"a":2,"b":2}"#);
		assert_json!("{b:3}+{b:2}", r#"{"b":2}"#);
		assert_json!("{b:3}+{b+:2}", r#"{"b":5}"#);
		assert_json!("local test='a'; {[test]:2}", r#"{"a":2}"#);
		assert_json!(
			r#"
				{
					name: "Alice",
					welcome: "Hello " + self.name + "!",
				}
			"#,
			r#"{"name":"Alice","welcome":"Hello Alice!"}"#
		);
		assert_json!(
			r#"
				{
					name: "Alice",
					welcome: "Hello " + self.name + "!",
				} + {
					name: "Bob"
				}
			"#,
			r#"{"name":"Bob","welcome":"Hello Bob!"}"#
		);
	}

	#[test]
	fn functions() {
		assert_json!(r#"local a = function(b, c = 2) b + c; a(2)"#, "4");
		assert_json!(
			r#"local a = function(b, c = "Dear") b + c + d, d = "World"; a("Hello")"#,
			r#""HelloDearWorld""#
		);
	}

	#[test]
	fn local_methods() {
		assert_json!(r#"local a(b, c = 2) = b + c; a(2)"#, "4");
		assert_json!(
			r#"local a(b, c = "Dear") = b + c + d, d = "World"; a("Hello")"#,
			r#""HelloDearWorld""#
		);
	}

	#[test]
	fn object_locals() {
		assert_json!(r#"{local a = 3, b: a}"#, r#"{"b":3}"#);
		assert_json!(r#"{local a = 3, local c = a, b: c}"#, r#"{"b":3}"#);
		assert_json!(
			r#"{local a = function (b) {[b]:4}, test: a("test")}"#,
			r#"{"test":{"test":4}}"#
		);
	}

	// We can't trust other tests (And official jsonnet testsuite), if assert is not working correctly
	#[test]
	fn std_assert_ok() {
		let std = "local std = ".to_owned() + jsonnet_stdlib::STDLIB_STR + ";";
		evaluate(
			Context::new(),
			&parse(&(std + "std.assertEqual(4.5 << 2, 16,)")).unwrap(),
		);
	}

	#[test]
	#[should_panic]
	fn std_assert_failure() {
		let std = "local std = ".to_owned() + jsonnet_stdlib::STDLIB_STR + ";";
		evaluate(
			Context::new(),
			&parse(&(std + "std.assertEqual(4.5 << 2, 15,)")).unwrap(),
		);
	}
}
