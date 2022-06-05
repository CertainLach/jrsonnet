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
	// False positives
	// https://github.com/rust-lang/rust-clippy/issues/6902
	clippy::use_self,
	// https://github.com/rust-lang/rust-clippy/issues/8539
	clippy::iter_with_drain,
)]

// For jrsonnet-macros
extern crate self as jrsonnet_evaluator;

mod ctx;
mod dynamic;
pub mod error;
mod evaluate;
pub mod function;
pub mod gc;
mod import;
mod integrations;
mod map;
mod obj;
mod stdlib;
pub mod trace;
pub mod typed;
pub mod val;

use std::{
	borrow::Cow,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	fmt::{self, Debug},
	path::{Path, PathBuf},
	rc::Rc,
};

pub use ctx::*;
pub use dynamic::*;
use error::{Error::*, LocError, Result, StackTraceElement};
pub use evaluate::*;
use function::{builtin::Builtin, CallLocation, TlaArg};
use gc::{GcHashMap, TraceBox};
use hashbrown::hash_map::RawEntryMut;
pub use import::*;
use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IBytes;
pub use jrsonnet_interner::IStr;
pub use jrsonnet_parser as parser;
use jrsonnet_parser::*;
pub use obj::*;
use trace::{location_to_offset, offset_to_location, CodeLocation, CompactFormat, TraceFormat};
pub use val::{ManifestFormat, Thunk, Val};

pub trait Unbound: Trace {
	type Bound;
	fn bind(&self, s: State, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Self::Bound>;
}

#[derive(Clone, Trace)]
pub enum LazyBinding {
	Bindable(Cc<TraceBox<dyn Unbound<Bound = Thunk<Val>>>>),
	Bound(Thunk<Val>),
}

impl Debug for LazyBinding {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "LazyBinding")
	}
}
impl LazyBinding {
	pub fn evaluate(
		&self,
		s: State,
		sup: Option<ObjValue>,
		this: Option<ObjValue>,
	) -> Result<Thunk<Val>> {
		match self {
			Self::Bindable(v) => v.bind(s, sup, this),
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
	pub ext_vars: HashMap<IStr, TlaArg>,
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
	files: GcHashMap<PathBuf, FileData>,
	/// Contains tla arguments and others, which aren't needed to be obtained by name
	volatile_files: GcHashMap<String, String>,
}
struct FileData {
	string: Option<IStr>,
	bytes: Option<IBytes>,
	parsed: Option<LocExpr>,
	evaluated: Option<Val>,

	evaluating: bool,
}
impl FileData {
	fn new_string(data: IStr) -> Self {
		Self {
			string: Some(data),
			bytes: None,
			parsed: None,
			evaluated: None,
			evaluating: false,
		}
	}
	fn new_bytes(data: IBytes) -> Self {
		Self {
			string: None,
			bytes: Some(data),
			parsed: None,
			evaluated: None,
			evaluating: false,
		}
	}
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
	pub fn import_str(&self, path: PathBuf) -> Result<IStr> {
		let mut data = self.data_mut();
		let mut file = data.files.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(v) => {
				let data = self.settings().import_resolver.load_file_contents(&path)?;
				v.insert(
					path.clone(),
					FileData::new_string(
						std::str::from_utf8(&data)
							.map_err(|_| ImportBadFileUtf8(path.clone()))?
							.into(),
					),
				)
				.1
			}
		};
		if let Some(str) = &file.string {
			return Ok(str.clone());
		}
		if file.string.is_none() {
			file.string = Some(
				file.bytes
					.as_ref()
					.expect("either string or bytes should be set")
					.clone()
					.cast_str()
					.ok_or_else(|| ImportBadFileUtf8(path.clone()))?,
			);
		}
		Ok(file.string.as_ref().expect("just set").clone())
	}
	pub fn import_bin(&self, path: PathBuf) -> Result<IBytes> {
		let mut data = self.data_mut();
		let mut file = data.files.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(v) => {
				let data = self.settings().import_resolver.load_file_contents(&path)?;
				v.insert(path.clone(), FileData::new_bytes(data.as_slice().into()))
					.1
			}
		};
		if let Some(str) = &file.bytes {
			return Ok(str.clone());
		}
		if file.bytes.is_none() {
			file.bytes = Some(
				file.string
					.as_ref()
					.expect("either string or bytes should be set")
					.clone()
					.cast_bytes(),
			);
		}
		Ok(file.bytes.as_ref().expect("just set").clone())
	}
	pub fn import(&self, path: PathBuf) -> Result<Val> {
		let mut data = self.data_mut();
		let mut file = data.files.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(v) => {
				let data = self.settings().import_resolver.load_file_contents(&path)?;
				v.insert(
					path.clone(),
					FileData::new_string(
						std::str::from_utf8(&data)
							.map_err(|_| ImportBadFileUtf8(path.clone()))?
							.into(),
					),
				)
				.1
			}
		};
		if let Some(val) = &file.evaluated {
			return Ok(val.clone());
		}
		if file.string.is_none() {
			file.string = Some(
				std::str::from_utf8(
					file.bytes
						.as_ref()
						.expect("either string or bytes should be set"),
				)
				.map_err(|_| ImportBadFileUtf8(path.clone()))?
				.into(),
			);
		}
		let code = file.string.as_ref().expect("just set");
		let file_name = Source::new(path.clone()).expect("resolver should return correct name");
		if file.parsed.is_none() {
			file.parsed = Some(
				jrsonnet_parser::parse(
					code,
					&ParserSettings {
						file_name: file_name.clone(),
					},
				)
				.map_err(|e| ImportSyntaxError {
					path: file_name,
					source_code: code.clone(),
					error: Box::new(e),
				})?,
			);
		}
		let parsed = file.parsed.as_ref().expect("just set").clone();
		if file.evaluating {
			throw!(InfiniteRecursionDetected)
		}
		file.evaluating = true;
		// Dropping file here, as it borrows data, which may be used in evaluation
		drop(data);
		let res = evaluate(self.clone(), self.create_default_context(), &parsed);

		let mut data = self.data_mut();
		let mut file = data.files.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(_) => unreachable!("this file was just here!"),
		};
		file.evaluating = false;
		match res {
			Ok(v) => {
				file.evaluated = Some(v.clone());
				Ok(v)
			}
			Err(e) => Err(e),
		}
	}

	pub fn get_source(&self, name: Source) -> Option<String> {
		let data = self.data();
		match name.repr() {
			Ok(real) => data
				.files
				.get(real)
				.and_then(|f| f.string.as_ref())
				.map(ToString::to_string),
			Err(e) => data.volatile_files.get(e).map(ToOwned::to_owned),
		}
	}
	pub fn map_source_locations(&self, file: Source, locs: &[u32]) -> Vec<CodeLocation> {
		offset_to_location(&self.get_source(file).unwrap_or_else(|| "".into()), locs)
	}
	pub fn map_from_source_location(
		&self,
		file: Source,
		line: usize,
		column: usize,
	) -> Option<usize> {
		location_to_offset(
			&self.get_source(file).expect("file not found"),
			line,
			column,
		)
	}
	/// Adds standard library global variable (std) to this evaluator
	pub fn with_stdlib(&self) -> &Self {
		let val = evaluate(
			self.clone(),
			self.create_default_context(),
			&stdlib::get_parsed_stdlib(),
		)
		.expect("std should not fail");
		self.settings_mut().globals.insert("std".into(), val);
		self
	}

	/// Creates context with all passed global variables
	pub fn create_default_context(&self) -> Context {
		let globals = &self.settings().globals;
		let mut new_bindings = GcHashMap::with_capacity(globals.len());
		for (name, value) in globals.iter() {
			new_bindings.insert(name.clone(), Thunk::evaluated(value.clone()));
		}
		Context::new().extend(new_bindings, None, None, None)
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
	/// Parses and evaluates the given snippet
	pub fn evaluate_snippet(&self, name: String, code: String) -> Result<Val> {
		let source = Source::new_virtual(Cow::Owned(name.clone()));
		let parsed = jrsonnet_parser::parse(
			&code,
			&ParserSettings {
				file_name: source.clone(),
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source,
			source_code: code.clone().into(),
			error: Box::new(e),
		})?;
		self.data_mut().volatile_files.insert(name, code);
		evaluate(self.clone(), self.create_default_context(), &parsed)
	}
}

/// Settings utilities
impl State {
	pub fn add_ext_var(&self, name: IStr, value: Val) {
		self.settings_mut()
			.ext_vars
			.insert(name, TlaArg::Val(value));
	}
	pub fn add_ext_str(&self, name: IStr, value: IStr) {
		self.settings_mut()
			.ext_vars
			.insert(name, TlaArg::String(value));
	}
	pub fn add_ext_code(&self, name: &str, code: String) -> Result<()> {
		let source_name = format!("<extvar:{}>", name);
		let source = Source::new_virtual(Cow::Owned(source_name.clone()));
		let parsed = jrsonnet_parser::parse(
			&code,
			&ParserSettings {
				file_name: source.clone(),
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source,
			source_code: code.clone().into(),
			error: Box::new(e),
		})?;
		self.data_mut().volatile_files.insert(source_name, code);
		self.settings_mut()
			.ext_vars
			.insert(name.into(), TlaArg::Code(parsed));
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
	pub fn add_tla_code(&self, name: IStr, code: &str) -> Result<()> {
		let source_name = format!("<top-level-arg:{}>", name);
		let source = Source::new_virtual(Cow::Owned(source_name.clone()));
		let parsed = jrsonnet_parser::parse(
			code,
			&ParserSettings {
				file_name: source.clone(),
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source,
			source_code: code.into(),
			error: Box::new(e),
		})?;
		self.data_mut()
			.volatile_files
			.insert(source_name, code.to_owned());
		self.settings_mut()
			.tla_vars
			.insert(name, TlaArg::Code(parsed));
		Ok(())
	}

	pub fn resolve_file(&self, from: &Path, path: &str) -> Result<PathBuf> {
		self.settings()
			.import_resolver
			.resolve_file(from, path.as_ref())
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
