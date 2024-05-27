//! jsonnet interpreter implementation
#![cfg_attr(feature = "nightly", feature(thread_local, type_alias_impl_trait))]

// For jrsonnet-macros
extern crate self as jrsonnet_evaluator;

mod arr;
#[cfg(feature = "async-import")]
pub mod async_import;
mod ctx;
mod dynamic;
pub mod error;
mod evaluate;
pub mod function;
pub mod gc;
mod import;
mod integrations;
pub mod manifest;
mod map;
mod obj;
pub mod stack;
pub mod stdlib;
mod tla;
pub mod trace;
pub mod typed;
pub mod val;

use std::{
	any::Any,
	cell::{RefCell, RefMut},
	fmt::{self, Debug},
	path::Path,
};

pub use ctx::*;
pub use dynamic::*;
pub use error::{Error, ErrorKind::*, Result, ResultExt};
pub use evaluate::*;
use function::CallLocation;
use gc::{GcHashMap, TraceBox};
use hashbrown::hash_map::RawEntryMut;
pub use import::*;
use jrsonnet_gcmodule::{Cc, Trace};
pub use jrsonnet_interner::{IBytes, IStr};
#[doc(hidden)]
pub use jrsonnet_macros;
pub use jrsonnet_parser as parser;
use jrsonnet_parser::{LocExpr, ParserSettings, Source, SourcePath, Span};
pub use obj::*;
use stack::check_depth;
pub use tla::apply_tla;
pub use val::{Thunk, Val};

/// Thunk without bound `super`/`this`
/// object inheritance may be overriden multiple times, and will be fixed only on field read
pub trait Unbound: Trace {
	/// Type of value after object context is bound
	type Bound;
	/// Create value bound to specified object context
	fn bind(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Self::Bound>;
}

/// Object fields may, or may not depend on `this`/`super`, this enum allows cheaper reuse of object-independent fields for native code
/// Standard jsonnet fields are always unbound
#[derive(Clone, Trace)]
pub enum MaybeUnbound {
	/// Value needs to be bound to `this`/`super`
	Unbound(Cc<TraceBox<dyn Unbound<Bound = Val>>>),
	/// Value is object-independent
	Bound(Thunk<Val>),
}

impl Debug for MaybeUnbound {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "MaybeUnbound")
	}
}
impl MaybeUnbound {
	/// Attach object context to value, if required
	pub fn evaluate(&self, sup: Option<ObjValue>, this: Option<ObjValue>) -> Result<Val> {
		match self {
			Self::Unbound(v) => v.bind(sup, this),
			Self::Bound(v) => Ok(v.evaluate()?),
		}
	}
}

/// During import, this trait will be called to create initial context for file.
/// It may initialize global variables, stdlib for example.
pub trait ContextInitializer: Trace {
	/// For which size the builder should be preallocated
	fn reserve_vars(&self) -> usize {
		0
	}
	/// Initialize default file context.
	/// Has default implementation, which calls `populate`.
	/// Prefer to always implement `populate` instead.
	fn initialize(&self, state: State, for_file: Source) -> Context {
		let mut builder = ContextBuilder::with_capacity(state, self.reserve_vars());
		self.populate(for_file, &mut builder);
		builder.build()
	}
	/// For composability: extend builder. May panic if this initialization is not supported,
	/// and the context may only be created via `initialize`.
	fn populate(&self, for_file: Source, builder: &mut ContextBuilder);
	/// Allows upcasting from abstract to concrete context initializer.
	/// jrsonnet by itself doesn't use this method, it is allowed for it to panic.
	fn as_any(&self) -> &dyn Any;
}

/// Context initializer which adds nothing.
impl ContextInitializer for () {
	fn populate(&self, _for_file: Source, _builder: &mut ContextBuilder) {}
	fn as_any(&self) -> &dyn Any {
		self
	}
}

impl<T> ContextInitializer for Option<T>
where
	T: ContextInitializer,
{
	fn initialize(&self, state: State, for_file: Source) -> Context {
		if let Some(ctx) = self {
			ctx.initialize(state, for_file)
		} else {
			().initialize(state, for_file)
		}
	}

	fn populate(&self, for_file: Source, builder: &mut ContextBuilder) {
		if let Some(ctx) = self {
			ctx.populate(for_file, builder);
		}
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

macro_rules! impl_context_initializer {
	($($gen:ident)*) => {
		#[allow(non_snake_case)]
		impl<$($gen: ContextInitializer + Trace,)*> ContextInitializer for ($($gen,)*) {
			fn reserve_vars(&self) -> usize {
				let mut out = 0;
				let ($($gen,)*) = self;
				$(out += $gen.reserve_vars();)*
				out
			}
			fn populate(&self, for_file: Source, builder: &mut ContextBuilder) {
				let ($($gen,)*) = self;
				$($gen.populate(for_file.clone(), builder);)*
			}
			fn as_any(&self) -> &dyn Any {
				self
			}
		}
	};
	($($cur:ident)* @ $c:ident $($rest:ident)*) => {
		impl_context_initializer!($($cur)*);
		impl_context_initializer!($($cur)* $c @ $($rest)*);
	};
	($($cur:ident)* @) => {
		impl_context_initializer!($($cur)*);
	}
}
impl_context_initializer! {
	A @ B C D E F G
}

#[derive(Trace)]
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
	pub(crate) fn get_string(&mut self) -> Option<IStr> {
		if self.string.is_none() {
			self.string = Some(
				self.bytes
					.as_ref()
					.expect("either string or bytes should be set")
					.clone()
					.cast_str()?,
			);
		}
		Some(self.string.clone().expect("just set"))
	}
}

#[derive(Trace)]
pub struct EvaluationStateInternals {
	/// Internal state
	file_cache: RefCell<GcHashMap<SourcePath, FileData>>,
	/// Context initializer, which will be used for imports and everything
	/// [`NoopContextInitializer`] is used by default, most likely you want to have `jrsonnet-stdlib`
	context_initializer: TraceBox<dyn ContextInitializer>,
	/// Used to resolve file locations/contents
	import_resolver: TraceBox<dyn ImportResolver>,
}

/// Maintains stack trace and import resolution
#[derive(Clone, Trace)]
pub struct State(Cc<EvaluationStateInternals>);

impl State {
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved_str(&self, path: SourcePath) -> Result<IStr> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(v) => {
				let data = self.import_resolver().load_file_contents(&path)?;
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
		Ok(file
			.get_string()
			.ok_or_else(|| ImportBadFileUtf8(path.clone()))?)
	}
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved_bin(&self, path: SourcePath) -> Result<IBytes> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(v) => {
				let data = self.import_resolver().load_file_contents(&path)?;
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
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved(&self, path: SourcePath) -> Result<Val> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.raw_entry_mut().from_key(&path);

		let file = match file {
			RawEntryMut::Occupied(ref mut d) => d.get_mut(),
			RawEntryMut::Vacant(v) => {
				let data = self.import_resolver().load_file_contents(&path)?;
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
		let code = file
			.get_string()
			.ok_or_else(|| ImportBadFileUtf8(path.clone()))?;
		let file_name = Source::new(path.clone(), code.clone());
		if file.parsed.is_none() {
			file.parsed = Some(
				jrsonnet_parser::parse(
					&code,
					&ParserSettings {
						source: file_name.clone(),
					},
				)
				.map_err(|e| ImportSyntaxError {
					path: file_name.clone(),
					error: Box::new(e),
				})?,
			);
		}
		let parsed = file.parsed.as_ref().expect("just set").clone();
		if file.evaluating {
			bail!(InfiniteRecursionDetected)
		}
		file.evaluating = true;
		// Dropping file cache guard here, as evaluation may use this map too
		drop(file_cache);
		let res = evaluate(self.create_default_context(file_name), &parsed);

		let mut file_cache = self.file_cache();
		let mut file = file_cache.raw_entry_mut().from_key(&path);

		let RawEntryMut::Occupied(file) = &mut file else {
			unreachable!("this file was just here!")
		};
		let file = file.get_mut();
		file.evaluating = false;
		match res {
			Ok(v) => {
				file.evaluated = Some(v.clone());
				Ok(v)
			}
			Err(e) => Err(e),
		}
	}

	/// Has same semantics as `import 'path'` called from `from` file
	pub fn import_from(&self, from: &SourcePath, path: &str) -> Result<Val> {
		let resolved = self.resolve_from(from, path)?;
		self.import_resolved(resolved)
	}
	pub fn import(&self, path: impl AsRef<Path>) -> Result<Val> {
		let resolved = self.resolve(path)?;
		self.import_resolved(resolved)
	}

	/// Creates context with all passed global variables
	pub fn create_default_context(&self, source: Source) -> Context {
		self.context_initializer().initialize(self.clone(), source)
	}

	/// Creates context with all passed global variables, calling custom modifier
	pub fn create_default_context_with(
		&self,
		source: Source,
		context_initializer: impl ContextInitializer,
	) -> Context {
		let default_initializer = self.context_initializer();
		let mut builder = ContextBuilder::with_capacity(
			self.clone(),
			default_initializer.reserve_vars() + context_initializer.reserve_vars(),
		);
		default_initializer.populate(source.clone(), &mut builder);
		context_initializer.populate(source, &mut builder);

		builder.build()
	}

	/// Executes code creating a new stack frame
	pub fn push<T>(
		e: CallLocation<'_>,
		frame_desc: impl FnOnce() -> String,
		f: impl FnOnce() -> Result<T>,
	) -> Result<T> {
		let _guard = check_depth()?;

		f().with_description_src(e, frame_desc)
	}

	/// Executes code creating a new stack frame
	pub fn push_val(
		&self,
		e: &Span,
		frame_desc: impl FnOnce() -> String,
		f: impl FnOnce() -> Result<Val>,
	) -> Result<Val> {
		let _guard = check_depth()?;

		f().with_description_src(e, frame_desc)
	}
	/// Executes code creating a new stack frame
	pub fn push_description<T>(
		frame_desc: impl FnOnce() -> String,
		f: impl FnOnce() -> Result<T>,
	) -> Result<T> {
		let _guard = check_depth()?;

		f().with_description(frame_desc)
	}
}

/// Internals
impl State {
	fn file_cache(&self) -> RefMut<'_, GcHashMap<SourcePath, FileData>> {
		self.0.file_cache.borrow_mut()
	}
}

#[derive(Trace)]
pub struct InitialUnderscore(pub Thunk<Val>);
impl ContextInitializer for InitialUnderscore {
	fn populate(&self, _for_file: Source, builder: &mut ContextBuilder) {
		builder.bind("_", self.0.clone());
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

/// Raw methods evaluate passed values but don't perform TLA execution
impl State {
	/// Parses and evaluates the given snippet
	pub fn evaluate_snippet(&self, name: impl Into<IStr>, code: impl Into<IStr>) -> Result<Val> {
		let code = code.into();
		let source = Source::new_virtual(name.into(), code.clone());
		let parsed = jrsonnet_parser::parse(
			&code,
			&ParserSettings {
				source: source.clone(),
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source.clone(),
			error: Box::new(e),
		})?;
		evaluate(self.create_default_context(source), &parsed)
	}
	/// Parses and evaluates the given snippet with custom context modifier
	pub fn evaluate_snippet_with(
		&self,
		name: impl Into<IStr>,
		code: impl Into<IStr>,
		context_initializer: impl ContextInitializer,
	) -> Result<Val> {
		let code = code.into();
		let source = Source::new_virtual(name.into(), code.clone());
		let parsed = jrsonnet_parser::parse(
			&code,
			&ParserSettings {
				source: source.clone(),
			},
		)
		.map_err(|e| ImportSyntaxError {
			path: source.clone(),
			error: Box::new(e),
		})?;
		evaluate(
			self.create_default_context_with(source, context_initializer),
			&parsed,
		)
	}
}

/// Settings utilities
impl State {
	// Only panics in case of [`ImportResolver`] contract violation
	#[allow(clippy::missing_panics_doc)]
	pub fn resolve_from(&self, from: &SourcePath, path: &str) -> Result<SourcePath> {
		self.import_resolver().resolve_from(from, path.as_ref())
	}

	// Only panics in case of [`ImportResolver`] contract violation
	#[allow(clippy::missing_panics_doc)]
	pub fn resolve(&self, path: impl AsRef<Path>) -> Result<SourcePath> {
		self.import_resolver().resolve(path.as_ref())
	}
	pub fn import_resolver(&self) -> &dyn ImportResolver {
		&*self.0.import_resolver
	}
	pub fn context_initializer(&self) -> &dyn ContextInitializer {
		&*self.0.context_initializer
	}
}

impl State {
	pub fn builder() -> StateBuilder {
		StateBuilder::default()
	}
}

impl Default for State {
	fn default() -> Self {
		Self::builder().build()
	}
}

#[derive(Default)]
pub struct StateBuilder {
	import_resolver: Option<TraceBox<dyn ImportResolver>>,
	context_initializer: Option<TraceBox<dyn ContextInitializer>>,
}
impl StateBuilder {
	pub fn import_resolver(&mut self, import_resolver: impl ImportResolver) -> &mut Self {
		let _ = self.import_resolver.insert(tb!(import_resolver));
		self
	}
	pub fn context_initializer(
		&mut self,
		context_initializer: impl ContextInitializer,
	) -> &mut Self {
		let _ = self.context_initializer.insert(tb!(context_initializer));
		self
	}
	pub fn build(mut self) -> State {
		State(Cc::new(EvaluationStateInternals {
			file_cache: RefCell::new(GcHashMap::new()),
			context_initializer: self.context_initializer.take().unwrap_or_else(|| tb!(())),
			import_resolver: self
				.import_resolver
				.take()
				.unwrap_or_else(|| tb!(DummyImportResolver)),
		}))
	}
}
