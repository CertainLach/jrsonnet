use std::{cmp::Ordering, fmt::Debug, iter};

use educe::Educe;
use jrsonnet_gcmodule::Trace;
use jrsonnet_interner::IStr;

use crate::{
	bail, error::ErrorKind::*, typed::IntoUntyped, ObjValue, Pending, Result, SupThis, Thunk, Val,
};

#[derive(Trace, Clone, Educe)]
#[educe(Debug(name = false))]
pub enum BindingValue {
	Thunk(Thunk<Val>),
	Value(Val),
}
impl BindingValue {
	pub fn new<T: IntoUntyped>(v: T) -> Self {
		if T::provides_lazy() {
			Self::Thunk(T::into_lazy_untyped(v))
		} else {
			match T::into_untyped(v) {
				Ok(v) => Self::Value(v),
				Err(e) => Self::Thunk(Thunk::errored(e)),
			}
		}
	}
	pub fn evaluate(&self) -> Result<Val> {
		match self {
			Self::Thunk(thunk) => thunk.evaluate(),
			Self::Value(val) => Ok(val.clone()),
		}
	}
	pub fn as_thunk(&self) -> Thunk<Val> {
		match self {
			Self::Thunk(thunk) => thunk.clone(),
			Self::Value(val) => Thunk::evaluated(val.clone()),
		}
	}
}
impl From<Thunk<Val>> for BindingValue {
	fn from(value: Thunk<Val>) -> Self {
		Self::Thunk(value)
	}
}
impl From<Val> for BindingValue {
	fn from(value: Val) -> Self {
		Self::Value(value)
	}
}

#[derive(Trace, Clone, Debug, Default)]
pub struct BindingsMap {
	data: Vec<(IStr, BindingValue)>,
}
#[must_use = "should restore bindings to original"]
pub struct CapturedOld {
	data: Vec<(usize, BindingValue)>,
	old_len: usize,
}
impl BindingsMap {
	pub fn new() -> Self {
		Self::default()
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			data: Vec::with_capacity(capacity),
		}
	}
	pub fn insert(&mut self, key: IStr, value: impl Into<BindingValue>) -> bool {
		let value = value.into();
		for (k, v) in &mut self.data {
			if *k == key {
				*v = value;
				return false;
			}
		}
		self.data.push((key, value));
		true
	}
	pub fn evaluate(&self, key: &IStr) -> Result<Option<Val>> {
		let Some(v) = self.data.iter().find(|(k, _)| k == key) else {
			return Ok(None);
		};
		v.1.evaluate().map(Some)
	}
	pub fn contains_key(&self, key: &IStr) -> bool {
		self.data.iter().any(|(k, _)| k == key)
	}
	pub fn extend(&mut self, v: impl IntoIterator<Item = (IStr, BindingValue)>) {
		for (key, value) in v {
			self.insert(key, value);
		}
	}
	pub fn overlay_capture(&mut self, v: Self) -> CapturedOld {
		let v = v.into_iter();
		let mut captured = CapturedOld {
			data: Vec::with_capacity(v.size_hint().0),
			old_len: self.data.len(),
		};
		'value: for (key, value) in v {
			for (i, (k, v)) in &mut self.data.iter_mut().enumerate() {
				if k == &key {
					let old_v = std::mem::replace(v, value);
					if i < captured.old_len {
						captured.data.push((i, old_v));
					}
					continue 'value;
				}
			}
			self.data.push((key, value));
		}
		captured
	}
	pub fn overlay_restore(&mut self, captured: CapturedOld) {
		self.data.truncate(captured.old_len);
		for (ele, v) in captured.data.into_iter().rev() {
			self.data[ele].1 = v;
		}
	}
	fn keys(&self) -> impl Iterator<Item = &IStr> {
		self.data.iter().map(|(k, _)| k)
	}
	pub fn len(&self) -> usize {
		self.data.len()
	}
}
impl IntoIterator for BindingsMap {
	type Item = (IStr, BindingValue);

	type IntoIter = std::vec::IntoIter<Self::Item>;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

fn suggest_context_bindings(v: &Context, key: IStr) -> Vec<IStr> {
	let mut heap = Vec::new();
	for k in v.bindings.keys() {
		let conf = strsim::jaro_winkler(k as &str, &key as &str);
		if conf < 0.8 {
			continue;
		}
		heap.push((conf, k.clone()));
	}
	heap.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

	heap.into_iter().map(|v| v.1).collect()
}

/// Context keeps information about current lexical code location
///
/// This information includes local variables, top-level object (`$`), current object (`this`), and super object (`super`)
#[derive(Debug, Trace, Clone)]
pub struct Context {
	pub(crate) dollar: Option<ObjValue>,
	pub(crate) sup_this: Option<SupThis>,
	pub(crate) bindings: BindingsMap,
}
impl Context {
	pub fn new_future() -> Pending<Self> {
		Pending::new()
	}

	pub fn dollar(&self) -> Option<&ObjValue> {
		self.dollar.as_ref()
	}

	pub fn try_dollar(&self) -> Result<ObjValue> {
		self.dollar
			.clone()
			.ok_or_else(|| CantUseSelfSupOutsideOfObject.into())
	}

	pub fn this(&self) -> Option<&ObjValue> {
		self.sup_this.as_ref().map(SupThis::this)
	}

	pub fn try_this(&self) -> Result<ObjValue> {
		self.sup_this
			.as_ref()
			.ok_or_else(|| CantUseSelfSupOutsideOfObject.into())
			.map(SupThis::this)
			.cloned()
	}

	pub fn sup_this(&self) -> Option<&SupThis> {
		self.sup_this.as_ref()
	}

	pub fn try_sup_this(&self) -> Result<SupThis> {
		self.sup_this
			.clone()
			.ok_or_else(|| CantUseSelfSupOutsideOfObject.into())
	}

	pub fn binding(&self, name: IStr) -> Result<Val> {
		let Some(val) = self.bindings.evaluate(&name)? else {
			let suggestions = suggest_context_bindings(self, name.clone());

			bail!(LocalIsNotDefined(name, suggestions,))
		};

		Ok(val)
	}
	pub fn contains_binding(&self, name: IStr) -> bool {
		self.bindings.contains_key(&name)
	}
	#[must_use]
	pub fn into_future(self, ctx: Pending<Self>) -> Self {
		{
			ctx.fill(self.clone());
		}
		self
	}

	#[must_use]
	pub fn with_var(self, name: impl Into<IStr>, value: impl Into<BindingValue>) -> Self {
		self.with_bindings(iter::once((name.into(), value.into())))
	}

	#[must_use]
	pub fn with_bindings_sup_this(
		self,
		new_bindings: impl IntoIterator<Item = (IStr, BindingValue)>,
		sup_this: SupThis,
	) -> Self {
		let mut ctx = ContextBuilder::extend(self);
		ctx.binds(new_bindings).with_sup_this(sup_this);
		ctx.build()
	}
	#[must_use]
	pub fn with_bindings<I>(self, new_bindings: I) -> Self
	where
		I: IntoIterator<Item = (IStr, BindingValue)>,
	{
		let mut ctx = ContextBuilder::extend(self);
		ctx.binds(new_bindings);
		ctx.build()
	}
	pub fn extend_bindings<I>(&mut self, new_bindings: I)
	where
		I: IntoIterator<Item = (IStr, BindingValue)>,
		I::IntoIter: ExactSizeIterator,
	{
		let new_bindings = new_bindings.into_iter();
		if new_bindings.len() == 0 {
			return;
		}
		self.bindings = {
			let mut v = (self.bindings).clone();
			v.extend(new_bindings);
			v
		};
	}
}

pub struct ContextBuilder {
	bindings: BindingsMap,
	sup_this: Option<SupThis>,
	dollar: Option<ObjValue>,
}
impl Default for ContextBuilder {
	fn default() -> Self {
		Self {
			bindings: BindingsMap::new(),
			sup_this: None,
			dollar: None,
		}
	}
}

impl ContextBuilder {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			bindings: BindingsMap::with_capacity(capacity),
			sup_this: None,
			dollar: None,
		}
	}

	pub fn extend(parent: Context) -> Self {
		Self {
			bindings: parent.bindings,
			sup_this: parent.sup_this,
			dollar: parent.dollar,
		}
	}
	pub fn reserve_binds(&mut self, reserve: usize) {
		self.bindings.data.reserve(reserve);
	}

	pub fn with_sup_this(&mut self, sup_this: SupThis) -> &mut Self {
		if self.dollar.is_none() {
			self.dollar = Some(sup_this.this().clone());
		}
		self.sup_this = Some(sup_this);
		self
	}

	/// Returns true if binding is new
	pub fn bind(&mut self, name: impl Into<IStr>, value: impl Into<BindingValue>) -> bool {
		self.bindings.insert(name.into(), value)
	}
	pub fn binds(&mut self, bindings: impl IntoIterator<Item = (IStr, BindingValue)>) -> &mut Self {
		let iter = bindings.into_iter();
		let (min, _) = iter.size_hint();
		self.bindings.data.reserve(min);
		self.bindings.extend(iter);
		self
	}
	pub fn build(self) -> Context {
		Context {
			bindings: self.bindings,
			sup_this: self.sup_this,
			dollar: self.dollar,
		}
	}
}
