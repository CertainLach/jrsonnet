use std::fmt::Debug;

use jrsonnet_gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;

use crate::{
	error::Error::*, gc::GcHashMap, map::LayeredHashMap, ObjValue, Pending, Result, Thunk, Val,
};

#[derive(Trace)]
struct ContextInternals {
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

#[derive(Debug, Clone, Trace)]
pub struct Context(Cc<ContextInternals>);
impl Context {
	pub fn new_future() -> Pending<Self> {
		Pending::new()
	}

	pub fn dollar(&self) -> &Option<ObjValue> {
		&self.0.dollar
	}

	pub fn this(&self) -> &Option<ObjValue> {
		&self.0.this
	}

	pub fn super_obj(&self) -> &Option<ObjValue> {
		&self.0.sup
	}

	pub fn new() -> Self {
		Self(Cc::new(ContextInternals {
			dollar: None,
			this: None,
			sup: None,
			bindings: LayeredHashMap::default(),
		}))
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
			dollar,
			sup,
			this,
			bindings,
		}))
	}
}

impl Default for Context {
	fn default() -> Self {
		Self::new()
	}
}

impl PartialEq for Context {
	fn eq(&self, other: &Self) -> bool {
		Cc::ptr_eq(&self.0, &other.0)
	}
}

#[derive(Default)]
pub struct ContextBuilder {
	bindings: GcHashMap<IStr, Thunk<Val>>,
}
impl ContextBuilder {
	pub fn new() -> Self {
		Self::default()
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			bindings: GcHashMap::with_capacity(capacity),
		}
	}
	pub fn bind(&mut self, name: IStr, value: Thunk<Val>) -> &mut Self {
		self.bindings.insert(name, value);
		self
	}
	pub fn build(self) -> Context {
		Context(Cc::new(ContextInternals {
			bindings: LayeredHashMap::new(self.bindings),
			dollar: None,
			sup: None,
			this: None,
		}))
	}
}
