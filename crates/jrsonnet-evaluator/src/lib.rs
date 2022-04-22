#![warn(clippy::all, clippy::nursery, clippy::pedantic)]
#![allow(
	macro_expanded_macro_exports_accessed_by_absolute_paths,
	clippy::ptr_arg,
	// Too verbose
	clippy::must_use_candidate,
	// A lot of functions pass around errors thrown by code
	clippy::missing_errors_doc,
	// A lot of pointers have interior Rc
	clippy::needless_pass_by_value,
	// Its fine
	clippy::wildcard_imports,
	clippy::enum_glob_use,
	clippy::module_name_repetitions,
	// TODO: fix individual issues, however this works as intended almost everywhere
	clippy::cast_precision_loss,
	clippy::cast_possible_wrap,
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
)]

// For jrsonnet-macros
extern crate self as jrsonnet_evaluator;

mod builtin;
mod ctx;
mod dynamic;
pub mod error;
mod evaluate;
pub mod function;
pub mod gc;
mod import;
mod integrations;
mod map;
pub mod native;
mod obj;
pub mod trace;
pub mod typed;
pub mod val;

use std::{
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	fmt::Debug,
	path::{Path, PathBuf},
	rc::Rc,
};

pub use ctx::*;
pub use dynamic::*;
use error::{Error::*, LocError, Result, StackTraceElement};
pub use evaluate::*;
use function::{Builtin, CallLocation, TlaArg};
use gc::{GcHashMap, TraceBox};
use gcmodule::{Cc, Trace, Weak};
pub use import::*;
pub use jrsonnet_interner::IStr;
pub use jrsonnet_parser as parser;
use jrsonnet_parser::*;
pub use obj::*;
use trace::{location_to_offset, offset_to_location, CodeLocation, CompactFormat, TraceFormat};
pub use val::{LazyVal, ManifestFormat, Val};

pub trait Bindable: Trace + 'static {
	fn bind(
		&self,
		s: State,
		this: Option<ObjValue>,
		super_obj: Option<ObjValue>,
	) -> Result<LazyVal>;
}

#[derive(Clone, Trace)]
pub enum LazyBinding {
	Bindable(Cc<TraceBox<dyn Bindable>>),
	Bound(LazyVal),
}

impl Debug for LazyBinding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "LazyBinding")
	}
}
impl LazyBinding {
	pub fn evaluate(
		&self,
		s: State,
		this: Option<ObjValue>,
		super_obj: Option<ObjValue>,
	) -> Result<LazyVal> {
		match self {
			Self::Bindable(v) => v.bind(s, this, super_obj),
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
	pub ext_natives: HashMap<IStr, Cc<TraceBox<dyn Builtin>>>,
	/// TLA vars
	pub tla_vars: HashMap<IStr, TlaArg>,
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
			globals: HashMap::default(),
			ext_vars: HashMap::default(),
			ext_natives: HashMap::default(),
			tla_vars: HashMap::default(),
			import_resolver: Box::new(DummyImportResolver),
			manifest_format: ManifestFormat::Json {
				padding: 4,
				#[cfg(feature = "exp-preserve-order")]
				preserve_order: false,
			},
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
	files: GcHashMap<Rc<Path>, FileData>,
	str_files: GcHashMap<Rc<Path>, IStr>,
	bin_files: GcHashMap<Rc<Path>, Rc<[u8]>>,
}

pub struct FileData {
	source_code: IStr,
	parsed: LocExpr,
	evaluated: Option<Val>,
}

#[allow(clippy::type_complexity)]
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
		for item in &self.0 {
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

/// Maintains stack trace and import resolution
#[derive(Default, Clone)]
pub struct State(Rc<EvaluationStateInternals>);

impl State {
	/// Parses and adds file as loaded
	pub fn add_file(&self, path: Rc<Path>, source_code: IStr) -> Result<LocExpr> {
		let parsed = parse(
			&source_code,
			&ParserSettings {
				file_name: path.clone(),
			},
		)
		.map_err(|error| ImportSyntaxError {
			error: Box::new(error),
			path: path.clone(),
			source_code: source_code.clone(),
		})?;
		self.add_parsed_file(path, source_code, parsed.clone())?;

		Ok(parsed)
	}

	pub fn reset_evaluation_state(&self, name: &Path) {
		self.data_mut()
			.files
			.get_mut(name)
			.expect("file not found")
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
		offset_to_location(&self.get_source(file).unwrap_or_else(|| "".into()), locs)
	}
	pub fn map_from_source_location(
		&self,
		file: &Path,
		line: usize,
		column: usize,
	) -> Option<usize> {
		location_to_offset(
			&self.get_source(file).expect("file not found"),
			line,
			column,
		)
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
		let contents = self.load_file_str(&file_path)?;
		self.add_file(file_path.clone(), contents)?;
		self.evaluate_loaded_file_raw(&file_path)
	}
	pub(crate) fn import_file_str(&self, from: &Path, path: &Path) -> Result<IStr> {
		let path = self.resolve_file(from, path)?;
		if !self.data().str_files.contains_key(&path) {
			let file_str = self.load_file_str(&path)?;
			self.data_mut().str_files.insert(path.clone(), file_str);
		}
		Ok(self.data().str_files.get(&path).cloned().unwrap())
	}
	pub(crate) fn import_file_bin(&self, from: &Path, path: &Path) -> Result<Rc<[u8]>> {
		let path = self.resolve_file(from, path)?;
		if !self.data().bin_files.contains_key(&path) {
			let file_bin = self.load_file_bin(&path)?;
			self.data_mut().bin_files.insert(path.clone(), file_bin);
		}
		Ok(self.data().bin_files.get(&path).cloned().unwrap())
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
		let value = evaluate(self.clone(), self.create_default_context(), &expr)?;
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

		self.add_parsed_file(
			std_path.clone(),
			STDLIB_STR.to_owned().into(),
			builtin::get_parsed_stdlib(),
		)
		.expect("stdlib is correct");
		let val = self
			.evaluate_loaded_file_raw(&std_path)
			.expect("stdlib is correct");
		self.settings_mut().globals.insert("std".into(), val);
		self
	}

	/// Creates context with all passed global variables
	pub fn create_default_context(&self) -> Context {
		let globals = &self.settings().globals;
		let mut new_bindings = GcHashMap::with_capacity(globals.len());
		for (name, value) in globals.iter() {
			new_bindings.insert(name.clone(), LazyVal::new_resolved(value.clone()));
		}
		Context::new().extend_bound(new_bindings)
	}

	/// Executes code creating a new stack frame
	pub fn push<T>(
		&self,
		e: CallLocation,
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
			}
			*stack_depth += 1;
		}
		let result = f();
		{
			let mut data = self.data_mut();
			data.stack_depth -= 1;
			data.stack_generation += 1;
		}
		if let Err(mut err) = result {
			err.trace_mut().0.push(StackTraceElement {
				location: e.0.cloned(),
				desc: frame_desc(),
			});
			return Err(err);
		}
		result
	}

	/// Executes code creating a new stack frame
	pub fn push_val(
		&self,
		e: &ExprLocation,
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
			}
			*stack_depth += 1;
		}
		let mut result = f();
		{
			let mut data = self.data_mut();
			data.stack_depth -= 1;
			data.stack_generation += 1;
			result = data
				.breakpoints
				.insert(data.stack_depth, data.stack_generation, e, result);
		}
		if let Err(mut err) = result {
			err.trace_mut().0.push(StackTraceElement {
				location: Some(e.clone()),
				desc: frame_desc(),
			});
			return Err(err);
		}
		result
	}
	/// Executes code creating a new stack frame
	pub fn push_description<T>(
		&self,
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
			}
			*stack_depth += 1;
		}
		let result = f();
		{
			let mut data = self.data_mut();
			data.stack_depth -= 1;
			data.stack_generation += 1;
		}
		if let Err(mut err) = result {
			err.trace_mut().0.push(StackTraceElement {
				location: None,
				desc: frame_desc(),
			});
			return Err(err);
		}
		result
	}

	/// # Panics
	/// In case of formatting failure
	pub fn stringify_err(&self, e: &LocError) -> String {
		let mut out = String::new();
		self.settings()
			.trace_format
			.write_trace(&mut out, self, e)
			.unwrap();
		out
	}

	pub fn manifest(&self, val: Val) -> Result<IStr> {
		self.push_description(
			|| "manifestification".to_string(),
			|| val.manifest(self.clone(), &self.manifest_format()),
		)
	}
	pub fn manifest_multi(&self, val: Val) -> Result<Vec<(IStr, IStr)>> {
		val.manifest_multi(self.clone(), &self.manifest_format())
	}
	pub fn manifest_stream(&self, val: Val) -> Result<Vec<IStr>> {
		val.manifest_stream(self.clone(), &self.manifest_format())
	}

	/// If passed value is function then call with set TLA
	pub fn with_tla(&self, val: Val) -> Result<Val> {
		Ok(match val {
			Val::Func(func) => self.push_description(
				|| "during TLA call".to_owned(),
				|| {
					func.evaluate(
						self.clone(),
						self.create_default_context(),
						CallLocation::native(),
						&self.settings().tla_vars,
						true,
					)
				},
			)?,
			v => v,
		})
	}
}

/// Internals
impl State {
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
impl State {
	pub fn evaluate_file_raw(&self, name: &Path) -> Result<Val> {
		self.import_file(&std::env::current_dir().expect("cwd"), name)
	}
	pub fn evaluate_file_raw_nocwd(&self, name: &Path) -> Result<Val> {
		self.import_file(&PathBuf::from("."), name)
	}
	/// Parses and evaluates the given snippet
	pub fn evaluate_snippet_raw(&self, source: Rc<Path>, code: IStr) -> Result<Val> {
		let parsed = parse(
			&code,
			&ParserSettings {
				file_name: source.clone(),
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
		evaluate(self.clone(), self.create_default_context(), &code)
	}
}

/// Settings utilities
impl State {
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
		self.settings_mut()
			.tla_vars
			.insert(name, TlaArg::Val(value));
	}
	pub fn add_tla_str(&self, name: IStr, value: IStr) {
		self.settings_mut()
			.tla_vars
			.insert(name, TlaArg::String(value));
	}
	pub fn add_tla_code(&self, name: IStr, code: IStr) -> Result<()> {
		let parsed = self.add_file(PathBuf::from(format!("tla_code {}", name)).into(), code)?;
		self.settings_mut()
			.tla_vars
			.insert(name, TlaArg::Code(parsed));
		Ok(())
	}

	pub fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>> {
		self.settings().import_resolver.resolve_file(from, path)
	}
	pub fn load_file_str(&self, path: &Path) -> Result<IStr> {
		self.settings().import_resolver.load_file_str(path)
	}
	pub fn load_file_bin(&self, path: &Path) -> Result<Rc<[u8]>> {
		self.settings().import_resolver.load_file_bin(path)
	}

	pub fn import_resolver(&self) -> Ref<dyn ImportResolver> {
		Ref::map(self.settings(), |s| &*s.import_resolver)
	}
	pub fn set_import_resolver(&self, resolver: Box<dyn ImportResolver>) {
		self.settings_mut().import_resolver = resolver;
	}

	pub fn add_native(&self, name: IStr, cb: Cc<TraceBox<dyn Builtin>>) {
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

pub fn cc_ptr_eq<T>(a: &Cc<T>, b: &Cc<T>) -> bool {
	let a = a as &T;
	let b = b as &T;
	std::ptr::eq(a, b)
}

fn weak_raw<T>(a: Weak<T>) -> *const () {
	unsafe { std::mem::transmute(a) }
}
fn weak_ptr_eq<T>(a: Weak<T>, b: Weak<T>) -> bool {
	std::ptr::eq(weak_raw(a), weak_raw(b))
}

#[test]
fn weak_unsafe() {
	let a = Cc::new(1);
	let b = Cc::new(2);

	let aw1 = a.clone().downgrade();
	let aw2 = a.clone().downgrade();
	let aw3 = a.clone().downgrade();

	let bw = b.clone().downgrade();

	assert!(weak_ptr_eq(aw1, aw2));
	assert!(!weak_ptr_eq(aw3, bw));
}
