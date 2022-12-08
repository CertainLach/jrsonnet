use std::fmt::Debug;

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;

use crate::{
	error::ErrorKind::*, gc::GcHashMap, map::LayeredHashMap, ObjValue, Pending, Result, State,
	Thunk, Val,
};

#[derive(Trace)]
struct ContextInternals {
	state: Option<State>,
	dollar: Option<ObjValue>,
	sup: Option<ObjValue>,
	this: Option<ObjValue>,
	bindings: LayeredHashMap,
}
impl Debug for ContextInternals {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Context").finish()
	}
}

/// Context keeps information about current lexical code location
///
/// This information includes local variables, top-level object (`$`), current object (`this`), and super object (`super`)
#[derive(Debug, Clone, Trace)]
pub struct Context(Cc<ContextInternals>);
impl Context {
	pub fn new_future() -> Pending<Self> {
		Pending::new()
	}

	pub fn state(&self) -> &State {
		self.0
			.state
			.as_ref()
			.expect("used state from dummy context")
	}

	pub fn dollar(&self) -> Option<&ObjValue> {
		self.0.dollar.as_ref()
	}

	pub fn this(&self) -> Option<&ObjValue> {
		self.0.this.as_ref()
	}

	pub fn super_obj(&self) -> Option<&ObjValue> {
		self.0.sup.as_ref()
	}

	#[cfg(not(feature = "friendly-errors"))]
	pub fn binding(&self, name: IStr) -> Result<Thunk<Val>> {
		Ok(self
			.0
			.bindings
			.get(&name)
			.cloned()
			.ok_or(VariableIsNotDefined(name, vec![]))?)
	}

	#[cfg(feature = "friendly-errors")]
	pub fn binding(&self, name: IStr) -> Result<Thunk<Val>> {
		use std::cmp::Ordering;

		use crate::throw;

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

		throw!(VariableIsNotDefined(
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
			ctx.0.borrow_mut().replace(self);
		}
		ctx.unwrap()
	}

	#[must_use]
	pub fn with_var(self, name: IStr, value: Val) -> Self {
		let mut new_bindings = GcHashMap::with_capacity(1);
		new_bindings.insert(name, Thunk::evaluated(value));
		self.extend(new_bindings, None, None, None)
	}

	#[must_use]
	pub fn extend(
		self,
		new_bindings: GcHashMap<IStr, Thunk<Val>>,
		new_dollar: Option<ObjValue>,
		new_sup: Option<ObjValue>,
		new_this: Option<ObjValue>,
	) -> Self {
		let ctx = &self.0;
		let dollar = new_dollar.or_else(|| ctx.dollar.clone());
		let this = new_this.or_else(|| ctx.this.clone());
		let sup = new_sup.or_else(|| ctx.sup.clone());
		let bindings = if new_bindings.is_empty() {
			ctx.bindings.clone()
		} else {
			ctx.bindings.clone().extend(new_bindings)
		};
		Self(Cc::new(ContextInternals {
			state: ctx.state.clone(),
			dollar,
			sup,
			this,
			bindings,
		}))
	}
}

impl PartialEq for Context {
	fn eq(&self, other: &Self) -> bool {
		Cc::ptr_eq(&self.0, &other.0)
	}
}

pub struct ContextBuilder {
	state: Option<State>,
	bindings: GcHashMap<IStr, Thunk<Val>>,
	extend: Option<Context>,
}

impl ContextBuilder {
	/// # Panics
	/// Panics aren't directly caused by this function, but if state from resulting context is used
	pub fn dangerous_empty_state() -> Self {
		Self {
			state: None,
			bindings: GcHashMap::new(),
			extend: None,
		}
	}
	pub fn new(state: State) -> Self {
		Self::with_capacity(state, 0)
	}
	pub fn with_capacity(state: State, capacity: usize) -> Self {
		Self {
			state: Some(state),
			bindings: GcHashMap::with_capacity(capacity),
			extend: None,
		}
	}
	pub fn extend(parent: Context) -> Self {
		Self {
			state: parent.0.state.clone(),
			bindings: GcHashMap::new(),
			extend: Some(parent),
		}
	}
	/// # Panics
	/// If `name` is already bound
	pub fn bind(&mut self, name: IStr, value: Thunk<Val>) -> &mut Self {
		let old = self.bindings.insert(name, value);
		assert!(old.is_none(), "variable bound twice in single context call");
		self
	}
	pub fn build(self) -> Context {
		if let Some(parent) = self.extend {
			// TODO: replace self.extend with Result<Context, State>, and remove `state` field
			parent.extend(self.bindings, None, None, None)
		} else {
			Context(Cc::new(ContextInternals {
				state: self.state,
				bindings: LayeredHashMap::new(self.bindings),
				dollar: None,
				sup: None,
				this: None,
			}))
		}
	}
}
