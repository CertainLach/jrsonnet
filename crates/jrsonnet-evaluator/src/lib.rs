#![cfg_attr(feature = "unstable", feature(stmt_expr_attributes))]
#![warn(clippy::all, clippy::nursery)]
#![allow(
	macro_expanded_macro_exports_accessed_by_absolute_paths,
	clippy::ptr_arg
)]

mod builtin;
mod ctx;
mod dynamic;
pub mod error;
mod evaluate;
mod function;
mod import;
mod integrations;
mod map;
pub mod native;
mod obj;
pub mod trace;
pub mod typed;
mod val;

pub use ctx::*;
pub use dynamic::*;
use error::{Error::*, LocError, Result, StackTraceElement};
pub use evaluate::*;
pub use function::parse_function_call;
pub use import::*;
use jrsonnet_gc::{Finalize, Gc, Trace};
pub use jrsonnet_interner::IStr;
use jrsonnet_parser::*;
use native::NativeCallback;
pub use obj::*;
use rustc_hash::FxHashMap;
use std::{
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	fmt::Debug,
	hash::BuildHasherDefault,
	path::{Path, PathBuf},
	rc::Rc,
};
use trace::{location_to_offset, offset_to_location, CodeLocation, CompactFormat, TraceFormat};
pub use val::*;

pub trait Bindable: Trace {
	fn bind(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<LazyVal>;
}
#[derive(Trace, Finalize, Clone)]
pub enum LazyBinding {
	Bindable(Gc<Box<dyn Bindable>>),
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
			Self::Bindable(v) => v.bind(this, super_obj),
			Self::Bound(v) => Ok(v.clone()),
		}
	}
}

pub struct EvaluationSettings {
	/// Limits recursion by limiting the number of stack frames
	pub max_stack: usize,
	/// Limits amount of stack trace items preserved
	pub max_trace: usize,
	/// Used for s`td.extVar`
	pub ext_vars: HashMap<IStr, Val>,
	/// Used for ext.native
	pub ext_natives: HashMap<IStr, Gc<NativeCallback>>,
	/// TLA vars
	pub tla_vars: HashMap<IStr, Val>,
	/// Global variables are inserted in default context
	pub globals: HashMap<IStr, Val>,
	/// Used to resolve file locations/contents
	pub import_resolver: Box<dyn ImportResolver>,
	/// Used in manifestification functions
	pub manifest_format: ManifestFormat,
	/// Used for bindings
	pub trace_format: Box<dyn TraceFormat>,
}
impl Default for EvaluationSettings {
	fn default() -> Self {
		Self {
			max_stack: 200,
			max_trace: 20,
			globals: Default::default(),
			ext_vars: Default::default(),
			ext_natives: Default::default(),
			tla_vars: Default::default(),
			import_resolver: Box::new(DummyImportResolver),
			manifest_format: ManifestFormat::Json(4),
			trace_format: Box::new(CompactFormat {
				padding: 4,
				resolver: trace::PathResolver::Absolute,
			}),
		}
	}
}

#[derive(Default)]
struct EvaluationData {
	/// Used for stack overflow detection, stacktrace is populated on unwind
	stack_depth: usize,
	/// Updated every time stack entry is popt
	stack_generation: usize,

	breakpoints: Breakpoints,
	/// Contains file source codes and evaluation results for imports and pretty-printed stacktraces
	files: HashMap<Rc<Path>, FileData>,
	str_files: HashMap<Rc<Path>, IStr>,
}

pub struct FileData {
	source_code: IStr,
	parsed: LocExpr,
	evaluated: Option<Val>,
}

pub struct Breakpoint {
	loc: ExprLocation,
	collected: RefCell<HashMap<usize, (usize, Vec<Result<Val>>)>>,
}
#[derive(Default)]
struct Breakpoints(Vec<Rc<Breakpoint>>);
impl Breakpoints {
	fn insert(
		&self,
		stack_depth: usize,
		stack_generation: usize,
		loc: &ExprLocation,
		result: Result<Val>,
	) -> Result<Val> {
		if self.0.is_empty() {
			return result;
		}
		for item in self.0.iter() {
			if item.loc.belongs_to(loc) {
				let mut collected = item.collected.borrow_mut();
				let (depth, vals) = collected.entry(stack_generation).or_default();
				if stack_depth > *depth {
					vals.clear();
				}
				vals.push(result.clone());
			}
		}
		result
	}
}

#[derive(Default)]
pub struct EvaluationStateInternals {
	/// Internal state
	data: RefCell<EvaluationData>,
	/// Settings, safe to change at runtime
	settings: RefCell<EvaluationSettings>,
}

thread_local! {
	/// Contains the state for a currently executed file.
	/// Global state is fine here.
	pub(crate) static EVAL_STATE: RefCell<Option<EvaluationState>> = RefCell::new(None)
}
pub(crate) fn with_state<T>(f: impl FnOnce(&EvaluationState) -> T) -> T {
	EVAL_STATE.with(|s| f(s.borrow().as_ref().unwrap()))
}
pub(crate) fn push_frame<T>(
	e: Option<&ExprLocation>,
	frame_desc: impl FnOnce() -> String,
	f: impl FnOnce() -> Result<T>,
) -> Result<T> {
	with_state(|s| s.push(e, frame_desc, f))
}

pub(crate) fn push_val_frame(
	e: Option<&ExprLocation>,
	frame_desc: impl FnOnce() -> String,
	f: impl FnOnce() -> Result<Val>,
) -> Result<Val> {
	with_state(|s| s.push(e, frame_desc, f))
}

/// Maintains stack trace and import resolution
#[derive(Default, Clone)]
pub struct EvaluationState(Rc<EvaluationStateInternals>);

impl EvaluationState {
	/// Parses and adds file as loaded
	pub fn add_file(&self, path: Rc<Path>, source_code: IStr) -> Result<()> {
		self.add_parsed_file(
			path.clone(),
			source_code.clone(),
			parse(
				&source_code,
				&ParserSettings {
					file_name: path.clone(),
					loc_data: true,
				},
			)
			.map_err(|error| ImportSyntaxError {
				error: Box::new(error),
				path: path.to_owned(),
				source_code,
			})?,
		)?;

		Ok(())
	}

	pub fn reset_evaluation_state(&self, name: &Path) {
		self.data_mut()
			.files
			.get_mut(name)
			.unwrap()
			.evaluated
			.take();
	}

	/// Adds file by source code and parsed expr
	pub fn add_parsed_file(
		&self,
		name: Rc<Path>,
		source_code: IStr,
		parsed: LocExpr,
	) -> Result<()> {
		self.data_mut().files.insert(
			name,
			FileData {
				source_code,
				parsed,
				evaluated: None,
			},
		);

		Ok(())
	}
	pub fn get_source(&self, name: &Path) -> Option<IStr> {
		let ro_map = &self.data().files;
		ro_map.get(name).map(|value| value.source_code.clone())
	}
	pub fn map_source_locations(&self, file: &Path, locs: &[usize]) -> Vec<CodeLocation> {
		offset_to_location(&self.get_source(file).unwrap(), locs)
	}
	pub fn map_from_source_location(
		&self,
		file: &Path,
		line: usize,
		column: usize,
	) -> Option<usize> {
		location_to_offset(&self.get_source(file).unwrap(), line, column)
	}
	pub fn import_file(&self, from: &Path, path: &Path) -> Result<Val> {
		let file_path = self.resolve_file(from, path)?;
		{
			let data = self.data();
			let files = &data.files;
			if files.contains_key(&file_path as &Path) {
				drop(data);
				return self.evaluate_loaded_file_raw(&file_path);
			}
		}
		let contents = self.load_file_contents(&file_path)?;
		self.add_file(file_path.clone(), contents)?;
		self.evaluate_loaded_file_raw(&file_path)
	}
	pub(crate) fn import_file_str(&self, from: &Path, path: &Path) -> Result<IStr> {
		let path = self.resolve_file(from, path)?;
		if !self.data().str_files.contains_key(&path) {
			let file_str = self.load_file_contents(&path)?;
			self.data_mut().str_files.insert(path.clone(), file_str);
		}
		Ok(self.data().str_files.get(&path).cloned().unwrap())
	}

	fn evaluate_loaded_file_raw(&self, name: &Path) -> Result<Val> {
		let expr: LocExpr = {
			let ro_map = &self.data().files;
			let value = ro_map
				.get(name)
				.unwrap_or_else(|| panic!("file not added: {:?}", name));
			if let Some(ref evaluated) = value.evaluated {
				return Ok(evaluated.clone());
			}
			value.parsed.clone()
		};
		let value = evaluate(self.create_default_context(), &expr)?;
		{
			self.data_mut()
				.files
				.get_mut(name)
				.unwrap()
				.evaluated
				.replace(value.clone());
		}
		Ok(value)
	}

	/// Adds standard library global variable (std) to this evaluator
	pub fn with_stdlib(&self) -> &Self {
		use jrsonnet_stdlib::STDLIB_STR;
		let std_path: Rc<Path> = PathBuf::from("std.jsonnet").into();
		self.run_in_state(|| {
			self.add_parsed_file(
				std_path.clone(),
				STDLIB_STR.to_owned().into(),
				builtin::get_parsed_stdlib(),
			)
			.unwrap();
			let val = self.evaluate_loaded_file_raw(&std_path).unwrap();
			self.settings_mut().globals.insert("std".into(), val);
		});
		self
	}

	/// Creates context with all passed global variables
	pub fn create_default_context(&self) -> Context {
		let globals = &self.settings().globals;
		let mut new_bindings: FxHashMap<IStr, LazyVal> =
			FxHashMap::with_capacity_and_hasher(globals.len(), BuildHasherDefault::default());
		for (name, value) in globals.iter() {
			new_bindings.insert(name.clone(), LazyVal::new_resolved(value.clone()));
		}
		Context::new().extend_bound(new_bindings)
	}

	/// Executes code creating a new stack frame
	pub fn push<T>(
		&self,
		e: Option<&ExprLocation>,
		frame_desc: impl FnOnce() -> String,
		f: impl FnOnce() -> Result<T>,
	) -> Result<T> {
		{
			let mut data = self.data_mut();
			let stack_depth = &mut data.stack_depth;
			if *stack_depth > self.max_stack() {
				// Error creation uses data, so i drop guard here
				drop(data);
				throw!(StackOverflow);
			} else {
				*stack_depth += 1;
			}
		}
		let result = f();
		{
			let mut data = self.data_mut();
			data.stack_depth -= 1;
			data.stack_generation += 1;
			// if let Some(e) = e {
			// 	result =
			// 		data.breakpoints
			// 			.insert(data.stack_depth, data.stack_generation, &e, result)
			// }
		}
		if let Err(mut err) = result {
			err.trace_mut().0.push(StackTraceElement {
				location: e.cloned(),
				desc: frame_desc(),
			});
			return Err(err);
		}
		result
	}
	/// Executes code creating a new stack frame
	pub fn push_val(
		&self,
		e: Option<&ExprLocation>,
		frame_desc: impl FnOnce() -> String,
		f: impl FnOnce() -> Result<Val>,
	) -> Result<Val> {
		{
			let mut data = self.data_mut();
			let stack_depth = &mut data.stack_depth;
			if *stack_depth > self.max_stack() {
				// Error creation uses data, so i drop guard here
				drop(data);
				throw!(StackOverflow);
			} else {
				*stack_depth += 1;
			}
		}
		let mut result = f();
		{
			let mut data = self.data_mut();
			data.stack_depth -= 1;
			data.stack_generation += 1;
			if let Some(e) = e {
				result =
					data.breakpoints
						.insert(data.stack_depth, data.stack_generation, &e, result)
			}
		}
		if let Err(mut err) = result {
			err.trace_mut().0.push(StackTraceElement {
				location: e.cloned(),
				desc: frame_desc(),
			});
			return Err(err);
		}
		result
	}

	/// Runs passed function in state (required if function needs to modify stack trace)
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
	pub fn run_in_state_with_breakpoint(
		&self,
		bp: Rc<Breakpoint>,
		f: impl FnOnce() -> Result<()>,
	) -> Result<()> {
		{
			let mut data = self.data_mut();
			data.breakpoints.0.push(bp);
		}

		let result = self.run_in_state(f);

		{
			let mut data = self.data_mut();
			data.breakpoints.0.pop();
		}

		result
	}

	pub fn stringify_err(&self, e: &LocError) -> String {
		let mut out = String::new();
		self.settings()
			.trace_format
			.write_trace(&mut out, self, e)
			.unwrap();
		out
	}

	pub fn manifest(&self, val: Val) -> Result<IStr> {
		self.run_in_state(|| val.manifest(&self.manifest_format()))
	}
	pub fn manifest_multi(&self, val: Val) -> Result<Vec<(IStr, IStr)>> {
		self.run_in_state(|| val.manifest_multi(&self.manifest_format()))
	}
	pub fn manifest_stream(&self, val: Val) -> Result<Vec<IStr>> {
		self.run_in_state(|| val.manifest_stream(&self.manifest_format()))
	}

	/// If passed value is function then call with set TLA
	pub fn with_tla(&self, val: Val) -> Result<Val> {
		self.run_in_state(|| {
			Ok(match val {
				Val::Func(func) => push_frame(
					None,
					|| "during TLA call".to_owned(),
					|| {
						func.evaluate_map(
							self.create_default_context(),
							&self.settings().tla_vars,
							true,
						)
					},
				)?,
				v => v,
			})
		})
	}
}

/// Internals
impl EvaluationState {
	fn data(&self) -> Ref<EvaluationData> {
		self.0.data.borrow()
	}
	fn data_mut(&self) -> RefMut<EvaluationData> {
		self.0.data.borrow_mut()
	}
	pub fn settings(&self) -> Ref<EvaluationSettings> {
		self.0.settings.borrow()
	}
	pub fn settings_mut(&self) -> RefMut<EvaluationSettings> {
		self.0.settings.borrow_mut()
	}
}

/// Raw methods evaluate passed values but don't perform TLA execution
impl EvaluationState {
	pub fn evaluate_file_raw(&self, name: &Path) -> Result<Val> {
		self.run_in_state(|| self.import_file(&std::env::current_dir().expect("cwd"), name))
	}
	pub fn evaluate_file_raw_nocwd(&self, name: &Path) -> Result<Val> {
		self.run_in_state(|| self.import_file(&PathBuf::from("."), name))
	}
	/// Parses and evaluates the given snippet
	pub fn evaluate_snippet_raw(&self, source: Rc<Path>, code: IStr) -> Result<Val> {
		let parsed = parse(
			&code,
			&ParserSettings {
				file_name: source.clone(),
				loc_data: true,
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source.clone(),
			source_code: code.clone(),
			error: Box::new(e),
		})?;
		self.add_parsed_file(source, code, parsed.clone())?;
		self.evaluate_expr_raw(parsed)
	}
	/// Evaluates the parsed expression
	pub fn evaluate_expr_raw(&self, code: LocExpr) -> Result<Val> {
		self.run_in_state(|| evaluate(self.create_default_context(), &code))
	}
}

/// Settings utilities
impl EvaluationState {
	pub fn add_ext_var(&self, name: IStr, value: Val) {
		self.settings_mut().ext_vars.insert(name, value);
	}
	pub fn add_ext_str(&self, name: IStr, value: IStr) {
		self.add_ext_var(name, Val::Str(value));
	}
	pub fn add_ext_code(&self, name: IStr, code: IStr) -> Result<()> {
		let value =
			self.evaluate_snippet_raw(PathBuf::from(format!("ext_code {}", name)).into(), code)?;
		self.add_ext_var(name, value);
		Ok(())
	}

	pub fn add_tla(&self, name: IStr, value: Val) {
		self.settings_mut().tla_vars.insert(name, value);
	}
	pub fn add_tla_str(&self, name: IStr, value: IStr) {
		self.add_tla(name, Val::Str(value));
	}
	pub fn add_tla_code(&self, name: IStr, code: IStr) -> Result<()> {
		let value =
			self.evaluate_snippet_raw(PathBuf::from(format!("tla_code {}", name)).into(), code)?;
		self.add_tla(name, value);
		Ok(())
	}

	pub fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>> {
		self.settings().import_resolver.resolve_file(from, path)
	}
	pub fn load_file_contents(&self, path: &Path) -> Result<IStr> {
		self.settings().import_resolver.load_file_contents(path)
	}

	pub fn import_resolver(&self) -> Ref<dyn ImportResolver> {
		Ref::map(self.settings(), |s| &*s.import_resolver)
	}
	pub fn set_import_resolver(&self, resolver: Box<dyn ImportResolver>) {
		self.settings_mut().import_resolver = resolver;
	}

	pub fn add_native(&self, name: IStr, cb: Gc<NativeCallback>) {
		self.settings_mut().ext_natives.insert(name, cb);
	}

	pub fn manifest_format(&self) -> ManifestFormat {
		self.settings().manifest_format.clone()
	}
	pub fn set_manifest_format(&self, format: ManifestFormat) {
		self.settings_mut().manifest_format = format;
	}

	pub fn trace_format(&self) -> Ref<dyn TraceFormat> {
		Ref::map(self.settings(), |s| &*s.trace_format)
	}
	pub fn set_trace_format(&self, format: Box<dyn TraceFormat>) {
		self.settings_mut().trace_format = format;
	}

	pub fn max_trace(&self) -> usize {
		self.settings().max_trace
	}
	pub fn set_max_trace(&self, trace: usize) {
		self.settings_mut().max_trace = trace;
	}

	pub fn max_stack(&self) -> usize {
		self.settings().max_stack
	}
	pub fn set_max_stack(&self, trace: usize) {
		self.settings_mut().max_stack = trace;
	}
}

#[cfg(test)]
pub mod tests {
	use super::Val;
	use crate::{
		error::Error::*, native::NativeCallbackHandler, primitive_equals, EvaluationState,
	};
	use jrsonnet_gc::{Finalize, Gc, Trace};
	use jrsonnet_interner::IStr;
	use jrsonnet_parser::*;
	use std::{
		path::{Path, PathBuf},
		rc::Rc,
	};

	#[test]
	#[should_panic]
	fn eval_state_stacktrace() {
		let state = EvaluationState::default();
		state.run_in_state(|| {
			state
				.push(
					Some(&ExprLocation(PathBuf::from("test1.jsonnet").into(), 10, 20)),
					|| "outer".to_owned(),
					|| {
						state.push(
							Some(&ExprLocation(PathBuf::from("test2.jsonnet").into(), 30, 40)),
							|| "inner".to_owned(),
							|| Err(RuntimeError("".into()).into()),
						)?;
						Ok(Val::Null)
					},
				)
				.unwrap();
		});
	}

	#[test]
	fn eval_state_standard() {
		let state = EvaluationState::default();
		state.with_stdlib();
		assert!(primitive_equals(
			&state
				.evaluate_snippet_raw(
					PathBuf::from("raw.jsonnet").into(),
					r#"std.assertEqual(std.base64("test"), "dGVzdA==")"#.into()
				)
				.unwrap(),
			&Val::Bool(true),
		)
		.unwrap());
	}

	macro_rules! eval {
		($str: expr) => {
			EvaluationState::default()
				.with_stdlib()
				.evaluate_snippet_raw(PathBuf::from("raw.jsonnet").into(), $str.into())
				.unwrap()
		};
	}
	macro_rules! eval_json {
		($str: expr) => {{
			let evaluator = EvaluationState::default();
			evaluator.with_stdlib();
			evaluator.run_in_state(|| {
				evaluator
					.evaluate_snippet_raw(PathBuf::from("raw.jsonnet").into(), $str.into())
					.unwrap()
					.to_json(0)
					.unwrap()
					.replace("\n", "")
			})
		}};
	}

	/// Asserts given code returns `true`
	macro_rules! assert_eval {
		($str: expr) => {
			assert!(primitive_equals(&eval!($str), &Val::Bool(true)).unwrap())
		};
	}

	/// Asserts given code returns `false`
	macro_rules! assert_eval_neg {
		($str: expr) => {
			assert!(primitive_equals(&eval!($str), &Val::Bool(false)).unwrap())
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
		assert!(primitive_equals(
			&eval!("local arr = 'hello'; (!std.isArray(arr)) && (!std.isString(arr))"),
			&Val::Bool(false),
		)
		.unwrap());
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
	fn parse_json() {
		assert_json!(
			r#"std.parseJson('{"a": -1,"b": 1,"c": 3.141,"d": []}')"#,
			r#"{"a": -1,"b": 1,"c": 3.141,"d": []}"#
		);
		// TODO: this should in fact fail as is no proper JSON syntax
		assert_json!(
			r#"std.parseJson("{a:-1, b:1, c:3.141, d:[]}")"#,
			r#"{"a": -1,"b": 1,"c": 3.141,"d": []}"#
		);
		// TODO: this is also no valid JSON
		assert_json!(r#"std.parseJson('local x = 2; x * x')"#, r#"4"#);
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

	// This test is commented out by default, because of huge compilation slowdown
	// #[bench]
	// fn bench_codegen(b: &mut Bencher) {
	// 	b.iter(|| {
	// 		#[allow(clippy::all)]
	// 		let stdlib = {
	// 			use jrsonnet_parser::*;
	// 			include!(concat!(env!("OUT_DIR"), "/stdlib.rs"))
	// 		};
	// 		stdlib
	// 	})
	// }

	/*
	#[bench]
	fn bench_serialize(b: &mut Bencher) {
		b.iter(|| {
			bincode::deserialize::<jrsonnet_parser::LocExpr>(include_bytes!(concat!(
				env!("OUT_DIR"),
				"/stdlib.bincode"
			)))
			.expect("deserialize stdlib")
		})
	}

	#[bench]
	fn bench_parse(b: &mut Bencher) {
		b.iter(|| {
			jrsonnet_parser::parse(
				jrsonnet_stdlib::STDLIB_STR,
				&jrsonnet_parser::ParserSettings {
					loc_data: true,
					file_name: Rc::new(PathBuf::from("std.jsonnet")),
				},
			)
		})
	}
	*/

	#[test]
	fn equality() {
		println!(
			"{:?}",
			jrsonnet_parser::parse(
				"{ x: 1, y: 2 } == { x: 1, y: 2 }",
				&ParserSettings {
					file_name: PathBuf::from("equality").into(),
					loc_data: true,
				}
			)
		);
		assert_eval!("{ x: 1, y: 2 } == { x: 1, y: 2 }")
	}

	#[test]
	fn native_ext() -> crate::error::Result<()> {
		use super::native::NativeCallback;
		let evaluator = EvaluationState::default();

		evaluator.with_stdlib();

		#[derive(Trace, Finalize)]
		struct NativeAdd;
		impl NativeCallbackHandler for NativeAdd {
			fn call(&self, from: Option<Rc<Path>>, args: &[Val]) -> crate::error::Result<Val> {
				assert_eq!(
					&from.unwrap() as &Path,
					&PathBuf::from("native_caller.jsonnet")
				);
				match (&args[0], &args[1]) {
					(Val::Num(a), Val::Num(b)) => Ok(Val::Num(a + b)),
					(_, _) => unreachable!(),
				}
			}
		}
		evaluator.settings_mut().ext_natives.insert(
			"native_add".into(),
			Gc::new(NativeCallback::new(
				ParamsDesc(Rc::new(vec![
					Param("a".into(), None),
					Param("b".into(), None),
				])),
				Box::new(NativeAdd),
			)),
		);
		evaluator.evaluate_snippet_raw(
			PathBuf::from("native_caller.jsonnet").into(),
			"std.assertEqual(std.native(\"native_add\")(1, 2), 3)".into(),
		)?;
		Ok(())
	}

	#[test]
	fn constant_intrinsic() -> crate::error::Result<()> {
		assert_eval!(
			"local std2 = std; local std = std2 { primitiveEquals(a, b):: false }; 1 == 1"
		);
		Ok(())
	}

	#[test]
	fn standalone_super() -> crate::error::Result<()> {
		assert_eval!(
			r#"
			local obj = {
				a: 1,
				b: 2,
				c: 3,
			};
			local test = obj + {
				fields: std.objectFields(super),
				d: 5,
			};
			test.fields == ['a', 'b', 'c']
		"#
		);
		Ok(())
	}

	#[test]
	fn comp_self() -> crate::error::Result<()> {
		assert_eval!(
			r#"
			std.objectFields({
				a:{
					[name]: name for name in std.objectFields(self)
				},
				b: 2,
				c: 3,
			}.a) == ['a', 'b', 'c']
			"#
		);

		Ok(())
	}

	struct TestImportResolver(IStr);
	impl crate::import::ImportResolver for TestImportResolver {
		fn resolve_file(&self, _: &Path, _: &Path) -> crate::error::Result<Rc<Path>> {
			Ok(PathBuf::from("/test").into())
		}

		fn load_file_contents(&self, _: &Path) -> crate::error::Result<IStr> {
			Ok(self.0.clone())
		}

		unsafe fn as_any(&self) -> &dyn std::any::Any {
			panic!()
		}
	}

	#[test]
	fn issue_23() {
		let state = EvaluationState::default();
		state.set_import_resolver(Box::new(TestImportResolver(r#"import "/test""#.into())));
		let _ = state.evaluate_file_raw(&PathBuf::from("/test"));
	}

	#[test]
	fn issue_40() {
		let state = EvaluationState::default();
		state.with_stdlib();

		let error = state
			.evaluate_snippet_raw(
				PathBuf::from("issue40.jsonnet").into(),
				r#"
				local conf = {
					n: ""
				};

				local result = conf + {
					assert std.isNumber(self.n): "is number"
				};

				std.manifestJsonEx(result, "")
			"#
				.into(),
			)
			.unwrap_err();
		assert_eq!(error.error().to_string(), "assert failed: is number");
	}

	#[test]
	fn test_ascii_upper_lower() {
		assert_eval!(r#"std.assertEqual(std.asciiUpper("aBc😀"), "ABC😀")"#);
		assert_eval!(r#"std.assertEqual(std.asciiLower("aBc😀"), "abc😀")"#);
	}

	#[test]
	fn test_member() {
		assert_eval!(r#"!std.member("", "")"#);
		assert_eval!(r#"std.member("abc", "a")"#);
		assert_eval!(r#"!std.member("abc", "d")"#);
		assert_eval!(r#"!std.member([], "")"#);
		assert_eval!(r#"std.member(["a", "b", "c"], "a")"#);
		assert_eval!(r#"!std.member(["a", "b", "c"], "d")"#);
	}

	#[test]
	fn test_count() {
		assert_eval!(r#"std.assertEqual(std.count([], ""), 0)"#);
		assert_eval!(r#"std.assertEqual(std.count(["a", "b", "a"], "d"), 0)"#);
		assert_eval!(r#"std.assertEqual(std.count(["a", "b", "a"], "a"), 2)"#);
	}
}
