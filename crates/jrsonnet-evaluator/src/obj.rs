use std::{
	any::Any,
	cell::RefCell,
	fmt::Debug,
	hash::{Hash, Hasher},
	ptr::addr_of,
};

use jrsonnet_gcmodule::{Cc, Trace, Weak};
use jrsonnet_interner::IStr;
use jrsonnet_parser::{Span, Visibility};
use rustc_hash::FxHashMap;

use crate::{
	arr::{PickObjectKeyValues, PickObjectValues},
	bail,
	error::{suggest_object_fields, Error, ErrorKind::*},
	function::{CallLocation, FuncVal},
	gc::{GcHashMap, GcHashSet, TraceBox},
	operator::evaluate_add_op,
	tb,
	val::{ArrValue, ThunkValue},
	MaybeUnbound, Result, State, Thunk, Unbound, Val,
};

#[cfg(not(feature = "exp-preserve-order"))]
mod ordering {
	#![allow(
		// This module works as stub for preserve-order feature
		clippy::unused_self,
	)]

	use jrsonnet_gcmodule::Trace;

	#[derive(Clone, Copy, Default, Debug, Trace)]
	pub struct FieldIndex(());
	impl FieldIndex {
		pub const fn next(self) -> Self {
			Self(())
		}
	}

	#[derive(Clone, Copy, Default, Debug, Trace)]
	pub struct SuperDepth(());
	impl SuperDepth {
		pub const fn deeper(self) -> Self {
			Self(())
		}
	}

	#[derive(Clone, Copy)]
	pub struct FieldSortKey(());
	impl FieldSortKey {
		pub const fn new(_: SuperDepth, _: FieldIndex) -> Self {
			Self(())
		}
	}
}

#[cfg(feature = "exp-preserve-order")]
mod ordering {
	use std::cmp::Reverse;

	use jrsonnet_gcmodule::Trace;

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
	}
}

use ordering::{FieldIndex, FieldSortKey, SuperDepth};

// 0 - add
//  12 - visibility
#[derive(Clone, Copy)]
pub struct ObjFieldFlags(u8);
impl ObjFieldFlags {
	fn new(add: bool, visibility: Visibility) -> Self {
		let mut v = 0;
		if add {
			v |= 1;
		}
		v |= match visibility {
			Visibility::Normal => 0b000,
			Visibility::Hidden => 0b010,
			Visibility::Unhide => 0b100,
		};
		Self(v)
	}
	pub fn add(&self) -> bool {
		self.0 & 1 != 0
	}
	pub fn visibility(&self) -> Visibility {
		match (self.0 & 0b110) >> 1 {
			0b00 => Visibility::Normal,
			0b01 => Visibility::Hidden,
			0b10 => Visibility::Unhide,
			_ => unreachable!(),
		}
	}
}
impl Debug for ObjFieldFlags {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ObjFieldFlags")
			.field("add", &self.add())
			.field("visibility", &self.visibility())
			.finish()
	}
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Trace)]
pub struct ObjMember {
	#[trace(skip)]
	flags: ObjFieldFlags,
	original_index: FieldIndex,
	pub invoke: MaybeUnbound,
	pub location: Option<Span>,
}

pub trait ObjectAssertion: Trace {
	fn run(&self, super_obj: Option<ObjValue>, this: Option<ObjValue>) -> Result<()>;
}

// Field => This

#[derive(Trace)]
enum CacheValue {
	Cached(Val),
	NotFound,
	Pending,
	Errored(Error),
}

#[allow(clippy::module_name_repetitions)]
#[derive(Trace)]
#[trace(tracking(force))]
pub struct OopObject {
	sup: Option<ObjValue>,
	// this: Option<ObjValue>,
	assertions: Cc<Vec<TraceBox<dyn ObjectAssertion>>>,
	assertions_ran: RefCell<GcHashSet<ObjValue>>,
	this_entries: Cc<GcHashMap<IStr, ObjMember>>,
	value_cache: RefCell<GcHashMap<(IStr, Option<WeakObjValue>), CacheValue>>,
}
impl Debug for OopObject {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("OopObject")
			.field("sup", &self.sup)
			// .field("assertions", &self.assertions)
			// .field("assertions_ran", &self.assertions_ran)
			.field("this_entries", &self.this_entries)
			// .field("value_cache", &self.value_cache)
			.finish_non_exhaustive()
	}
}

type EnumFieldsHandler<'a> = dyn FnMut(SuperDepth, FieldIndex, IStr, Visibility) -> bool + 'a;

pub trait ObjectLike: Trace + Any + Debug {
	fn extend_from(&self, sup: ObjValue) -> ObjValue;
	/// When using standalone super in object, `this.super_obj.with_this(this)` is executed
	fn with_this(&self, me: ObjValue, this: ObjValue) -> ObjValue {
		ObjValue::new(ThisOverride { inner: me, this })
	}
	fn this(&self) -> Option<ObjValue> {
		None
	}
	fn len(&self) -> usize;
	fn is_empty(&self) -> bool;
	// If callback returns false, iteration stops
	fn enum_fields(&self, depth: SuperDepth, handler: &mut EnumFieldsHandler<'_>) -> bool;

	fn has_field_include_hidden(&self, name: IStr) -> bool;
	fn has_field(&self, name: IStr) -> bool;

	fn get_for(&self, key: IStr, this: ObjValue) -> Result<Option<Val>>;
	fn get_for_uncached(&self, key: IStr, this: ObjValue) -> Result<Option<Val>>;
	fn field_visibility(&self, field: IStr) -> Option<Visibility>;

	fn run_assertions_raw(&self, this: ObjValue) -> Result<()>;
}

#[derive(Clone, Trace)]
pub struct WeakObjValue(#[trace(skip)] pub(crate) Weak<TraceBox<dyn ObjectLike>>);

impl PartialEq for WeakObjValue {
	fn eq(&self, other: &Self) -> bool {
		Weak::ptr_eq(&self.0, &other.0)
	}
}

impl Eq for WeakObjValue {}
impl Hash for WeakObjValue {
	fn hash<H: Hasher>(&self, hasher: &mut H) {
		// Safety: usize is POD
		let addr = unsafe { *std::ptr::addr_of!(self.0).cast() };
		hasher.write_usize(addr);
	}
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Trace, Debug)]
pub struct ObjValue(pub(crate) Cc<TraceBox<dyn ObjectLike>>);

#[derive(Debug, Trace)]
struct EmptyObject;
impl ObjectLike for EmptyObject {
	fn extend_from(&self, sup: ObjValue) -> ObjValue {
		// obj + {} == obj
		sup
	}

	fn this(&self) -> Option<ObjValue> {
		None
	}

	fn len(&self) -> usize {
		0
	}

	fn is_empty(&self) -> bool {
		true
	}

	fn enum_fields(&self, _depth: SuperDepth, _handler: &mut EnumFieldsHandler<'_>) -> bool {
		false
	}

	fn has_field_include_hidden(&self, _name: IStr) -> bool {
		false
	}

	fn has_field(&self, _name: IStr) -> bool {
		false
	}

	fn get_for(&self, _key: IStr, _this: ObjValue) -> Result<Option<Val>> {
		Ok(None)
	}
	fn get_for_uncached(&self, _key: IStr, _this: ObjValue) -> Result<Option<Val>> {
		Ok(None)
	}

	fn run_assertions_raw(&self, _this: ObjValue) -> Result<()> {
		Ok(())
	}

	fn field_visibility(&self, _field: IStr) -> Option<Visibility> {
		None
	}
}

#[derive(Trace, Debug)]
struct ThisOverride {
	inner: ObjValue,
	this: ObjValue,
}
impl ObjectLike for ThisOverride {
	fn with_this(&self, _me: ObjValue, this: ObjValue) -> ObjValue {
		ObjValue::new(Self {
			inner: self.inner.clone(),
			this,
		})
	}

	fn extend_from(&self, sup: ObjValue) -> ObjValue {
		self.inner.extend_from(sup).with_this(self.this.clone())
	}

	fn this(&self) -> Option<ObjValue> {
		Some(self.this.clone())
	}

	fn len(&self) -> usize {
		self.inner.len()
	}

	fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	fn enum_fields(&self, depth: SuperDepth, handler: &mut EnumFieldsHandler<'_>) -> bool {
		self.inner.enum_fields(depth, handler)
	}

	fn has_field_include_hidden(&self, name: IStr) -> bool {
		self.inner.has_field_include_hidden(name)
	}

	fn has_field(&self, name: IStr) -> bool {
		self.inner.has_field(name)
	}

	fn get_for(&self, key: IStr, this: ObjValue) -> Result<Option<Val>> {
		self.inner.get_for(key, this)
	}

	fn get_for_uncached(&self, key: IStr, this: ObjValue) -> Result<Option<Val>> {
		self.inner.get_raw(key, this)
	}

	fn field_visibility(&self, field: IStr) -> Option<Visibility> {
		self.inner.field_visibility(field)
	}

	fn run_assertions_raw(&self, this: ObjValue) -> Result<()> {
		self.inner.run_assertions_raw(this)
	}
}

impl ObjValue {
	pub fn new(v: impl ObjectLike) -> Self {
		Self(Cc::new(tb!(v)))
	}
	pub fn new_empty() -> Self {
		Self::new(EmptyObject)
	}
	pub fn builder() -> ObjValueBuilder {
		ObjValueBuilder::new()
	}
	pub fn builder_with_capacity(capacity: usize) -> ObjValueBuilder {
		ObjValueBuilder::with_capacity(capacity)
	}
	pub(crate) fn extend_with_raw_member(self, key: IStr, value: ObjMember) -> Self {
		let mut out = ObjValueBuilder::with_capacity(1);
		out.with_super(self);
		let mut member = out.field(key);
		if value.flags.add() {
			member = member.add();
		}
		if let Some(loc) = value.location {
			member = member.with_location(loc);
		}
		let _ = member
			.with_visibility(value.flags.visibility())
			.binding(value.invoke);
		out.build()
	}
	pub fn extend_field(&mut self, name: IStr) -> ObjMemberBuilder<ExtendBuilder<'_>> {
		ObjMemberBuilder::new(ExtendBuilder(self), name, FieldIndex::default())
	}

	#[must_use]
	pub fn extend_from(&self, sup: Self) -> Self {
		self.0.extend_from(sup)
	}
	#[must_use]
	pub fn with_this(&self, this: Self) -> Self {
		self.0.with_this(self.clone(), this)
	}
	pub fn len(&self) -> usize {
		self.0.len()
	}
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
	pub fn enum_fields(&self, depth: SuperDepth, handler: &mut EnumFieldsHandler<'_>) -> bool {
		self.0.enum_fields(depth, handler)
	}

	pub fn has_field_include_hidden(&self, name: IStr) -> bool {
		self.0.has_field_include_hidden(name)
	}
	pub fn has_field(&self, name: IStr) -> bool {
		self.0.has_field(name)
	}
	pub fn has_field_ex(&self, name: IStr, include_hidden: bool) -> bool {
		if include_hidden {
			self.has_field_include_hidden(name)
		} else {
			self.has_field(name)
		}
	}

	pub fn get(&self, key: IStr) -> Result<Option<Val>> {
		self.run_assertions()?;
		self.get_for(key, self.0.this().unwrap_or_else(|| self.clone()))
	}

	pub fn get_for(&self, key: IStr, this: Self) -> Result<Option<Val>> {
		self.0.get_for(key, this)
	}

	pub fn get_or_bail(&self, key: IStr) -> Result<Val> {
		let Some(value) = self.get(key.clone())? else {
			let suggestions = suggest_object_fields(self, key.clone());
			bail!(NoSuchField(key, suggestions))
		};
		Ok(value)
	}

	fn get_raw(&self, key: IStr, this: Self) -> Result<Option<Val>> {
		self.0.get_for_uncached(key, this)
	}

	fn field_visibility(&self, field: IStr) -> Option<Visibility> {
		self.0.field_visibility(field)
	}

	pub fn run_assertions(&self) -> Result<()> {
		// FIXME: Should it use `self.0.this()` in case of standalone super?
		self.run_assertions_raw(self.clone())
	}
	fn run_assertions_raw(&self, this: Self) -> Result<()> {
		self.0.run_assertions_raw(this)
	}

	pub fn iter(
		&self,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> impl Iterator<Item = (IStr, Result<Val>)> + '_ {
		let fields = self.fields(
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		);
		fields.into_iter().map(|field| {
			(
				field.clone(),
				self.get(field)
					.map(|opt| opt.expect("iterating over keys, field exists")),
			)
		})
	}
	pub fn get_lazy(&self, key: IStr) -> Option<Thunk<Val>> {
		#[derive(Trace)]
		struct ThunkGet {
			obj: ObjValue,
			key: IStr,
		}
		impl ThunkValue for ThunkGet {
			type Output = Val;

			fn get(self: Box<Self>) -> Result<Self::Output> {
				Ok(self.obj.get(self.key)?.expect("field exists"))
			}
		}

		if !self.has_field_ex(key.clone(), true) {
			return None;
		}
		Some(Thunk::new(ThunkGet {
			obj: self.clone(),
			key,
		}))
	}
	pub fn get_lazy_or_bail(&self, key: IStr) -> Thunk<Val> {
		#[derive(Trace)]
		struct ThunkGet {
			obj: ObjValue,
			key: IStr,
		}
		impl ThunkValue for ThunkGet {
			type Output = Val;

			fn get(self: Box<Self>) -> Result<Self::Output> {
				self.obj.get_or_bail(self.key)
			}
		}

		Thunk::new(ThunkGet {
			obj: self.clone(),
			key,
		})
	}
	pub fn ptr_eq(a: &Self, b: &Self) -> bool {
		Cc::ptr_eq(&a.0, &b.0)
	}
	pub fn downgrade(self) -> WeakObjValue {
		WeakObjValue(self.0.downgrade())
	}
	fn fields_visibility(&self) -> FxHashMap<IStr, (bool, FieldSortKey)> {
		let mut out = FxHashMap::default();
		self.enum_fields(
			SuperDepth::default(),
			&mut |depth, index, name, visibility| {
				let new_sort_key = FieldSortKey::new(depth, index);
				let entry = out.entry(name);
				let (visible, _) = entry.or_insert((true, new_sort_key));
				match visibility {
					Visibility::Normal => {}
					Visibility::Hidden => {
						*visible = false;
					}
					Visibility::Unhide => {
						*visible = true;
					}
				};
				false
			},
		);
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
					j = k;
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
	pub fn values_ex(
		&self,
		include_hidden: bool,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> ArrValue {
		ArrValue::new(PickObjectValues::new(
			self.clone(),
			self.fields_ex(
				include_hidden,
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			),
		))
	}
	pub fn values(&self, #[cfg(feature = "exp-preserve-order")] preserve_order: bool) -> ArrValue {
		self.values_ex(
			false,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		)
	}
	pub fn key_values_ex(
		&self,
		include_hidden: bool,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> ArrValue {
		ArrValue::new(PickObjectKeyValues::new(
			self.clone(),
			self.fields_ex(
				include_hidden,
				#[cfg(feature = "exp-preserve-order")]
				preserve_order,
			),
		))
	}
	pub fn key_values(
		&self,
		#[cfg(feature = "exp-preserve-order")] preserve_order: bool,
	) -> ArrValue {
		self.key_values_ex(
			false,
			#[cfg(feature = "exp-preserve-order")]
			preserve_order,
		)
	}
}

impl OopObject {
	pub fn new(
		sup: Option<ObjValue>,
		this_entries: Cc<GcHashMap<IStr, ObjMember>>,
		assertions: Cc<Vec<TraceBox<dyn ObjectAssertion>>>,
	) -> Self {
		Self {
			sup,
			// this: None,
			assertions,
			assertions_ran: RefCell::new(GcHashSet::new()),
			this_entries,
			value_cache: RefCell::new(GcHashMap::new()),
		}
	}

	fn evaluate_this(&self, v: &ObjMember, real_this: ObjValue) -> Result<Val> {
		v.invoke.evaluate(self.sup.clone(), Some(real_this))
	}

	// FIXME: Duplication between ObjValue and OopObject
	fn fields_visibility(&self) -> FxHashMap<IStr, (bool, FieldSortKey)> {
		let mut out = FxHashMap::default();
		self.enum_fields(
			SuperDepth::default(),
			&mut |depth, index, name, visibility| {
				let new_sort_key = FieldSortKey::new(depth, index);
				let entry = out.entry(name);
				let (visible, _) = entry.or_insert((true, new_sort_key));
				match visibility {
					Visibility::Normal => {}
					Visibility::Hidden => {
						*visible = false;
					}
					Visibility::Unhide => {
						*visible = true;
					}
				};
				false
			},
		);
		out
	}
}

impl ObjectLike for OopObject {
	fn extend_from(&self, sup: ObjValue) -> ObjValue {
		ObjValue::new(match &self.sup {
			None => Self::new(
				Some(sup),
				self.this_entries.clone(),
				self.assertions.clone(),
			),
			Some(v) => Self::new(
				Some(v.extend_from(sup)),
				self.this_entries.clone(),
				self.assertions.clone(),
			),
		})
	}

	fn len(&self) -> usize {
		// Maybe it will be better to not compute sort key here?
		self.fields_visibility()
			.into_iter()
			.filter(|(_, (visible, _))| *visible)
			.count()
	}

	/// Returns false only if there is any visible entry.
	///
	/// Note that object with hidden fields `{a:: 1}` will be reported as empty here.
	fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Run callback for every field found in object
	///
	/// Returns true if ended prematurely
	fn enum_fields(&self, depth: SuperDepth, handler: &mut EnumFieldsHandler<'_>) -> bool {
		if let Some(s) = &self.sup {
			if s.enum_fields(depth.deeper(), handler) {
				return true;
			}
		}
		for (name, member) in self.this_entries.iter() {
			if handler(
				depth,
				member.original_index,
				name.clone(),
				member.flags.visibility(),
			) {
				return true;
			}
		}
		false
	}

	fn has_field_include_hidden(&self, name: IStr) -> bool {
		if self.this_entries.contains_key(&name) {
			true
		} else if let Some(super_obj) = &self.sup {
			super_obj.has_field_include_hidden(name)
		} else {
			false
		}
	}
	fn has_field(&self, name: IStr) -> bool {
		self.field_visibility(name)
			.map_or(false, |v| v.is_visible())
	}

	fn get_for(&self, key: IStr, this: ObjValue) -> Result<Option<Val>> {
		let cache_key = (key.clone(), Some(this.clone().downgrade()));
		if let Some(v) = self.value_cache.borrow().get(&cache_key) {
			return Ok(match v {
				CacheValue::Cached(v) => Some(v.clone()),
				CacheValue::NotFound => None,
				CacheValue::Pending => bail!(InfiniteRecursionDetected),
				CacheValue::Errored(e) => return Err(e.clone()),
			});
		}
		self.value_cache
			.borrow_mut()
			.insert(cache_key.clone(), CacheValue::Pending);
		let value = self.get_for_uncached(key, this).map_err(|e| {
			self.value_cache
				.borrow_mut()
				.insert(cache_key.clone(), CacheValue::Errored(e.clone()));
			e
		})?;
		self.value_cache.borrow_mut().insert(
			cache_key,
			value
				.as_ref()
				.map_or(CacheValue::NotFound, |v| CacheValue::Cached(v.clone())),
		);
		Ok(value)
	}
	fn get_for_uncached(&self, key: IStr, real_this: ObjValue) -> Result<Option<Val>> {
		match (self.this_entries.get(&key), &self.sup) {
			(Some(k), None) => Ok(Some(self.evaluate_this(k, real_this)?)),
			(Some(k), Some(super_obj)) => {
				let our = self.evaluate_this(k, real_this.clone())?;
				if k.flags.add() {
					super_obj
						.get_raw(key, real_this)?
						.map_or(Ok(Some(our.clone())), |v| {
							Ok(Some(evaluate_add_op(&v, &our)?))
						})
				} else {
					Ok(Some(our))
				}
			}
			(None, Some(super_obj)) => super_obj.get_raw(key, real_this),
			(None, None) => Ok(None),
		}
	}
	fn field_visibility(&self, name: IStr) -> Option<Visibility> {
		if let Some(m) = self.this_entries.get(&name) {
			Some(match &m.flags.visibility() {
				Visibility::Normal => self
					.sup
					.as_ref()
					.and_then(|super_obj| super_obj.field_visibility(name))
					.unwrap_or(Visibility::Normal),
				v => *v,
			})
		} else if let Some(super_obj) = &self.sup {
			super_obj.field_visibility(name)
		} else {
			None
		}
	}

	fn run_assertions_raw(&self, real_this: ObjValue) -> Result<()> {
		if self.assertions.is_empty() {
			if let Some(super_obj) = &self.sup {
				super_obj.run_assertions_raw(real_this)?;
			}
			return Ok(());
		}
		if self.assertions_ran.borrow_mut().insert(real_this.clone()) {
			for assertion in self.assertions.iter() {
				if let Err(e) = assertion.run(self.sup.clone(), Some(real_this.clone())) {
					self.assertions_ran.borrow_mut().remove(&real_this);
					return Err(e);
				}
			}
			if let Some(super_obj) = &self.sup {
				super_obj.run_assertions_raw(real_this)?;
			}
		}
		Ok(())
	}
}

impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		Cc::ptr_eq(&self.0, &other.0)
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
	sup: Option<ObjValue>,
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
			sup: None,
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
		self.sup = Some(super_obj);
		self
	}

	pub fn assert(&mut self, assertion: impl ObjectAssertion + 'static) -> &mut Self {
		self.assertions.push(tb!(assertion));
		self
	}
	pub fn field(&mut self, name: impl Into<IStr>) -> ObjMemberBuilder<ValueBuilder<'_>> {
		let field_index = self.next_field_index;
		self.next_field_index = self.next_field_index.next();
		ObjMemberBuilder::new(ValueBuilder(self), name.into(), field_index)
	}
	/// Preset for common method definiton pattern:
	/// Create a hidden field with the function value.
	///
	/// `.field(name).hide().value(Val::function(value))`
	pub fn method(&mut self, name: impl Into<IStr>, value: impl Into<FuncVal>) -> &mut Self {
		self.field(name).hide().value(Val::Func(value.into()));
		self
	}
	pub fn try_method(
		&mut self,
		name: impl Into<IStr>,
		value: impl Into<FuncVal>,
	) -> Result<&mut Self> {
		self.field(name).hide().try_value(Val::Func(value.into()))?;
		Ok(self)
	}

	pub fn build(self) -> ObjValue {
		if self.sup.is_none() && self.map.is_empty() && self.assertions.is_empty() {
			return ObjValue::new_empty();
		}
		ObjValue::new(OopObject::new(
			self.sup,
			Cc::new(self.map),
			Cc::new(self.assertions),
		))
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
	location: Option<Span>,
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
	pub fn with_location(mut self, location: Span) -> Self {
		self.location = Some(location);
		self
	}
	fn build_member(self, binding: MaybeUnbound) -> (Kind, IStr, ObjMember) {
		(
			self.kind,
			self.name,
			ObjMember {
				flags: ObjFieldFlags::new(self.add, self.visibility),
				original_index: self.original_index,
				invoke: binding,
				location: self.location,
			},
		)
	}
}

pub struct ValueBuilder<'v>(&'v mut ObjValueBuilder);
impl ObjMemberBuilder<ValueBuilder<'_>> {
	/// Inserts value, replacing if it is already defined
	pub fn value(self, value: impl Into<Val>) {
		let (receiver, name, member) =
			self.build_member(MaybeUnbound::Bound(Thunk::evaluated(value.into())));
		let entry = receiver.0.map.entry(name);
		entry.insert(member);
	}

	/// Tries to insert value, returns an error if it was already defined
	pub fn try_value(self, value: impl Into<Val>) -> Result<()> {
		self.thunk(Thunk::evaluated(value.into()))
	}
	pub fn thunk(self, value: impl Into<Thunk<Val>>) -> Result<()> {
		self.binding(MaybeUnbound::Bound(value.into()))
	}
	pub fn bindable(self, bindable: impl Unbound<Bound = Val>) -> Result<()> {
		self.binding(MaybeUnbound::Unbound(Cc::new(tb!(bindable))))
	}
	pub fn binding(self, binding: MaybeUnbound) -> Result<()> {
		let (receiver, name, member) = self.build_member(binding);
		let location = member.location.clone();
		let old = receiver.0.map.insert(name.clone(), member);
		if old.is_some() {
			State::push(
				CallLocation(location.as_ref()),
				|| format!("field <{}> initializtion", name.clone()),
				|| bail!(DuplicateFieldName(name.clone())),
			)?;
		}
		Ok(())
	}
}

pub struct ExtendBuilder<'v>(&'v mut ObjValue);
impl ObjMemberBuilder<ExtendBuilder<'_>> {
	pub fn value(self, value: impl Into<Val>) {
		self.binding(MaybeUnbound::Bound(Thunk::evaluated(value.into())));
	}
	pub fn bindable(self, bindable: TraceBox<dyn Unbound<Bound = Val>>) {
		self.binding(MaybeUnbound::Unbound(Cc::new(bindable)));
	}
	pub fn binding(self, binding: MaybeUnbound) {
		let (receiver, name, member) = self.build_member(binding);
		let new = receiver.0.clone();
		*receiver.0 = new.extend_with_raw_member(name, member);
	}
}
