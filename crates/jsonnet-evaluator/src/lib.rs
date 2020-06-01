#![feature(box_syntax, box_patterns)]
#![feature(type_alias_impl_trait)]
#![feature(debug_non_exhaustive)]
#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]
mod ctx;
mod dynamic;
mod evaluate;
mod obj;
mod val;

pub use ctx::*;
pub use dynamic::*;
pub use evaluate::*;
use jsonnet_parser::*;
pub use obj::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
pub use val::*;

rc_fn_helper!(
	Binding,
	binding,
	dyn Fn(Option<ObjValue>, Option<ObjValue>) -> Val
);
rc_fn_helper!(
	LazyBinding,
	lazy_binding,
	dyn Fn(Option<ObjValue>, Option<ObjValue>) -> LazyVal
);
rc_fn_helper!(FunctionRhs, function_rhs, dyn Fn(Context) -> Val);
rc_fn_helper!(
	FunctionDefault,
	function_default,
	dyn Fn(Context, LocExpr) -> Val
);

pub struct ExitGuard<'s>(&'s EvaluationState);
impl<'s> Drop for ExitGuard<'s> {
	fn drop(&mut self) {
		self.0.stack.borrow_mut().pop();
	}
}

pub struct EvaluationState {
	pub stack: Rc<RefCell<Vec<LocExpr>>>,
	pub files: Rc<RefCell<HashMap<String, String>>>,
}
impl EvaluationState {
	#[must_use = "should keep exit guard before exit from function"]
	pub fn push(&self, e: LocExpr) -> ExitGuard {
		self.stack.borrow_mut().push(e);
		ExitGuard(self)
	}
	pub fn print_stack_trace(&self) {
		for e in self
			.stack
			.borrow()
			.iter()
			.rev()
			.map(|e| e.1.clone())
			.flatten()
		{
			println!("{:?}", e)
		}
	}
}
impl Default for EvaluationState {
	fn default() -> Self {
		EvaluationState {
			stack: Rc::new(RefCell::new(Vec::new())),
			files: Rc::new(RefCell::new(HashMap::new())),
		}
	}
}

#[cfg(test)]
pub mod tests {
	use super::{evaluate, Context, Val};
	use crate::EvaluationState;
	use jsonnet_parser::*;

	#[test]
	fn eval_state_stacktrace() {
		let state = EvaluationState::default();
		let _v = state.push(loc_expr!(
			Expr::Num(0.0),
			true,
			("test.jsonnet".to_owned(), 10, 20)
		));

		state.print_stack_trace()
	}

	macro_rules! eval {
		($str: expr) => {
			evaluate(
				Context::new(),
				&parse(
					$str,
					&ParserSettings {
						loc_data: true,
						file_name: "test.jsonnet".to_owned(),
					},
					)
				.unwrap(),
				)
		};
	}

	macro_rules! eval_stdlib {
		($str: expr) => {{
			let std = "local std = ".to_owned() + jsonnet_stdlib::STDLIB_STR + ";";
			evaluate(
				Context::new(),
				&parse(
					&(std + $str),
					&ParserSettings {
						loc_data: true,
						file_name: "test.jsonnet".to_owned(),
					},
					)
				.unwrap(),
				)
			}};
	}

	macro_rules! assert_eval {
		($str: expr) => {
			assert_eq!(
				evaluate(
					Context::new(),
					&parse(
						$str,
						&ParserSettings {
							loc_data: true,
							file_name: "test.jsonnet".to_owned(),
						}
						)
					.unwrap()
					),
				Val::Bool(true)
				)
		};
	}
	macro_rules! assert_json {
		($str: expr, $out: expr) => {
			assert_eq!(
				format!(
					"{}",
					evaluate(
						Context::new(),
						&parse(
							$str,
							&ParserSettings {
								loc_data: true,
								file_name: "test.jsonnet".to_owned(),
							}
						)
						.unwrap()
						)
					),
				$out
				)
		};
	}
	macro_rules! assert_json_stdlib {
		($str: expr, $out: expr) => {
			assert_eq!(format!("{}", eval_stdlib!($str)), $out)
		};
	}
	macro_rules! assert_eval_neg {
		($str: expr) => {
			assert_eq!(
				evaluate(
					Context::new(),
					&parse(
						$str,
						&ParserSettings {
							loc_data: true,
							file_name: "test.jsonnet".to_owned(),
						}
						)
					.unwrap()
					),
				Val::Bool(false)
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
		assert_json!("{a: self.b} + {b:3}", r#"{"a":3,"b":3}"#);
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

	#[test]
	fn direct_self() {
		println!(
			"{:#?}",
			eval!(
				r#"
					{
						local me = self,
						a: 3,
						b(): me.a,
					}
				"#
			)
		);
	}

	#[test]
	fn indirect_self() {
		// `self` assigned to `me` was lost when being
		// referenced from field
		eval_stdlib!(
			r#"{
				local me = self,
				a: 3,
				b: me.a,
			}.b"#
		);
	}

	// We can't trust other tests (And official jsonnet testsuite), if assert is not working correctly
	#[test]
	fn std_assert_ok() {
		eval_stdlib!("std.assertEqual(4.5 << 2, 16)");
	}

	#[test]
	#[should_panic]
	fn std_assert_failure() {
		eval_stdlib!("std.assertEqual(4.5 << 2, 15)");
	}

	#[test]
	fn string_is_string() {
		assert_eq!(
			eval_stdlib!("local arr = 'hello'; (!std.isArray(arr)) && (!std.isString(arr))"),
			Val::Bool(false)
		);
	}

	#[test]
	fn base64_works() {
		assert_json_stdlib!(r#"std.base64("test")"#, r#""dGVzdA==""#);
	}

	#[test]
	fn utf8_chars() {
		assert_json_stdlib!(
			r#"local c="😎";{c:std.codepoint(c),l:std.length(c)}"#,
			r#"{"c":128526,"l":1}"#
		)
	}

	#[test]
	fn json() {
		println!("{:?}", eval_stdlib!(r#"std.manifestJson({a:3, b:4, c:6})"#));
	}

	#[test]
	fn sjsonnet() {
		eval!(
			r#"
			local x0 = {k: 1};
			local x1 = {k: x0.k + x0.k};
			local x2 = {k: x1.k + x1.k};
			local x3 = {k: x2.k + x2.k};
			local x4 = {k: x3.k + x3.k};
			local x5 = {k: x4.k + x4.k};
			local x6 = {k: x5.k + x5.k};
			local x7 = {k: x6.k + x6.k};
			local x8 = {k: x7.k + x7.k};
			local x9 = {k: x8.k + x8.k};
			local x10 = {k: x9.k + x9.k};
			local x11 = {k: x10.k + x10.k};
			local x12 = {k: x11.k + x11.k};
			local x13 = {k: x12.k + x12.k};
			local x14 = {k: x13.k + x13.k};
			local x15 = {k: x14.k + x14.k};
			local x16 = {k: x15.k + x15.k};
			local x17 = {k: x16.k + x16.k};
			local x18 = {k: x17.k + x17.k};
			local x19 = {k: x18.k + x18.k};
			local x20 = {k: x19.k + x19.k};
			local x21 = {k: x20.k + x20.k};
			x21.k
		"#
		);
	}
}
