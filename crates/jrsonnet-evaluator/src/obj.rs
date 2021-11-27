use crate::gc::{GcHashMap, GcHashSet, TraceBox};
use crate::operator::evaluate_add_op;
use crate::{cc_ptr_eq, Bindable, LazyBinding, LazyVal, Result, Val};
use gcmodule::{Cc, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ExprLocation, Visibility};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

#[derive(Debug, Trace)]
pub struct ObjMember {
	pub add: bool,
	pub visibility: Visibility,
	pub invoke: LazyBinding,
	pub location: Option<ExprLocation>,
}

pub trait ObjectAssertion: Trace {
	fn run(&self, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<()>;
}

// Field => This
type CacheKey = (IStr, ObjValue);
#[derive(Trace)]
#[force_tracking]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	assertions: Cc<Vec<TraceBox<dyn ObjectAssertion>>>,
	assertions_ran: RefCell<GcHashSet<ObjValue>>,
	this_obj: Option<ObjValue>,
	this_entries: Cc<GcHashMap<IStr, ObjMember>>,
	value_cache: RefCell<GcHashMap<CacheKey, Option<Val>>>,
}

#[derive(Clone, Trace)]
pub struct ObjValue(pub(crate) Cc<ObjValueInternals>);
impl Debug for ObjValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(super_obj) = self.0.super_obj.as_ref() {
			if f.alternate() {
				write!(f, "{:#?}", super_obj)?;
			} else {
				write!(f, "{:?}", super_obj)?;
			}
			write!(f, " + ")?;
		}
		let mut debug = f.debug_struct("ObjValue");
		for (name, member) in self.0.this_entries.iter() {
			debug.field(name, member);
		}
		#[cfg(feature = "unstable")]
		{
			debug.finish_non_exhaustive()
		}
		#[cfg(not(feature = "unstable"))]
		{
			debug.finish()
		}
	}
}

impl ObjValue {
	pub fn new(
		super_obj: Option<Self>,
		this_entries: Cc<GcHashMap<IStr, ObjMember>>,
		assertions: Cc<Vec<TraceBox<dyn ObjectAssertion>>>,
	) -> Self {
		Self(Cc::new(ObjValueInternals {
			super_obj,
			assertions,
			assertions_ran: RefCell::new(GcHashSet::new()),
			this_obj: None,
			this_entries,
			value_cache: RefCell::new(GcHashMap::new()),
		}))
	}
	pub fn new_empty() -> Self {
		Self::new(None, Cc::new(GcHashMap::new()), Cc::new(Vec::new()))
	}
	pub fn extend_from(&self, super_obj: Self) -> Self {
		match &self.0.super_obj {
			None => Self::new(
				Some(super_obj),
				self.0.this_entries.clone(),
				self.0.assertions.clone(),
			),
			Some(v) => Self::new(
				Some(v.extend_from(super_obj)),
				self.0.this_entries.clone(),
				self.0.assertions.clone(),
			),
		}
	}
	pub fn with_this(&self, this_obj: Self) -> Self {
		Self(Cc::new(ObjValueInternals {
			super_obj: self.0.super_obj.clone(),
			assertions: self.0.assertions.clone(),
			assertions_ran: RefCell::new(GcHashSet::new()),
			this_obj: Some(this_obj),
			this_entries: self.0.this_entries.clone(),
			value_cache: RefCell::new(GcHashMap::new()),
		}))
	}

	pub fn is_empty(&self) -> bool {
		if !self.0.this_entries.is_empty() {
			return false;
		}
		self.0
			.super_obj
			.as_ref()
			.map(|s| s.is_empty())
			.unwrap_or(true)
	}

	/// Run callback for every field found in object
	pub(crate) fn enum_fields(&self, handler: &mut impl FnMut(&IStr, &ObjMember) -> bool) -> bool {
		if let Some(s) = &self.0.super_obj {
			if s.enum_fields(handler) {
				return true;
			}
		}
		for (name, member) in self.0.this_entries.iter() {
			if handler(name, member) {
				return true;
			}
		}
		false
	}

	pub fn fields_visibility(&self) -> FxHashMap<IStr, bool> {
		let mut out = FxHashMap::default();
		self.enum_fields(&mut |name, member| {
			match member.visibility {
				Visibility::Normal => {
					let entry = out.entry(name.to_owned());
					entry.or_insert(true);
				}
				Visibility::Hidden => {
					out.insert(name.to_owned(), false);
				}
				Visibility::Unhide => {
					out.insert(name.to_owned(), true);
				}
			};
			false
		});
		out
	}
	pub fn fields_ex(&self, include_hidden: bool) -> Vec<IStr> {
		let mut fields: Vec<_> = self
			.fields_visibility()
			.into_iter()
			.filter(|(_k, v)| include_hidden || *v)
			.map(|(k, _)| k)
			.collect();
		fields.sort_unstable();
		fields
	}
	pub fn fields(&self) -> Vec<IStr> {
		self.fields_ex(false)
	}

	pub fn field_visibility(&self, name: IStr) -> Option<Visibility> {
		if let Some(m) = self.0.this_entries.get(&name) {
			Some(match &m.visibility {
				Visibility::Normal => self
					.0
					.super_obj
					.as_ref()
					.and_then(|super_obj| super_obj.field_visibility(name))
					.unwrap_or(Visibility::Normal),
				v => *v,
			})
		} else if let Some(super_obj) = &self.0.super_obj {
			super_obj.field_visibility(name)
		} else {
			None
		}
	}

	fn has_field_include_hidden(&self, name: IStr) -> bool {
		if self.0.this_entries.contains_key(&name) {
			true
		} else if let Some(super_obj) = &self.0.super_obj {
			super_obj.has_field_include_hidden(name)
		} else {
			false
		}
	}

	pub fn has_field_ex(&self, name: IStr, include_hidden: bool) -> bool {
		if include_hidden {
			self.has_field_include_hidden(name)
		} else {
			self.has_field(name)
		}
	}
	pub fn has_field(&self, name: IStr) -> bool {
		self.field_visibility(name)
			.map(|v| v.is_visible())
			.unwrap_or(false)
	}

	pub fn get(&self, key: IStr) -> Result<Option<Val>> {
		self.run_assertions()?;
		self.get_raw(key, self.0.this_obj.as_ref())
	}

	pub fn extend_with_field(self, key: IStr, value: ObjMember) -> Self {
		let mut new = GcHashMap::with_capacity(1);
		new.insert(key, value);
		Self::new(Some(self), Cc::new(new), Cc::new(Vec::new()))
	}

	fn get_raw(&self, key: IStr, real_this: Option<&Self>) -> Result<Option<Val>> {
		let real_this = real_this.unwrap_or(self);
		let cache_key = (key.clone(), real_this.clone());

		if let Some(v) = self.0.value_cache.borrow().get(&cache_key) {
			return Ok(v.clone());
		}
		let value = match (self.0.this_entries.get(&key), &self.0.super_obj) {
			(Some(k), None) => Ok(Some(self.evaluate_this(k, real_this)?)),
			(Some(k), Some(s)) => {
				let our = self.evaluate_this(k, real_this)?;
				if k.add {
					s.get_raw(key, Some(real_this))?
						.map_or(Ok(Some(our.clone())), |v| {
							Ok(Some(evaluate_add_op(&v, &our)?))
						})
				} else {
					Ok(Some(our))
				}
			}
			(None, Some(s)) => s.get_raw(key, Some(real_this)),
			(None, None) => Ok(None),
		}?;
		self.0
			.value_cache
			.borrow_mut()
			.insert(cache_key, value.clone());
		Ok(value)
	}
	fn evaluate_this(&self, v: &ObjMember, real_this: &Self) -> Result<Val> {
		v.invoke
			.evaluate(Some(real_this.clone()), self.0.super_obj.clone())?
			.evaluate()
	}

	fn run_assertions_raw(&self, real_this: &Self) -> Result<()> {
		if self.0.assertions_ran.borrow_mut().insert(real_this.clone()) {
			for assertion in self.0.assertions.iter() {
				if let Err(e) = assertion.run(Some(real_this.clone()), self.0.super_obj.clone()) {
					self.0.assertions_ran.borrow_mut().remove(real_this);
					return Err(e);
				}
			}
			if let Some(super_obj) = &self.0.super_obj {
				super_obj.run_assertions_raw(real_this)?;
			}
		}
		Ok(())
	}
	pub fn run_assertions(&self) -> Result<()> {
		self.run_assertions_raw(self)
	}

	pub fn ptr_eq(a: &Self, b: &Self) -> bool {
		cc_ptr_eq(&a.0, &b.0)
	}
}

impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		cc_ptr_eq(&self.0, &other.0)
	}
}

impl Eq for ObjValue {}
impl Hash for ObjValue {
	fn hash<H: Hasher>(&self, hasher: &mut H) {
		hasher.write_usize(&*self.0 as *const _ as usize)
	}
}

pub struct ObjValueBuilder {
	super_obj: Option<ObjValue>,
	map: GcHashMap<IStr, ObjMember>,
	assertions: Vec<TraceBox<dyn ObjectAssertion>>,
}
impl ObjValueBuilder {
	pub fn new() -> Self {
		Self::with_capacity(0)
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			super_obj: None,
			map: GcHashMap::with_capacity(capacity),
			assertions: Vec::new(),
		}
	}
	pub fn reserve_asserts(&mut self, capacity: usize) -> &mut Self {
		self.assertions.reserve_exact(capacity);
		self
	}
	pub fn with_super(&mut self, super_obj: ObjValue) -> &mut Self {
		self.super_obj = Some(super_obj);
		self
	}

	pub fn assert(&mut self, assertion: TraceBox<dyn ObjectAssertion>) -> &mut Self {
		self.assertions.push(assertion);
		self
	}
	pub fn member(&mut self, name: IStr) -> ObjMemberBuilder {
		ObjMemberBuilder {
			value: self,
			name,
			add: false,
			visibility: Visibility::Normal,
			location: None,
		}
	}

	pub fn build(self) -> ObjValue {
		ObjValue::new(self.super_obj, Cc::new(self.map), Cc::new(self.assertions))
	}
}
impl Default for ObjValueBuilder {
	fn default() -> Self {
		Self::with_capacity(0)
	}
}

#[must_use = "value not added unless binding() was called"]
pub struct ObjMemberBuilder<'v> {
	value: &'v mut ObjValueBuilder,
	name: IStr,
	add: bool,
	visibility: Visibility,
	location: Option<ExprLocation>,
}

#[allow(clippy::missing_const_for_fn)]
impl<'v> ObjMemberBuilder<'v> {
	pub const fn with_add(mut self, add: bool) -> Self {
		self.add = add;
		self
	}
	pub fn add(self) -> Self {
		self.with_add(true)
	}
	pub fn with_visibility(mut self, visibility: Visibility) -> Self {
		self.visibility = visibility;
		self
	}
	pub fn hide(self) -> Self {
		self.with_visibility(Visibility::Hidden)
	}
	pub fn with_location(mut self, location: ExprLocation) -> Self {
		self.location = Some(location);
		self
	}
	pub fn value(self, value: Val) -> &'v mut ObjValueBuilder {
		self.binding(LazyBinding::Bound(LazyVal::new_resolved(value)))
	}
	pub fn bindable(self, bindable: TraceBox<dyn Bindable>) -> &'v mut ObjValueBuilder {
		self.binding(LazyBinding::Bindable(Cc::new(bindable)))
	}
	pub fn binding(self, binding: LazyBinding) -> &'v mut ObjValueBuilder {
		self.value.map.insert(
			self.name,
			ObjMember {
				add: self.add,
				visibility: self.visibility,
				invoke: binding,
				location: self.location,
			},
		);
		self.value
	}
}
