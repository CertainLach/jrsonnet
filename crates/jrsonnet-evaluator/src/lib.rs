//! jsonnet interpreter implementation
#![cfg_attr(feature = "nightly", feature(thread_local, type_alias_impl_trait))]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(
	clippy::all,
	clippy::nursery,
	clippy::pedantic,
	// missing_docs,
	elided_lifetimes_in_paths,
	explicit_outlives_requirements,
	noop_method_call,
	single_use_lifetimes,
	variant_size_differences,
	rustdoc::all
)]
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
	clippy::type_repetition_in_bounds,
	// ci is being run with nightly, but library should work on stable
	clippy::missing_const_for_fn,
	// too many false-positives with .expect() calls
	clippy::missing_panics_doc,
)]

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
	cell::{Ref, RefCell, RefMut},
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
pub use jrsonnet_parser as parser;
use jrsonnet_parser::*;
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

/// Dynamically reconfigurable evaluation settings
#[derive(Trace)]
pub struct EvaluationSettings {
	/// Context initializer, which will be used for imports and everything
	/// [`NoopContextInitializer`] is used by default, most likely you want to have `jrsonnet-stdlib`
	pub context_initializer: TraceBox<dyn ContextInitializer>,
	/// Used to resolve file locations/contents
	pub import_resolver: TraceBox<dyn ImportResolver>,
}
impl Default for EvaluationSettings {
	fn default() -> Self {
		Self {
			context_initializer: tb!(()),
			import_resolver: tb!(DummyImportResolver),
		}
	}
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

#[derive(Default, Trace)]
pub struct EvaluationStateInternals {
	/// Internal state
	file_cache: RefCell<GcHashMap<SourcePath, FileData>>,
	/// Settings, safe to change at runtime
	settings: RefCell<EvaluationSettings>,
}

/// Maintains stack trace and import resolution
#[derive(Default, Clone, Trace)]
pub struct State(Cc<EvaluationStateInternals>);

impl State {
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved_str(&self, path: SourcePath) -> Result<IStr> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.raw_entry_mut().from_key(&path);

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
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved(&self, path: SourcePath) -> Result<Val> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.raw_entry_mut().from_key(&path);

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
			throw!(InfiniteRecursionDetected)
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
		let context_initializer = &self.settings().context_initializer;
		context_initializer.initialize(self.clone(), source)
	}

	/// Creates context with all passed global variables, calling custom modifier
	pub fn create_default_context_with(
		&self,
		source: Source,
		context_initializer: impl ContextInitializer,
	) -> Context {
		let default_initializer = &self.settings().context_initializer;
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
		e: &ExprLocation,
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
	pub fn settings(&self) -> Ref<'_, EvaluationSettings> {
		self.0.settings.borrow()
	}
	pub fn settings_mut(&self) -> RefMut<'_, EvaluationSettings> {
		self.0.settings.borrow_mut()
	}
	pub fn add_global(&self, name: IStr, value: Thunk<Val>) {
		#[derive(Trace)]
		struct GlobalsCtx {
			globals: RefCell<GcHashMap<IStr, Thunk<Val>>>,
			inner: TraceBox<dyn ContextInitializer>,
		}
		impl ContextInitializer for GlobalsCtx {
			fn reserve_vars(&self) -> usize {
				self.inner.reserve_vars() + self.globals.borrow().len()
			}
			fn populate(&self, for_file: Source, builder: &mut ContextBuilder) {
				self.inner.populate(for_file, builder);
				for (name, val) in self.globals.borrow().iter() {
					builder.bind(name.clone(), val.clone());
				}
			}

			fn as_any(&self) -> &dyn Any {
				self
			}
		}
		let mut settings = self.settings_mut();
		let initializer = &mut settings.context_initializer;
		if let Some(global) = initializer.as_any().downcast_ref::<GlobalsCtx>() {
			global.globals.borrow_mut().insert(name, value);
		} else {
			let inner = std::mem::replace(&mut settings.context_initializer, tb!(()));
			settings.context_initializer = tb!(GlobalsCtx {
				globals: {
					let mut out = GcHashMap::with_capacity(1);
					out.insert(name, value);
					RefCell::new(out)
				},
				inner
			});
		}
	}
}

#[derive(Trace)]
pub struct InitialUnderscore(pub Thunk<Val>);
impl ContextInitializer for InitialUnderscore {
	fn populate(&self, _for_file: Source, builder: &mut ContextBuilder) {
		builder.bind("_".into(), self.0.clone());
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
	pub fn import_resolver(&self) -> Ref<'_, dyn ImportResolver> {
		Ref::map(self.settings(), |s| &*s.import_resolver)
	}
	pub fn set_import_resolver(&self, resolver: impl ImportResolver) {
		self.settings_mut().import_resolver = tb!(resolver);
	}
	pub fn context_initializer(&self) -> Ref<'_, dyn ContextInitializer> {
		Ref::map(self.settings(), |s| &*s.context_initializer)
	}
	pub fn set_context_initializer(&self, initializer: impl ContextInitializer) {
		self.settings_mut().context_initializer = tb!(initializer);
	}
}
