//! jsonnet interpreter implementation
#![cfg_attr(nightly, feature(thread_local, type_alias_impl_trait))]

// For jrsonnet-macros
extern crate self as jrsonnet_evaluator;

mod arr;
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
	clone::Clone,
	collections::hash_map::Entry,
	fmt::{self, Debug},
	marker::PhantomData,
	rc::Rc,
};

pub use ctx::*;
pub use dynamic::*;
pub use error::{Error, ErrorKind::*, Result, ResultExt};
pub use evaluate::*;
use function::CallLocation;
pub use import::*;
use jrsonnet_gcmodule::{cc_dyn, Cc, Trace};
pub use jrsonnet_interner::{IBytes, IStr};
#[doc(hidden)]
pub use jrsonnet_macros;
pub use jrsonnet_parser as parser;
use jrsonnet_parser::{LocExpr, ParserSettings, Source, SourcePath};
pub use obj::*;
pub use rustc_hash;
use rustc_hash::FxHashMap;
use stack::check_depth;
pub use tla::apply_tla;
pub use val::{Thunk, Val};

use crate::gc::WithCapacityExt as _;

cc_dyn!(
	#[derive(Clone)]
	CcUnbound<V>,
	Unbound<Bound = V>
);

/// Thunk without bound `super`/`this`
/// object inheritance may be overriden multiple times, and will be fixed only on field read
pub trait Unbound: Trace {
	/// Type of value after object context is bound
	type Bound;
	/// Create value bound to specified object context
	fn bind(&self, sup_this: SupThis) -> Result<Self::Bound>;
}

/// Object fields may, or may not depend on `this`/`super`, this enum allows cheaper reuse of object-independent fields for native code
/// Standard jsonnet fields are always unbound
#[derive(Clone, Trace)]
pub enum MaybeUnbound {
	/// Value needs to be bound to `this`/`super`
	Unbound(CcUnbound<Val>),
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
	pub fn evaluate(&self, sup_this: SupThis) -> Result<Val> {
		match self {
			Self::Unbound(v) => v.0.bind(sup_this),
			Self::Bound(v) => Ok(v.evaluate()?),
		}
	}
}

cc_dyn!(CcContextInitializer, ContextInitializer);

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
	fn initialize(&self, for_file: Source) -> Context {
		let mut builder = ContextBuilder::with_capacity(self.reserve_vars());
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
	fn initialize(&self, for_file: Source) -> Context {
		if let Some(ctx) = self {
			ctx.initialize(for_file)
		} else {
			().initialize(for_file)
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
	file_cache: RefCell<FxHashMap<SourcePath, FileData>>,
	/// Context initializer, which will be used for imports and everything
	/// [`NoopContextInitializer`] is used by default, most likely you want to have `jrsonnet-stdlib`
	context_initializer: CcContextInitializer,
	/// Used to resolve file locations/contents
	import_resolver: Rc<dyn ImportResolver>,
}

/// Maintains stack trace and import resolution
#[derive(Clone, Trace)]
pub struct State(Cc<EvaluationStateInternals>);

thread_local! {
	pub static DEFAULT_STATE: State = State::builder().build();
	pub static STATE: RefCell<Option<State>> = const {RefCell::new(None)};
}
pub struct StateEnterGuard(PhantomData<()>);
impl Drop for StateEnterGuard {
	fn drop(&mut self) {
		STATE.with_borrow_mut(|v| *v = None);
	}
}

pub fn with_state<V>(v: impl FnOnce(State) -> V) -> V {
	if let Some(state) = STATE.with_borrow(Clone::clone) {
		v(state)
	} else {
		let s = DEFAULT_STATE.with(Clone::clone);
		v(s)
	}
}

impl State {
	pub fn enter(&self) -> StateEnterGuard {
		self.try_enter().expect("entered state already exists")
	}
	pub fn try_enter(&self) -> Option<StateEnterGuard> {
		STATE.with_borrow_mut(|v| {
			if v.is_none() {
				*v = Some(self.clone());
				Some(StateEnterGuard(PhantomData))
			} else {
				None
			}
		})
	}
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved_str(&self, path: SourcePath) -> Result<IStr> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.entry(path.clone());

		let file = match file {
			Entry::Occupied(ref mut d) => d.get_mut(),
			Entry::Vacant(v) => {
				let data = self.import_resolver().load_file_contents(&path)?;
				v.insert(FileData::new_string(
					std::str::from_utf8(&data)
						.map_err(|_| ImportBadFileUtf8(path.clone()))?
						.into(),
				))
			}
		};
		Ok(file
			.get_string()
			.ok_or_else(|| ImportBadFileUtf8(path.clone()))?)
	}
	/// Should only be called with path retrieved from [`resolve_path`], may panic otherwise
	pub fn import_resolved_bin(&self, path: SourcePath) -> Result<IBytes> {
		let mut file_cache = self.file_cache();
		let mut file = file_cache.entry(path.clone());

		let file = match file {
			Entry::Occupied(ref mut d) => d.get_mut(),
			Entry::Vacant(v) => {
				let data = self.import_resolver().load_file_contents(&path)?;
				v.insert(FileData::new_bytes(data.as_slice().into()))
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
		let mut file = file_cache.entry(path.clone());

		let file = match file {
			Entry::Occupied(ref mut d) => d.get_mut(),
			Entry::Vacant(v) => {
				let data = self.import_resolver().load_file_contents(&path)?;
				v.insert(FileData::new_string(
					std::str::from_utf8(&data)
						.map_err(|_| ImportBadFileUtf8(path.clone()))?
						.into(),
				))
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
		let mut file = file_cache.entry(path.clone());

		let Entry::Occupied(file) = &mut file else {
			unreachable!("this file was just here")
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
	pub fn import_from(&self, from: &SourcePath, path: impl AsPathLike) -> Result<Val> {
		let resolved = self.resolve_from(from, &path)?;
		self.import_resolved(resolved)
	}
	pub fn import(&self, path: impl AsPathLike) -> Result<Val> {
		let resolved = self.resolve_from_default(&path)?;
		self.import_resolved(resolved)
	}

	/// Creates context with all passed global variables
	pub fn create_default_context(&self, source: Source) -> Context {
		self.context_initializer().initialize(source)
	}

	/// Creates context with all passed global variables, calling custom modifier
	pub fn create_default_context_with(
		&self,
		source: Source,
		context_initializer: impl ContextInitializer,
	) -> Context {
		let default_initializer = self.context_initializer();
		let mut builder = ContextBuilder::with_capacity(
			default_initializer.reserve_vars() + context_initializer.reserve_vars(),
		);
		default_initializer.populate(source.clone(), &mut builder);
		context_initializer.populate(source, &mut builder);

		builder.build()
	}
}

/// Internals
impl State {
	fn file_cache(&self) -> RefMut<'_, FxHashMap<SourcePath, FileData>> {
		self.0.file_cache.borrow_mut()
	}
}
/// Executes code creating a new stack frame, to be replaced with try{}
pub fn in_frame<T>(
	e: CallLocation<'_>,
	frame_desc: impl FnOnce() -> String,
	f: impl FnOnce() -> Result<T>,
) -> Result<T> {
	let _guard = check_depth()?;

	f().with_description_src(e, frame_desc)
}

/// Executes code creating a new stack frame, to be replaced with try{}
pub fn in_description_frame<T>(
	frame_desc: impl FnOnce() -> String,
	f: impl FnOnce() -> Result<T>,
) -> Result<T> {
	let _guard = check_depth()?;

	f().with_description(frame_desc)
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
	pub fn resolve_from(&self, from: &SourcePath, path: &dyn AsPathLike) -> Result<SourcePath> {
		self.import_resolver().resolve_from(from, path)
	}
	#[allow(clippy::missing_panics_doc)]
	pub fn resolve_from_default(&self, path: &dyn AsPathLike) -> Result<SourcePath> {
		self.import_resolver().resolve_from_default(path)
	}
	pub fn import_resolver(&self) -> &dyn ImportResolver {
		&*self.0.import_resolver
	}
	pub fn context_initializer(&self) -> &dyn ContextInitializer {
		&*self.0.context_initializer.0
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
	import_resolver: Option<Rc<dyn ImportResolver>>,
	context_initializer: Option<CcContextInitializer>,
}
impl StateBuilder {
	pub fn import_resolver(&mut self, import_resolver: impl ImportResolver) -> &mut Self {
		let _ = self.import_resolver.insert(Rc::new(import_resolver));
		self
	}
	pub fn context_initializer(
		&mut self,
		context_initializer: impl ContextInitializer,
	) -> &mut Self {
		let _ = self
			.context_initializer
			.insert(CcContextInitializer::new(context_initializer));
		self
	}
	pub fn build(mut self) -> State {
		State(Cc::new(EvaluationStateInternals {
			file_cache: RefCell::new(FxHashMap::new()),
			context_initializer: self
				.context_initializer
				.take()
				.unwrap_or_else(|| CcContextInitializer::new(())),
			import_resolver: self
				.import_resolver
				.take()
				.unwrap_or_else(|| Rc::new(DummyImportResolver)),
		}))
	}
}
