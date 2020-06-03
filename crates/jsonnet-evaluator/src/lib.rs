#![feature(box_syntax, box_patterns)]
#![feature(type_alias_impl_trait)]
#![feature(debug_non_exhaustive)]
#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]
mod ctx;
mod dynamic;
mod error;
mod evaluate;
mod obj;
mod val;

use closure::closure;
pub use ctx::*;
pub use dynamic::*;
pub use error::*;
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

#[derive(Default)]
pub struct EvaluationStateInternals {
	/// Used for stack-overflows and stacktraces
	stack: RefCell<Vec<(LocExpr, String)>>,
	/// Contains file source codes and evaluated results for imports and pretty printing stacktraces
	files: RefCell<HashMap<String, (String, LocExpr, Option<Val>)>>,
	globals: RefCell<HashMap<String, Val>>,
}

#[derive(Default, Clone)]
pub struct EvaluationState(Rc<EvaluationStateInternals>);
impl EvaluationState {
	pub fn add_file(&self, name: String, code: String) -> Result<(), Box<dyn std::error::Error>> {
		self.0.files.borrow_mut().insert(
			name.clone(),
			(
				code.clone(),
				parse(
					&code,
					&ParserSettings {
						file_name: name.clone(),
						loc_data: true,
					},
				)?,
				None,
			),
		);

		Ok(())
	}
	pub fn evaluate_file(&self, name: &str) -> Result<Val, Box<dyn std::error::Error>> {
		let expr: LocExpr = {
			let ro_map = self.0.files.borrow();
			let value = ro_map
				.get(name)
				.unwrap_or_else(|| panic!("file not added: {:?}", name));
			if value.2.is_some() {
				return Ok(value.2.clone().unwrap());
			}
			value.1.clone()
		};
		let value = evaluate(self.create_default_context(), self.clone(), &expr);
		{
			self.0
				.files
				.borrow_mut()
				.get_mut(name)
				.unwrap()
				.2
				.replace(value.clone());
		}
		Ok(value)
	}

	pub fn parse_evaluate_raw(&self, code: &str) -> Val {
		let parsed = parse(
			&code,
			&ParserSettings {
				file_name: "raw.jsonnet".to_owned(),
				loc_data: true,
			},
		);
		evaluate(
			self.create_default_context(),
			self.clone(),
			&parsed.unwrap(),
		)
	}

	pub fn add_stdlib(&self) {
		use jsonnet_stdlib::STDLIB_STR;
		self.add_file("std.jsonnet".to_owned(), STDLIB_STR.to_owned())
			.unwrap();
		let val = self.evaluate_file("std.jsonnet").unwrap();
		self.0.globals.borrow_mut().insert("std".to_owned(), val);
	}

	pub fn create_default_context(&self) -> Context {
		let globals = self.0.globals.borrow();
		let mut new_bindings: HashMap<String, LazyBinding> = HashMap::new();
		for (name, value) in globals.iter() {
			new_bindings.insert(
				name.clone(),
				lazy_binding!(
					closure!(clone value, |_self, _super_obj| lazy_val!(closure!(clone value, ||value.clone())))
				),
			);
		}
		Context::new().extend(new_bindings, None, None, None)
	}

	pub fn push<T>(&self, e: LocExpr, comment: String, f: impl FnOnce() -> T) -> T {
		self.0.stack.borrow_mut().push((e, comment));
		let result = f();
		self.0.stack.borrow_mut().pop();
		result
	}
	pub fn print_stack_trace(&self) {
		for e in self.stack_trace() {
			println!("{:?} - {:?}", e.0, e.1)
		}
	}
	pub fn stack_trace(&self) -> Vec<(LocExpr, String)> {
		self.0
			.stack
			.borrow()
			.iter()
			.rev()
			.map(|e| e.clone())
			.collect()
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
		state.push(
			loc_expr!(Expr::Num(0.0), true, ("test1.jsonnet".to_owned(), 10, 20)),
			"outer".to_owned(),
			|| {
				state.push(
					loc_expr!(Expr::Num(0.0), true, ("test2.jsonnet".to_owned(), 30, 40)),
					"inner".to_owned(),
					|| state.print_stack_trace(),
				);
			},
		);
	}

	#[test]
	fn eval_state_standard() {
		let state = EvaluationState::default();
		state.add_stdlib();
		assert_eq!(
			state.parse_evaluate_raw(r#"std.base64("test") == "dGVzdA==""#),
			Val::Bool(true)
		);
	}

	macro_rules! eval {
		($str: expr) => {
			evaluate(
				Context::new(),
				EvaluationState::default(),
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
				EvaluationState::default(),
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
					EvaluationState::default(),
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
						EvaluationState::default(),
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
					EvaluationState::default(),
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
		assert_json_stdlib!(
			r#"std.manifestJsonEx({a:3, b:4, c:6},"")"#,
			r#""{\n"a": 3,\n"b": 4,\n"c": 6\n}""#
		);
	}

	#[test]
	fn test() {
		assert_json_stdlib!(
			r#"[[a, b] for a in [1,2,3] for b in [4,5,6]]"#,
			"[[1,4],[1,5],[1,6],[2,4],[2,5],[2,6],[3,4],[3,5],[3,6]]"
		);
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
