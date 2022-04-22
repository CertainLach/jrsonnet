use std::{
	cell::RefCell,
	fmt::Debug,
	hash::{Hash, Hasher},
	ptr::addr_of,
};

use gcmodule::{Cc, Trace, Weak};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ExprLocation, Visibility};
use rustc_hash::FxHashMap;

use crate::{
	cc_ptr_eq,
	error::{Error::*, LocError},
	function::CallLocation,
	gc::{GcHashMap, GcHashSet, TraceBox},
	operator::evaluate_add_op,
	throw, weak_ptr_eq, weak_raw, Bindable, LazyBinding, LazyVal, Result, State, Val,
};

#[cfg(not(feature = "exp-preserve-order"))]
mod ordering {
	#![allow(
		// This module works as stub for preserve-order feature
		clippy::unused_self,
	)]

	use gcmodule::Trace;

	#[derive(Clone, Copy, Default, Debug, Trace)]
	pub struct FieldIndex;
	impl FieldIndex {
		pub const fn next(self) -> Self {
			Self
		}
	}

	#[derive(Clone, Copy, Default, Debug, Trace)]
	pub struct SuperDepth;
	impl SuperDepth {
		pub const fn deeper(self) -> Self {
			Self
		}
	}

	#[derive(Clone, Copy)]
	pub struct FieldSortKey;
	impl FieldSortKey {
		pub const fn new(_: SuperDepth, _: FieldIndex) -> Self {
			Self
		}
	}
}

#[cfg(feature = "exp-preserve-order")]
mod ordering {
	use std::cmp::Reverse;

	use gcmodule::Trace;

	#[derive(Clone, Copy, Default, Debug, Trace, PartialEq, Eq, PartialOrd, Ord)]
	pub struct FieldIndex(u32);
	impl FieldIndex {
		pub fn next(self) -> Self {
			Self(self.0 + 1)
		}
	}

	#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
	pub struct SuperDepth(u32);
	impl SuperDepth {
		pub fn deeper(self) -> Self {
			Self(self.0 + 1)
		}
	}

	#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
	pub struct FieldSortKey(Reverse<SuperDepth>, FieldIndex);
	impl FieldSortKey {
		pub fn new(depth: SuperDepth, index: FieldIndex) -> Self {
			Self(Reverse(depth), index)
		}
		pub fn collide(self, other: Self) -> Self {
			if self.0 .0 > other.0 .0 {
				self
			} else if self.0 .0 < other.0 .0 {
				other
			} else {
				unreachable!("object can't have two fields with same name")
			}
		}
	}
}

use ordering::*;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Trace)]
pub struct ObjMember {
	pub add: bool,
	pub visibility: Visibility,
	original_index: FieldIndex,
	pub invoke: LazyBinding,
	pub location: Option<ExprLocation>,
}

pub trait ObjectAssertion: Trace {
	fn run(&self, s: State, this: Option<ObjValue>, super_obj: Option<ObjValue>) -> Result<()>;
}

// Field => This
type CacheKey = (IStr, WeakObjValue);

#[derive(Trace)]
enum CacheValue {
	Cached(Val),
	NotFound,
	Pending,
	Errored(LocError),
}

#[allow(clippy::module_name_repetitions)]
#[derive(Trace)]
#[force_tracking]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	assertions: Cc<Vec<TraceBox<dyn ObjectAssertion>>>,
	assertions_ran: RefCell<GcHashSet<ObjValue>>,
	this_obj: Option<ObjValue>,
	this_entries: Cc<GcHashMap<IStr, ObjMember>>,
	value_cache: RefCell<GcHashMap<CacheKey, CacheValue>>,
}

#[derive(Clone, Trace)]
pub struct WeakObjValue(#[skip_trace] pub(crate) Weak<ObjValueInternals>);

impl PartialEq for WeakObjValue {
	fn eq(&self, other: &Self) -> bool {
		weak_ptr_eq(self.0.clone(), other.0.clone())
	}
}

impl Eq for WeakObjValue {}
impl Hash for WeakObjValue {
	fn hash<H: Hasher>(&self, hasher: &mut H) {
		hasher.write_usize(weak_raw(self.0.clone()) as usize);
	}
}

#[allow(clippy::module_name_repetitions)]
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
		debug.finish_non_exhaustive()
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
	#[must_use]
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
	pub(crate) fn extend_with_raw_member(self, key: IStr, value: ObjMember) -> Self {
		let mut new = GcHashMap::with_capacity(1);
		new.insert(key, value);
		Self::new(Some(self), Cc::new(new), Cc::new(Vec::new()))
	}
	pub fn extend_field(&mut self, name: IStr) -> ObjMemberBuilder<ExtendBuilder> {
		ObjMemberBuilder::new(ExtendBuilder(self), name, FieldIndex::default())
	}

	#[must_use]
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

	pub fn len(&self) -> usize {
		self.fields_visibility()
			.into_iter()
			.filter(|(_, (visible, _))| *visible)
			.count()
	}

	pub fn is_empty(&self) -> bool {
		if !self.0.this_entries.is_empty() {
			return false;
		}
		self.0.super_obj.as_ref().map_or(true, Self::is_empty)
	}

	/// Run callback for every field found in object
	pub(crate) fn enum_fields(
		&self,
		depth: SuperDepth,
		handler: &mut impl FnMut(SuperDepth, &IStr, &ObjMember) -> bool,
	) -> bool {
		if let Some(s) = &self.0.super_obj {
			if s.enum_fields(depth.deeper(), handler) {
				return true;
			}
		}
		for (name, member) in self.0.this_entries.iter() {
			if handler(depth, name, member) {
				return true;
			}
		}
		false
	}

	pub fn fields_visibility(&self) -> FxHashMap<IStr, (bool, FieldSortKey)> {
		let mut out = FxHashMap::default();
		self.enum_fields(SuperDepth::default(), &mut |depth, name, member| {
			let new_sort_key = FieldSortKey::new(depth, member.original_index);
			match member.visibility {
				Visibility::Normal => {
					let entry = out.entry(name.clone());
					let v = entry.or_insert((true, new_sort_key));
					v.1 = new_sort_key;
				}
				Visibility::Hidden => {
					out.insert(name.clone(), (false, new_sort_key));
				}
				Visibility::Unhide => {
					out.insert(name.clone(), (true, new_sort_key));
				}
			};
			false
		});
		out
	}
	pub fn fields_ex(
		&self,
		include_hidden: bool,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> Vec<IStr> {
		#[cfg(feature = "exp-preserve-order")]
		if preserve_order {
			let (mut fields, mut keys): (Vec<_>, Vec<_>) = self
				.fields_visibility()
				.into_iter()
				.filter(|(_, (visible, _))| include_hidden || *visible)
				.enumerate()
				.map(|(idx, (k, (_, sk)))| (k, (sk, idx)))
				.unzip();
			keys.sort_unstable_by_key(|v| v.0);
			// Reorder in-place by resulting indexes
			for i in 0..fields.len() {
				let x = fields[i].clone();
				let mut j = i;
				loop {
					let k = keys[j].1;
					keys[j].1 = j;
					if k == i {
						break;
					}
					fields[j] = fields[k].clone();
					j = k
				}
				fields[j] = x;
			}
			return fields;
		}

		let mut fields: Vec<_> = self
			.fields_visibility()
			.into_iter()
			.filter(|(_, (visible, _))| include_hidden || *visible)
			.map(|(k, _)| k)
			.collect();
		fields.sort_unstable();
		fields
	}
	pub fn fields(&self, #[cfg(feature = "exp-preserve-order")] preserve_order: bool) -> Vec<IStr> {
		self.fields_ex(
			false,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		)
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
			.map_or(false, |v| v.is_visible())
	}

	pub fn get(&self, s: State, key: IStr) -> Result<Option<Val>> {
		self.run_assertions(s.clone())?;
		self.get_raw(s, key, self.0.this_obj.as_ref())
	}

	// pub fn extend_with(self, key: )

	fn get_raw(&self, s: State, key: IStr, real_this: Option<&Self>) -> Result<Option<Val>> {
		let real_this = real_this.unwrap_or(self);
		let cache_key = (key.clone(), WeakObjValue(real_this.0.downgrade()));

		if let Some(v) = self.0.value_cache.borrow().get(&cache_key) {
			return Ok(match v {
				CacheValue::Cached(v) => Some(v.clone()),
				CacheValue::NotFound => None,
				CacheValue::Pending => throw!(InfiniteRecursionDetected),
				CacheValue::Errored(e) => return Err(e.clone()),
			});
		}
		self.0
			.value_cache
			.borrow_mut()
			.insert(cache_key.clone(), CacheValue::Pending);
		let fill_error = |e: LocError| {
			self.0
				.value_cache
				.borrow_mut()
				.insert(cache_key.clone(), CacheValue::Errored(e.clone()));
			e
		};
		let value = match (self.0.this_entries.get(&key), &self.0.super_obj) {
			(Some(k), None) => Ok(Some(
				self.evaluate_this(s, k, real_this).map_err(fill_error)?,
			)),
			(Some(k), Some(super_obj)) => {
				let our = self
					.evaluate_this(s.clone(), k, real_this)
					.map_err(fill_error)?;
				if k.add {
					super_obj
						.get_raw(s.clone(), key, Some(real_this))
						.map_err(fill_error)?
						.map_or(Ok(Some(our.clone())), |v| {
							Ok(Some(evaluate_add_op(s.clone(), &v, &our)?))
						})
				} else {
					Ok(Some(our))
				}
			}
			(None, Some(super_obj)) => super_obj.get_raw(s, key, Some(real_this)),
			(None, None) => Ok(None),
		}
		.map_err(fill_error)?;
		self.0.value_cache.borrow_mut().insert(
			cache_key,
			match &value {
				Some(v) => CacheValue::Cached(v.clone()),
				None => CacheValue::NotFound,
			},
		);
		Ok(value)
	}
	fn evaluate_this(&self, s: State, v: &ObjMember, real_this: &Self) -> Result<Val> {
		v.invoke
			.evaluate(s.clone(), Some(real_this.clone()), self.0.super_obj.clone())?
			.evaluate(s)
	}

	fn run_assertions_raw(&self, s: State, real_this: &Self) -> Result<()> {
		if self.0.assertions_ran.borrow_mut().insert(real_this.clone()) {
			for assertion in self.0.assertions.iter() {
				if let Err(e) =
					assertion.run(s.clone(), Some(real_this.clone()), self.0.super_obj.clone())
				{
					self.0.assertions_ran.borrow_mut().remove(real_this);
					return Err(e);
				}
			}
			if let Some(super_obj) = &self.0.super_obj {
				super_obj.run_assertions_raw(s, real_this)?;
			}
		}
		Ok(())
	}
	pub fn run_assertions(&self, s: State) -> Result<()> {
		self.run_assertions_raw(s, self)
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
		hasher.write_usize(addr_of!(*self.0) as usize);
	}
}

#[allow(clippy::module_name_repetitions)]
pub struct ObjValueBuilder {
	super_obj: Option<ObjValue>,
	map: GcHashMap<IStr, ObjMember>,
	assertions: Vec<TraceBox<dyn ObjectAssertion>>,
	next_field_index: FieldIndex,
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
			next_field_index: FieldIndex::default(),
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
	pub fn member(&mut self, name: IStr) -> ObjMemberBuilder<ValueBuilder> {
		let field_index = self.next_field_index;
		self.next_field_index = self.next_field_index.next();
		ObjMemberBuilder::new(ValueBuilder(self), name, field_index)
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

#[allow(clippy::module_name_repetitions)]
#[must_use = "value not added unless binding() was called"]
pub struct ObjMemberBuilder<Kind> {
	kind: Kind,
	name: IStr,
	add: bool,
	visibility: Visibility,
	original_index: FieldIndex,
	location: Option<ExprLocation>,
}

#[allow(clippy::missing_const_for_fn)]
impl<Kind> ObjMemberBuilder<Kind> {
	pub(crate) fn new(kind: Kind, name: IStr, original_index: FieldIndex) -> Self {
		Self {
			kind,
			name,
			original_index,
			add: false,
			visibility: Visibility::Normal,
			location: None,
		}
	}

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
	fn build_member(self, binding: LazyBinding) -> (Kind, IStr, ObjMember) {
		(
			self.kind,
			self.name,
			ObjMember {
				add: self.add,
				visibility: self.visibility,
				original_index: self.original_index,
				invoke: binding,
				location: self.location,
			},
		)
	}
}

pub struct ValueBuilder<'v>(&'v mut ObjValueBuilder);
impl<'v> ObjMemberBuilder<ValueBuilder<'v>> {
	pub fn value(self, s: State, value: Val) -> Result<()> {
		self.binding(s, LazyBinding::Bound(LazyVal::new_resolved(value)))
	}
	pub fn bindable(self, s: State, bindable: TraceBox<dyn Bindable>) -> Result<()> {
		self.binding(s, LazyBinding::Bindable(Cc::new(bindable)))
	}
	pub fn binding(self, s: State, binding: LazyBinding) -> Result<()> {
		let (receiver, name, member) = self.build_member(binding);
		let location = member.location.clone();
		let old = receiver.0.map.insert(name.clone(), member);
		if old.is_some() {
			s.push(
				CallLocation(location.as_ref()),
				|| format!("field <{}> initializtion", name.clone()),
				|| throw!(DuplicateFieldName(name.clone())),
			)?;
		}
		Ok(())
	}
}

pub struct ExtendBuilder<'v>(&'v mut ObjValue);
impl<'v> ObjMemberBuilder<ExtendBuilder<'v>> {
	pub fn value(self, value: Val) {
		self.binding(LazyBinding::Bound(LazyVal::new_resolved(value)));
	}
	pub fn bindable(self, bindable: TraceBox<dyn Bindable>) {
		self.binding(LazyBinding::Bindable(Cc::new(bindable)));
	}
	pub fn binding(self, binding: LazyBinding) {
		let (receiver, name, member) = self.build_member(binding);
		let new = receiver.0.clone();
		*receiver.0 = new.extend_with_raw_member(name, member);
	}
}
