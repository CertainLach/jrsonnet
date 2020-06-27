#![feature(box_syntax, box_patterns)]
#![feature(type_alias_impl_trait)]
#![feature(debug_non_exhaustive)]
#![feature(test)]
#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]

extern crate test;

mod ctx;
mod dynamic;
mod error;
mod evaluate;
mod function;
mod import;
mod map;
mod obj;
mod val;

pub use ctx::*;
pub use dynamic::*;
pub use error::*;
pub use evaluate::*;
pub use function::parse_function_call;
pub use import::*;
use jsonnet_parser::*;
pub use obj::*;
use std::{cell::RefCell, collections::HashMap, fmt::Debug, path::PathBuf, rc::Rc};
pub use val::*;

type BindableFn = dyn Fn(Option<ObjValue>, Option<ObjValue>) -> Result<LazyVal>;
#[derive(Clone)]
pub enum LazyBinding {
	Bindable(Rc<BindableFn>),
	Bound(LazyVal),
}

impl Debug for LazyBinding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "LazyBinding")
	}
}
impl LazyBinding {
	pub fn evaluate(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<LazyVal> {
		match self {
			LazyBinding::Bindable(v) => v(this, super_obj),
			LazyBinding::Bound(v) => Ok(v.clone()),
		}
	}
}

pub struct EvaluationSettings {
	pub max_stack_frames: usize,
	pub max_stack_trace_size: usize,
}
impl Default for EvaluationSettings {
	fn default() -> Self {
		EvaluationSettings {
			max_stack_frames: 200,
			max_stack_trace_size: 20,
		}
	}
}

pub struct FileData(Rc<str>, LocExpr, Option<Val>);
#[derive(Default)]
pub struct EvaluationStateInternals {
	/// Used for stack-overflows and stacktraces
	stack: RefCell<Vec<StackTraceElement>>,
	/// Contains file source codes and evaluated results for imports and pretty
	/// printing stacktraces
	files: RefCell<HashMap<Rc<PathBuf>, FileData>>,
	str_files: RefCell<HashMap<Rc<PathBuf>, Rc<str>>>,
	globals: RefCell<HashMap<Rc<str>, Val>>,

	/// Values to use with std.extVar
	ext_vars: RefCell<HashMap<Rc<str>, Val>>,

	settings: EvaluationSettings,
	import_resolver: Box<dyn ImportResolver>,
}

thread_local! {
	/// Contains state for currently executing file
	/// Global state is fine there
	pub(crate) static EVAL_STATE: RefCell<Option<EvaluationState>> = RefCell::new(None)
}
pub(crate) fn with_state<T>(f: impl FnOnce(&EvaluationState) -> T) -> T {
	EVAL_STATE.with(|s| f(s.borrow().as_ref().unwrap()))
}
pub(crate) fn create_error<T>(err: Error) -> Result<T> {
	with_state(|s| s.error(err))
}
pub(crate) fn push<T>(
	e: &Option<ExprLocation>,
	comment: &str,
	f: impl FnOnce() -> Result<T>,
) -> Result<T> {
	if e.is_some() {
		with_state(|s| s.push(e.clone().unwrap(), comment.to_owned(), f))
	} else {
		f()
	}
}

/// Maintains stack trace and import resolution
#[derive(Default, Clone)]
pub struct EvaluationState(Rc<EvaluationStateInternals>);
impl EvaluationState {
	pub fn new(settings: EvaluationSettings, import_resolver: Box<dyn ImportResolver>) -> Self {
		EvaluationState(Rc::new(EvaluationStateInternals {
			settings,
			import_resolver,
			..Default::default()
		}))
	}
	pub fn add_file(
		&self,
		name: Rc<PathBuf>,
		code: Rc<str>,
	) -> std::result::Result<(), ParseError> {
		self.0.files.borrow_mut().insert(
			name.clone(),
			FileData(
				code.clone(),
				parse(
					&code,
					&ParserSettings {
						file_name: name,
						loc_data: true,
					},
				)?,
				None,
			),
		);

		Ok(())
	}
	pub fn add_parsed_file(
		&self,
		name: Rc<PathBuf>,
		code: Rc<str>,
		parsed: LocExpr,
	) -> std::result::Result<(), ()> {
		self.0
			.files
			.borrow_mut()
			.insert(name, FileData(code, parsed, None));

		Ok(())
	}
	pub fn get_source(&self, name: &PathBuf) -> Option<Rc<str>> {
		let ro_map = self.0.files.borrow();
		ro_map.get(name).map(|value| value.0.clone())
	}
	pub fn evaluate_file(&self, name: &PathBuf) -> Result<Val> {
		self.run_in_state(|| {
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
			let value = evaluate(self.create_default_context()?, &expr)?;
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
		})
	}
	pub(crate) fn import_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Val> {
		let file_path = self.0.import_resolver.resolve_file(from, path)?;
		{
			let files = self.0.files.borrow();
			if files.contains_key(&file_path) {
				return self.evaluate_file(&file_path);
			}
		}
		let contents = self.0.import_resolver.load_file_contents(&file_path)?;
		self.add_file(file_path.clone(), contents).map_err(|e| {
			create_error::<()>(Error::ImportSyntaxError(e))
				.err()
				.unwrap()
		})?;
		self.evaluate_file(&file_path)
	}
	pub(crate) fn import_file_str(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<str>> {
		let path = self.0.import_resolver.resolve_file(from, path)?;
		if !self.0.str_files.borrow().contains_key(&path) {
			let file_str = self.0.import_resolver.load_file_contents(&path)?;
			self.0
				.str_files
				.borrow_mut()
				.insert(path.clone(), file_str.into());
		}
		Ok(self.0.str_files.borrow().get(&path).cloned().unwrap())
	}

	pub fn parse_evaluate_raw(&self, code: &str) -> Result<Val> {
		let parsed = parse(
			&code,
			&ParserSettings {
				file_name: Rc::new(PathBuf::from("raw.jsonnet")),
				loc_data: true,
			},
		)
		.unwrap();
		self.evaluate_raw(parsed)
	}

	pub fn evaluate_raw(&self, code: LocExpr) -> Result<Val> {
		self.run_in_state(|| evaluate(self.create_default_context()?, &code))
	}

	pub fn add_global(&self, name: Rc<str>, value: Val) {
		self.0.globals.borrow_mut().insert(name, value);
	}
	pub fn add_ext_var(&self, name: Rc<str>, value: Val) {
		self.0.ext_vars.borrow_mut().insert(name, value);
	}

	pub fn with_stdlib(&self) -> &Self {
		let std_path = Rc::new(PathBuf::from("std.jsonnet"));
		self.run_in_state(|| {
			use jsonnet_stdlib::STDLIB_STR;
			let mut parsed = false;
			#[cfg(feature = "codegenerated-stdlib")]
			if !parsed {
				parsed = true;
				#[allow(clippy::all)]
				let stdlib = {
					use jsonnet_parser::*;
					include!(concat!(env!("OUT_DIR"), "/stdlib.rs"))
				};
				self.add_parsed_file(std_path.clone(), STDLIB_STR.to_owned().into(), stdlib)
					.unwrap();
			}

			#[cfg(feature = "serialized-stdlib")]
			if !parsed {
				parsed = true;
				self.add_parsed_file(
					std_path.clone(),
					STDLIB_STR.to_owned().into(),
					bincode::deserialize(include_bytes!(concat!(
						env!("OUT_DIR"),
						"/stdlib.bincode"
					)))
					.expect("deserialize stdlib"),
				)
				.unwrap();
			}

			if !parsed {
				self.add_file(std_path, STDLIB_STR.to_owned().into())
					.unwrap();
			}
			let val = self.evaluate_file(&PathBuf::from("std.jsonnet")).unwrap();
			self.add_global("std".into(), val);
		});
		self
	}

	pub fn create_default_context(&self) -> Result<Context> {
		let globals = self.0.globals.borrow();
		let mut new_bindings: HashMap<Rc<str>, LazyBinding> = HashMap::new();
		for (name, value) in globals.iter() {
			new_bindings.insert(
				name.clone(),
				LazyBinding::Bound(resolved_lazy_val!(value.clone())),
			);
		}
		Context::new().extend_unbound(new_bindings, None, None, None)
	}

	pub fn push<T>(
		&self,
		e: ExprLocation,
		comment: String,
		f: impl FnOnce() -> Result<T>,
	) -> Result<T> {
		{
			let mut stack = self.0.stack.borrow_mut();
			if stack.len() > self.0.settings.max_stack_frames {
				drop(stack);
				return self.error(Error::StackOverflow);
			} else {
				stack.push(StackTraceElement(e, comment));
			}
		}
		let result = f();
		self.0.stack.borrow_mut().pop();
		result
	}
	pub fn print_stack_trace(&self) {
		for e in self.stack_trace().0 {
			println!("{:?} - {:?}", e.0, e.1)
		}
	}
	pub fn stack_trace(&self) -> StackTrace {
		StackTrace(
			self.0
				.stack
				.borrow()
				.iter()
				.rev()
				.take(self.0.settings.max_stack_trace_size)
				.cloned()
				.collect(),
		)
	}
	pub fn error<T>(&self, err: Error) -> Result<T> {
		Err(LocError(err, self.stack_trace()))
	}

	pub fn run_in_state<T>(&self, f: impl FnOnce() -> T) -> T {
		EVAL_STATE.with(|v| {
			let has_state = v.borrow().is_some();
			if !has_state {
				v.borrow_mut().replace(self.clone());
			}
			let result = f();
			if !has_state {
				v.borrow_mut().take();
			}
			result
		})
	}
}

#[cfg(test)]
pub mod tests {
	use super::Val;
	use crate::EvaluationState;
	use jsonnet_parser::*;
	use std::{path::PathBuf, rc::Rc};

	#[test]
	fn eval_state_stacktrace() {
		let state = EvaluationState::default();
		state
			.push(
				ExprLocation(Rc::new(PathBuf::from("test1.jsonnet")), 10, 20),
				"outer".to_owned(),
				|| {
					state.push(
						ExprLocation(Rc::new(PathBuf::from("test2.jsonnet")), 30, 40),
						"inner".to_owned(),
						|| {
							state.print_stack_trace();
							Ok(())
						},
					)?;
					Ok(())
				},
			)
			.unwrap();
	}

	#[test]
	fn eval_state_standard() {
		let state = EvaluationState::default();
		state.with_stdlib();
		assert_eq!(
			state
				.parse_evaluate_raw(r#"std.assertEqual(std.base64("test"), "dGVzdA==")"#)
				.unwrap(),
			Val::Bool(true)
		);
	}

	macro_rules! eval {
		($str: expr) => {
			EvaluationState::default()
				.with_stdlib()
				.parse_evaluate_raw($str)
				.unwrap()
		};
	}
	macro_rules! eval_json {
		($str: expr) => {{
			let evaluator = EvaluationState::default();
			evaluator.with_stdlib();
			evaluator.run_in_state(||{
				evaluator
					.parse_evaluate_raw($str)
					.unwrap()
					.into_json(0)
					.unwrap()
					.replace("\n", "")
			})
		}}
	}

	/// Asserts given code returns `true`
	macro_rules! assert_eval {
		($str: expr) => {
			assert_eq!(eval!($str), Val::Bool(true))
		};
	}

	/// Asserts given code returns `false`
	macro_rules! assert_eval_neg {
		($str: expr) => {
			assert_eq!(eval!($str), Val::Bool(false))
		};
	}
	macro_rules! assert_json {
		($str: expr, $out: expr) => {
			assert_eq!(eval_json!($str), $out.replace("\t", ""))
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
	fn faster_join() {
		assert_eval!("std.join([0,0], [[1,2],[3,4],[5,6]]) == [1,2,0,0,3,4,0,0,5,6]");
		assert_eval!("std.join(',', ['1','2','3','4']) == '1,2,3,4'");
	}

	#[test]
	fn function_contexts() {
		assert_eval!(
			r#"
				local k = {
					t(name = self.h): [self.h, name],
					h: 3,
				};
				local f = {
					t: k.t(),
					h: 4,
				};
				f.t[0] == f.t[1]
			"#
		);
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
		assert_json!("{a: self.b} + {b:3}", r#"{"a": 3,"b": 3}"#);
	}

	#[test]
	fn object_assertion_success() {
		eval!("{assert \"a\" in self} + {a:2}");
	}

	#[test]
	fn object_assertion_error() {
		eval!("{assert \"a\" in self}");
	}

	#[test]
	fn lazy_args() {
		eval!("local test(a) = 2; test(error '3')");
	}

	#[test]
	#[should_panic]
	fn tailstrict_args() {
		eval!("local test(a) = 2; test(error '3') tailstrict");
	}

	#[test]
	#[should_panic]
	fn no_binding_error() {
		eval!("a");
	}

	#[test]
	fn test_object() {
		assert_json!("{a:2}", r#"{"a": 2}"#);
		assert_json!("{a:2+2}", r#"{"a": 4}"#);
		assert_json!("{a:2}+{b:2}", r#"{"a": 2,"b": 2}"#);
		assert_json!("{b:3}+{b:2}", r#"{"b": 2}"#);
		assert_json!("{b:3}+{b+:2}", r#"{"b": 5}"#);
		assert_json!("local test='a'; {[test]:2}", r#"{"a": 2}"#);
		assert_json!(
			r#"
				{
					name: "Alice",
					welcome: "Hello " + self.name + "!",
				}
			"#,
			r#"{"name": "Alice","welcome": "Hello Alice!"}"#
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
			r#"{"name": "Bob","welcome": "Hello Bob!"}"#
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
		assert_json!(r#"{local a = 3, b: a}"#, r#"{"b": 3}"#);
		assert_json!(r#"{local a = 3, local c = a, b: c}"#, r#"{"b": 3}"#);
		assert_json!(
			r#"{local a = function (b) {[b]:4}, test: a("test")}"#,
			r#"{"test": {"test": 4}}"#
		);
	}

	#[test]
	fn object_comp() {
		assert_json!(
			r#"{local t = "a", ["h"+i+"_"+z]: if "h"+(i-1)+"_"+z in self then t+1 else 0+t for i in [1,2,3] for z in [2,3,4] if z != i}"#,
			"{\"h1_2\": \"0a\",\"h1_3\": \"0a\",\"h1_4\": \"0a\",\"h2_3\": \"a1\",\"h2_4\": \"a1\",\"h3_2\": \"0a\",\"h3_4\": \"a1\"}"
		)
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
		eval!(
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
		eval!("std.assertEqual(4.5 << 2, 16)");
	}

	#[test]
	#[should_panic]
	fn std_assert_failure() {
		eval!("std.assertEqual(4.5 << 2, 15)");
	}

	#[test]
	fn string_is_string() {
		assert_eq!(
			eval!("local arr = 'hello'; (!std.isArray(arr)) && (!std.isString(arr))"),
			Val::Bool(false)
		);
	}

	#[test]
	fn base64_works() {
		assert_json!(r#"std.base64("test")"#, r#""dGVzdA==""#);
	}

	#[test]
	fn utf8_chars() {
		assert_json!(
			r#"local c="😎";{c:std.codepoint(c),l:std.length(c)}"#,
			r#"{"c": 128526,"l": 1}"#
		)
	}

	#[test]
	fn json() {
		assert_json!(
			r#"std.manifestJsonEx({a:3, b:4, c:6},"")"#,
			r#""{\n\"a\": 3,\n\"b\": 4,\n\"c\": 6\n}""#
		);
	}

	#[test]
	fn test() {
		assert_json!(
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

	use test::Bencher;

	// This test is commented out by default, because of huge compilation slowdown
	// #[bench]
	// fn bench_codegen(b: &mut Bencher) {
	// 	b.iter(|| {
	// 		#[allow(clippy::all)]
	// 		let stdlib = {
	// 			use jsonnet_parser::*;
	// 			include!(concat!(env!("OUT_DIR"), "/stdlib.rs"))
	// 		};
	// 		stdlib
	// 	})
	// }

	#[bench]
	fn bench_serialize(b: &mut Bencher) {
		b.iter(|| {
			bincode::deserialize::<jsonnet_parser::LocExpr>(include_bytes!(concat!(
				env!("OUT_DIR"),
				"/stdlib.bincode"
			)))
			.expect("deserialize stdlib")
		})
	}

	#[bench]
	fn bench_parse(b: &mut Bencher) {
		b.iter(|| {
			jsonnet_parser::parse(
				jsonnet_stdlib::STDLIB_STR,
				&jsonnet_parser::ParserSettings {
					loc_data: true,
					file_name: Rc::new(PathBuf::from("std.jsonnet")),
				},
			)
		})
	}
}
