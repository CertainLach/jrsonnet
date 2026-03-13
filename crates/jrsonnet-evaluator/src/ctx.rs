use std::fmt::Debug;

use educe::Educe;
use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use rustc_hash::FxHashMap;

use crate::{
	error::ErrorKind::*, gc::WithCapacityExt as _, map::LayeredHashMap, ObjValue, Pending, Result,
	SupThis, Thunk, Val,
};
/// Context keeps information about current lexical code location
///
/// This information includes local variables, top-level object (`$`), current object (`this`), and super object (`super`)
#[derive(Debug, Trace, Clone, Educe)]
#[educe(PartialEq)]
pub struct Context(#[educe(PartialEq(method = Cc::ptr_eq))] Cc<ContextInternal>);

#[derive(Debug, Trace)]
struct ContextInternal {
	dollar: Option<ObjValue>,
	sup_this: Option<SupThis>,
	bindings: LayeredHashMap,
}
impl Context {
	pub fn new_future() -> Pending<Self> {
		Pending::new()
	}

	pub fn dollar(&self) -> Option<&ObjValue> {
		self.0.dollar.as_ref()
	}

	pub fn try_dollar(&self) -> Result<ObjValue> {
		self.0
			.dollar
			.clone()
			.ok_or_else(|| CantUseSelfSupOutsideOfObject.into())
	}

	pub fn this(&self) -> Option<&ObjValue> {
		self.0.sup_this.as_ref().map(SupThis::this)
	}

	pub fn try_this(&self) -> Result<ObjValue> {
		self.0
			.sup_this
			.as_ref()
			.ok_or_else(|| CantUseSelfSupOutsideOfObject.into())
			.map(SupThis::this)
			.cloned()
	}

	pub fn sup_this(&self) -> Option<&SupThis> {
		self.0.sup_this.as_ref()
	}

	pub fn try_sup_this(&self) -> Result<SupThis> {
		self.0
			.sup_this
			.clone()
			.ok_or_else(|| CantUseSelfSupOutsideOfObject.into())
	}

	pub fn binding(&self, name: IStr) -> Result<Thunk<Val>> {
		use std::cmp::Ordering;

		use crate::bail;

		if let Some(val) = self.0.bindings.get(&name).cloned() {
			return Ok(val);
		}

		let mut heap = Vec::new();
		self.0.bindings.clone().iter_keys(|k| {
			let conf = strsim::jaro_winkler(&k as &str, &name as &str);
			if conf < 0.8 {
				return;
			}
			heap.push((conf, k));
		});
		heap.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

		bail!(VariableIsNotDefined(
			name,
			heap.into_iter().map(|(_, k)| k).collect()
		))
	}
	pub fn contains_binding(&self, name: IStr) -> bool {
		self.0.bindings.contains_key(&name)
	}
	#[must_use]
	pub fn into_future(self, ctx: Pending<Self>) -> Self {
		{
			ctx.clone().fill(self);
		}
		ctx.unwrap()
	}

	#[must_use]
	pub fn with_var(self, name: impl Into<IStr>, value: Val) -> Self {
		let mut new_bindings = FxHashMap::with_capacity(1);
		new_bindings.insert(name.into(), Thunk::evaluated(value));
		self.extend_bindings(new_bindings)
	}

	#[must_use]
	pub fn extend_bindings_sup_this(
		self,
		new_bindings: FxHashMap<IStr, Thunk<Val>>,
		sup_this: SupThis,
	) -> Self {
		let ctx = &self;
		let dollar = ctx
			.0
			.dollar
			.clone()
			.or_else(|| Some(sup_this.this().clone()));
		let bindings = if new_bindings.is_empty() {
			ctx.0.bindings.clone()
		} else {
			ctx.0.bindings.clone().extend(new_bindings)
		};
		Self(Cc::new(ContextInternal {
			dollar,
			sup_this: Some(sup_this),
			bindings,
		}))
	}
	#[must_use]
	pub fn extend_bindings(self, new_bindings: FxHashMap<IStr, Thunk<Val>>) -> Self {
		if new_bindings.is_empty() {
			return self;
		}
		let ctx = &self;
		let bindings = if new_bindings.is_empty() {
			ctx.0.bindings.clone()
		} else {
			ctx.0.bindings.clone().extend(new_bindings)
		};
		Self(Cc::new(ContextInternal {
			dollar: ctx.0.dollar.clone(),
			sup_this: ctx.0.sup_this.clone(),
			bindings,
		}))
	}
}

#[derive(Default)]
pub struct ContextBuilder {
	bindings: FxHashMap<IStr, Thunk<Val>>,
	extend: Option<Context>,
}

impl ContextBuilder {
	pub fn new() -> Self {
		Self::with_capacity(0)
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			bindings: FxHashMap::with_capacity(capacity),
			extend: None,
		}
	}

	pub fn extend(parent: Context) -> Self {
		Self {
			bindings: FxHashMap::new(),
			extend: Some(parent),
		}
	}

	/// # Panics
	///
	/// If `name` is already bound. Makes no sense to bind same local multiple times,
	/// unless it is separate context layers.
	pub fn bind(&mut self, name: impl Into<IStr>, value: Thunk<Val>) -> &mut Self {
		let old = self.bindings.insert(name.into(), value);
		assert!(old.is_none(), "variable bound twice in single context call");
		self
	}
	pub fn build(self) -> Context {
		if let Some(parent) = self.extend {
			parent.extend_bindings(self.bindings)
		} else {
			Context(Cc::new(ContextInternal {
				bindings: LayeredHashMap::new(self.bindings),
				dollar: None,
				sup_this: None,
			}))
		}
	}
}
