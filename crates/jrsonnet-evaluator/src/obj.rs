use crate::operator::evaluate_add_op;
use crate::{Bindable, LazyBinding, LazyVal, Result, Val};
use jrsonnet_gc::{Gc, GcCell, Trace};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ExprLocation, Visibility};
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::{fmt::Debug, hash::BuildHasherDefault};

#[derive(Debug, Trace)]
#[trivially_drop]
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
#[trivially_drop]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	assertions: Gc<Vec<Box<dyn ObjectAssertion>>>,
	assertions_ran: GcCell<FxHashSet<ObjValue>>,
	this_obj: Option<ObjValue>,
	this_entries: Gc<FxHashMap<IStr, ObjMember>>,
	value_cache: GcCell<FxHashMap<CacheKey, Option<Val>>>,
}

#[derive(Clone, Trace)]
#[trivially_drop]
pub struct ObjValue(pub(crate) Gc<ObjValueInternals>);
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
		this_entries: Gc<FxHashMap<IStr, ObjMember>>,
		assertions: Gc<Vec<Box<dyn ObjectAssertion>>>,
	) -> Self {
		Self(Gc::new(ObjValueInternals {
			super_obj,
			assertions,
			assertions_ran: GcCell::new(FxHashSet::default()),
			this_obj: None,
			this_entries,
			value_cache: GcCell::new(FxHashMap::default()),
		}))
	}
	pub fn new_empty() -> Self {
		Self::new(None, Gc::new(FxHashMap::default()), Gc::new(Vec::new()))
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
		Self(Gc::new(ObjValueInternals {
			super_obj: self.0.super_obj.clone(),
			assertions: self.0.assertions.clone(),
			assertions_ran: GcCell::new(FxHashSet::default()),
			this_obj: Some(this_obj),
			this_entries: self.0.this_entries.clone(),
			value_cache: GcCell::new(FxHashMap::default()),
		}))
	}

	/// Run callback for every field found in object
	pub(crate) fn enum_fields(&self, handler: &mut impl FnMut(&IStr, &Visibility) -> bool) -> bool {
		if let Some(s) = &self.0.super_obj {
			if s.enum_fields(handler) {
				return true;
			}
		}
		for (name, member) in self.0.this_entries.iter() {
			if handler(name, &member.visibility) {
				return true;
			}
		}
		false
	}

	pub fn fields_visibility(&self) -> FxHashMap<IStr, bool> {
		let mut out = FxHashMap::default();
		self.enum_fields(&mut |name, visibility| {
			match visibility {
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
		let mut new = FxHashMap::with_capacity_and_hasher(1, BuildHasherDefault::default());
		new.insert(key, value);
		Self::new(Some(self), Gc::new(new), Gc::new(Vec::new()))
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
		Gc::ptr_eq(&a.0, &b.0)
	}
}

impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		Gc::ptr_eq(&self.0, &other.0)
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
	map: FxHashMap<IStr, ObjMember>,
	assertions: Vec<Box<dyn ObjectAssertion>>,
}
impl ObjValueBuilder {
	pub fn new() -> Self {
		Self::with_capacity(0)
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			super_obj: None,
			map: HashMap::with_capacity_and_hasher(
				capacity,
				BuildHasherDefault::<FxHasher>::default(),
			),
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

	pub fn assert(&mut self, assertion: Box<dyn ObjectAssertion>) -> &mut Self {
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
		ObjValue::new(self.super_obj, Gc::new(self.map), Gc::new(self.assertions))
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
	pub fn with_location(mut self, location: Option<ExprLocation>) -> Self {
		self.location = location;
		self
	}
	pub fn value(self, value: Val) -> &'v mut ObjValueBuilder {
		self.binding(LazyBinding::Bound(LazyVal::new_resolved(value)))
	}
	pub fn bindable(self, bindable: Box<dyn Bindable>) -> &'v mut ObjValueBuilder {
		self.binding(LazyBinding::Bindable(Gc::new(bindable)))
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
