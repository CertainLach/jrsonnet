use std::{
	any::Any, cell::{Cell, RefCell}, collections::hash_map::Entry, fmt::{self, Debug}, hash::{Hash, Hasher}
};

use jrsonnet_gcmodule::{cc_dyn, Cc, Trace, Weak};
use educe::Educe;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{Span, Visibility};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
	arr::{PickObjectKeyValues, PickObjectValues},
	bail,
	error::{suggest_object_fields, ErrorKind::*},
	function::{CallLocation, FuncVal},
	gc::WithCapacityExt as _,
	in_frame,
	identity_hash, 
	operator::evaluate_add_op,
	val::ArrValue,
	CcUnbound, MaybeUnbound, Result, Thunk, Unbound, Val,
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
		pub(super) fn deepen(self) {}
	}

	#[derive(Clone, Copy, Debug)]
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
		pub(super) fn deepen(&mut self) {
			*self.0 += 1
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
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

cc_dyn!(CcObjectAssertion, ObjectAssertion);
pub trait ObjectAssertion: Trace {
	fn run(&self, sup_this: SupThis) -> Result<()>;
}

// Field => This

#[derive(Trace, Debug)]
enum CacheValue {
	Cached(Result<Option<Val>>),
	Pending,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Trace)]
#[trace(tracking(force))]
pub struct OopObject {
	// this: Option<ObjValue>,
	assertions: Cc<Vec<CcObjectAssertion>>,
	this_entries: Cc<FxHashMap<IStr, ObjMember>>,
	value_cache: RefCell<FxHashMap<(IStr, Option<WeakObjValue>), CacheValue>>,
}
impl Debug for OopObject {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("OopObject")
			// .field("assertions", &self.assertions)
			// .field("assertions_ran", &self.assertions_ran)
			.field("this_entries", &self.this_entries)
			// .field("value_cache", &self.value_cache)
			.finish_non_exhaustive()
	}
}

type EnumFieldsHandler<'a> = dyn FnMut(SuperDepth, FieldIndex, IStr, Visibility) -> bool + 'a;

#[derive(Trace, Clone)]
pub enum ValueProcess {
	None,
	SuperPlus,
}

pub trait ObjectCore: Trace + Any + Debug {
	// If callback returns false, iteration stops, and this call returns false.
	fn enum_fields_core(
		&self,
		super_depth: &mut SuperDepth,
		handler: &mut EnumFieldsHandler<'_>,
	) -> bool;

	fn has_field_include_hidden(&self, name: IStr) -> bool;

	fn get_for(&self, key: IStr, sup_this: SupThis) -> Result<Option<(Val, ValueProcess)>>;
	// fn get_for_uncached(&self, key: IStr, this: ObjValue) -> Result<Option<(Val, ValueProcess)>>;
	fn field_visibility(&self, field: IStr) -> Option<Visibility>;

	fn run_assertions_raw(&self, sup_this: SupThis) -> Result<()>;
}

#[derive(Clone, Trace)]
pub struct WeakObjValue(#[trace(skip)] Weak<ObjValueInner>);
impl Debug for WeakObjValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("WeakObjValue").finish()
	}
}

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

cc_dyn!(
	#[derive(Clone, Debug)]
	ObjCore, ObjectCore,
	pub fn new() {...}
);
#[derive(Trace, Educe)]
#[educe(Debug)]
struct ObjValueInner {
	cores: Vec<ObjCore>,
	assertions_ran: Cell<bool>,
	value_cache: RefCell<FxHashMap<(IStr, CoreIdx), CacheValue>>,
}

thread_local! {
	static RUNNING_ASSERTIONS: RefCell<FxHashSet<ObjValue>> = RefCell::default();
}
fn is_asserting(obj: &ObjValue) -> bool {
	RUNNING_ASSERTIONS.with_borrow(|v| v.contains(obj))
}
/// Returns false if already asserting
fn start_asserting(obj: &ObjValue) -> bool {
	RUNNING_ASSERTIONS.with_borrow_mut(|v| v.insert(obj.clone()))
}
fn finish_asserting(obj: &ObjValue) {
	RUNNING_ASSERTIONS.with_borrow_mut(|v| {
		let r = v.remove(obj);
		debug_assert!(
			r,
			"finish_asserting was called before start_asserting or twice"
		);
	});
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Trace, Debug, Educe)]
#[educe(PartialEq, Hash, Eq)]
pub struct ObjValue(
	#[educe(PartialEq(method(Cc::ptr_eq)), Hash(method(identity_hash)))] Cc<ObjValueInner>,
);

#[derive(Trace, Debug)]
struct StandaloneSuperCore {
	sup: CoreIdx,
	this: ObjValue,
}
impl ObjectCore for StandaloneSuperCore {
	fn enum_fields_core(
		&self,
		super_depth: &mut SuperDepth,
		handler: &mut EnumFieldsHandler<'_>,
	) -> bool {
		self.this
			.enum_fields_internal(super_depth, handler, self.sup)
	}

	fn has_field_include_hidden(&self, name: IStr) -> bool {
		self.this.has_field_include_hidden_idx(name, self.sup)
	}

	fn get_for(&self, key: IStr, _sup_this: SupThis) -> Result<Option<(Val, ValueProcess)>> {
		let v = self.this.get_idx(key, self.sup)?;
		Ok(v.map(|v| (v, ValueProcess::None)))
	}

	fn field_visibility(&self, field: IStr) -> Option<Visibility> {
		self.this.field_visibility_idx(field, self.sup)
	}

	fn run_assertions_raw(&self, _sup_this: SupThis) -> Result<()> {
		self.this.run_assertions()
	}
}

#[derive(Debug, Trace)]
struct EmptyObject;
impl ObjectCore for EmptyObject {
	fn enum_fields_core(
		&self,
		_super_depth: &mut SuperDepth,
		_handler: &mut EnumFieldsHandler<'_>,
	) -> bool {
		true
	}

	fn has_field_include_hidden(&self, _name: IStr) -> bool {
		false
	}

	fn get_for(&self, _key: IStr, _sup_this: SupThis) -> Result<Option<(Val, ValueProcess)>> {
		Ok(None)
	}

	fn run_assertions_raw(&self, _sup_this: SupThis) -> Result<()> {
		Ok(())
	}

	fn field_visibility(&self, _field: IStr) -> Option<Visibility> {
		None
	}
}

#[derive(Hash, PartialEq, Eq, Trace, Clone, Copy, Debug)]
struct CoreIdx {
	idx: usize,
}
impl CoreIdx {
	fn super_exists(self) -> bool {
		self.idx != 0
	}
}
#[derive(Trace, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SupThis {
	sup: CoreIdx,
	this: ObjValue,
}
impl SupThis {
	pub fn has_super(&self) -> bool {
		self.sup.super_exists()
	}
	/// Implementation of `"field" in super` operation,
	/// works faster than standalone super path.
	///
	/// In case of no `super` existence, returns false.
	pub fn field_in_super(&self, field: IStr) -> bool {
		self.this.has_field_include_hidden_idx(field, self.sup)
	}
	/// Implementation of `super.field` operation,
	/// works faster than standalone super path.
	///
	/// In case of no `super` existence, returns `NoSuperFound`
	pub fn get_super(&self, field: IStr) -> Result<Option<Val>> {
		if !self.sup.super_exists() {
			bail!(NoSuperFound);
		}
		self.this.get_idx(field, self.sup)
	}
	/// `super` with `self` overriden for top-level lookups.
	/// Exists when super appears outside of `super.field`/`"field" in super` expressions
	/// Exclusive to jrsonnet.
	///
	/// Might return `NoSuperFound` error.
	pub fn standalone_super(&self) -> Result<ObjValue> {
		if !self.sup.super_exists() {
			bail!(NoSuperFound)
		}
		Ok(ObjValue::new(StandaloneSuperCore {
			sup: self.sup,
			this: self.this.clone(),
		}))
	}
	pub fn this(&self) -> &ObjValue {
		&self.this
	}
	pub fn downgrade(self) -> WeakSupThis {
		WeakSupThis {
			sup: self.sup,
			this: self.this.downgrade(),
		}
	}
}
#[derive(Trace, PartialEq, Eq, Hash, Debug)]
pub struct WeakSupThis {
	sup: CoreIdx,
	this: WeakObjValue,
}

impl ObjValue {
	pub fn new(v: impl ObjectCore) -> Self {
		Self(Cc::new(ObjValueInner {
			cores: vec![ObjCore::new(v)],
			assertions_ran: Cell::new(false),
			value_cache: RefCell::new(FxHashMap::new()),
		}))
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
		let mut cores = sup.0.cores.clone();
		cores.extend(self.0.cores.iter().cloned());
		ObjValue(Cc::new(ObjValueInner {
			cores,
			value_cache: RefCell::default(),
			assertions_ran: Cell::new(false),
		}))
	}
	// #[must_use]
	// pub fn with_this(&self, this: Self) -> Self {
	// 	self.0.with_this(self.clone(), this)
	// }
	/// Returns amount of visible object fields
	/// If object only contains hidden fields - may return zero.
	pub fn len(&self) -> usize {
		self.fields_visibility()
			.iter()
			.filter(|(_, (visible, _))| *visible)
			.count()
	}
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
	/// For each field, calls callback.
	/// If callback returns false - ends iteration prematurely.
	///
	/// Returns false if ended prematurely
	pub fn enum_fields(&self, handler: &mut EnumFieldsHandler<'_>) -> bool {
		let mut super_depth = SuperDepth::default();
		self.enum_fields_internal(
			&mut super_depth,
			handler,
			CoreIdx {
				idx: self.0.cores.len(),
			},
		)
	}
	fn enum_fields_internal(
		&self,
		super_depth: &mut SuperDepth,
		handler: &mut EnumFieldsHandler<'_>,
		idx: CoreIdx,
	) -> bool {
		for core in self.0.cores[..idx.idx].iter() {
			if !core.0.enum_fields_core(super_depth, handler) {
				return false;
			}
			super_depth.deepen();
		}
		true
	}

	pub fn has_field_include_hidden(&self, name: IStr) -> bool {
		self.has_field_include_hidden_idx(
			name,
			CoreIdx {
				idx: self.0.cores.len(),
			},
		)
	}
	fn has_field_include_hidden_idx(&self, name: IStr, core: CoreIdx) -> bool {
		self.0.cores[..core.idx]
			.iter()
			.rev()
			.any(|v| v.0.has_field_include_hidden(name.clone()))
	}
	pub fn has_field(&self, name: IStr) -> bool {
		match self.field_visibility(name) {
			Some(Visibility::Unhide | Visibility::Normal) => true,
			Some(Visibility::Hidden) | None => false,
		}
	}
	pub fn has_field_ex(&self, name: IStr, include_hidden: bool) -> bool {
		if include_hidden {
			self.has_field_include_hidden(name)
		} else {
			self.has_field(name)
		}
	}
	pub fn get(&self, key: IStr) -> Result<Option<Val>> {
		self.get_idx(
			key,
			CoreIdx {
				idx: self.0.cores.len(),
			},
		)
	}

	fn get_idx(&self, key: IStr, core: CoreIdx) -> Result<Option<Val>> {
		let cache_key = (key.clone(), core);
		{
			let mut cache = self.0.value_cache.borrow_mut();
			// entry_ref candidate?
			match cache.entry(cache_key.clone()) {
				Entry::Occupied(v) => match v.get() {
					CacheValue::Cached(v) => return v.clone(),
					CacheValue::Pending => {
						if !is_asserting(self) {
							bail!(InfiniteRecursionDetected);
						}
					}
				},
				Entry::Vacant(v) => {
					v.insert(CacheValue::Pending);
				}
			};
		}
		let result = self.get_idx_uncached(key, core);
		{
			let mut cache = self.0.value_cache.borrow_mut();
			cache.insert(cache_key, CacheValue::Cached(result.clone()));
		}
		result
	}
	fn get_idx_uncached(&self, key: IStr, core: CoreIdx) -> Result<Option<Val>> {
		self.run_assertions()?;
		let mut add_stack = Vec::with_capacity(2);
		for (sup, core) in self.0.cores[..core.idx].iter().enumerate().rev() {
			let sup_this = SupThis {
				sup: CoreIdx { idx: sup },
				this: self.clone(),
			};
			if let Some((val, proc)) = core.0.get_for(key.clone(), sup_this)? {
				match proc {
					ValueProcess::None if add_stack.is_empty() => return Ok(Some(val)),
					ValueProcess::None => {
						add_stack.push(val);
						break;
					}
					ValueProcess::SuperPlus => {
						add_stack.push(val);
					}
				}
			}
		}
		if add_stack.is_empty() {
			// None of layers had this field
			return Ok(None);
		} else if add_stack.len() == 1 {
			// A layer had this field, but it wanted this field to be added with super.
			// However, no super had this field, fail-safe
			return Ok(Some(add_stack.pop().expect("single element on stack")));
		}
		let mut values = add_stack.into_iter().rev();
		let init = values.next().expect("at least 2 elements");

		values
			.try_fold(init, |a, b| evaluate_add_op(&a, &b))
			.map(Some)

		// self.0.get_raw(key, this)
	}

	pub fn get_or_bail(&self, key: IStr) -> Result<Val> {
		let Some(value) = self.get(key.clone())? else {
			let suggestions = suggest_object_fields(self, key.clone());
			bail!(NoSuchField(key, suggestions))
		};
		Ok(value)
	}

	fn field_visibility(&self, field: IStr) -> Option<Visibility> {
		self.field_visibility_idx(
			field,
			CoreIdx {
				idx: self.0.cores.len(),
			},
		)
	}
	fn field_visibility_idx(&self, field: IStr, core: CoreIdx) -> Option<Visibility> {
		let mut exists = false;
		for ele in self.0.cores[..core.idx].iter().rev() {
			let vis = ele.0.field_visibility(field.clone());
			match vis {
				Some(Visibility::Unhide | Visibility::Hidden) => return vis,
				Some(Visibility::Normal) => exists = true,
				None => {}
			}
		}
		exists.then_some(Visibility::Normal)
	}

	pub fn run_assertions(&self) -> Result<()> {
		if self.0.assertions_ran.get() {
			return Ok(());
		}
		if !start_asserting(self) {
			return Ok(());
		}
		for (idx, ele) in self.0.cores.iter().enumerate() {
			let sup_this = SupThis {
				sup: CoreIdx { idx },
				this: self.clone(),
			};
			ele.0.run_assertions_raw(sup_this).inspect_err(|_e| {
				finish_asserting(self);
			})?;
		}
		finish_asserting(self);
		self.0.assertions_ran.set(true);
		Ok(())
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
		if !self.has_field_ex(key.clone(), true) {
			return None;
		}
		let obj = self.clone();

		Some(Thunk!(move || Ok(obj.get(key)?.expect("field exists"))))
	}
	pub fn get_lazy_or_bail(&self, key: IStr) -> Thunk<Val> {
		let obj = self.clone();
		Thunk!(move || obj.get_or_bail(key))
	}
	pub fn ptr_eq(a: &Self, b: &Self) -> bool {
		Cc::ptr_eq(&a.0, &b.0)
	}
	pub fn downgrade(self) -> WeakObjValue {
		WeakObjValue(self.0.downgrade())
	}
	fn fields_visibility(&self) -> FxHashMap<IStr, (bool, FieldSortKey)> {
		let mut out = FxHashMap::default();
		self.enum_fields(&mut |depth, index, name, visibility| {
			dbg!(&name, visibility);
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
					j = k;
				}
				fields[j] = x;
			}
			return fields;
		}

		let mut fields: Vec<_> = dbg!(self
			.fields_visibility())
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
		this_entries: Cc<FxHashMap<IStr, ObjMember>>,
		assertions: Cc<Vec<CcObjectAssertion>>,
	) -> Self {
		Self {
			this_entries,
			value_cache: RefCell::new(FxHashMap::new()),
			assertions,
		}
	}
}

impl ObjectCore for OopObject {
	fn enum_fields_core(
		&self,
		super_depth: &mut SuperDepth,
		handler: &mut EnumFieldsHandler<'_>,
	) -> bool {
		for (name, member) in self.this_entries.iter() {
			if handler(
				*super_depth,
				member.original_index,
				name.clone(),
				member.flags.visibility(),
			) {
				return false;
			}
		}
		true
	}

	fn has_field_include_hidden(&self, name: IStr) -> bool {
		self.this_entries.contains_key(&name)
	}

	fn get_for(&self, key: IStr, sup_this: SupThis) -> Result<Option<(Val, ValueProcess)>> {
		match self.this_entries.get(&key) {
			Some(k) => Ok(Some((
				k.invoke.evaluate(sup_this)?,
				if k.flags.add() {
					ValueProcess::SuperPlus
				} else {
					ValueProcess::None
				},
			))),
			None => Ok(None),
		}
	}
	fn field_visibility(&self, name: IStr) -> Option<Visibility> {
		Some(self.this_entries.get(&name)?.flags.visibility())
	}

	fn run_assertions_raw(&self, sup_this: SupThis) -> Result<()> {
		if self.assertions.is_empty() {
			return Ok(());
		}
		for assertion in self.assertions.iter() {
			assertion.0.run(sup_this.clone())?;
		}
		Ok(())
	}
}

#[allow(clippy::module_name_repetitions)]
pub struct ObjValueBuilder {
	sup: Option<ObjValue>,
	map: FxHashMap<IStr, ObjMember>,
	assertions: Vec<CcObjectAssertion>,
	next_field_index: FieldIndex,
}
impl ObjValueBuilder {
	pub fn new() -> Self {
		Self::with_capacity(0)
	}
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			sup: None,
			map: FxHashMap::with_capacity(capacity),
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
		self.assertions.push(CcObjectAssertion::new(assertion));
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
		let res = ObjValue::new(OopObject::new(Cc::new(self.map), Cc::new(self.assertions)));
		self.sup.map(|sup| res.extend_from(sup)).unwrap_or(res)
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
		entry.insert_entry(member);
	}

	/// Tries to insert value, returns an error if it was already defined
	pub fn try_value(self, value: impl Into<Val>) -> Result<()> {
		self.try_thunk(Thunk::evaluated(value.into()))
	}
	pub fn try_thunk(self, value: impl Into<Thunk<Val>>) -> Result<()> {
		self.binding(MaybeUnbound::Bound(value.into()))
	}
	pub fn bindable(self, bindable: impl Unbound<Bound = Val>) -> Result<()> {
		self.binding(MaybeUnbound::Unbound(CcUnbound::new(bindable)))
	}
	pub fn binding(self, binding: MaybeUnbound) -> Result<()> {
		let (receiver, name, member) = self.build_member(binding);
		let location = member.location.clone();
		let old = receiver.0.map.insert(name.clone(), member);
		if old.is_some() {
			in_frame(
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
	pub fn bindable(self, bindable: impl Unbound<Bound = Val>) {
		self.binding(MaybeUnbound::Unbound(CcUnbound::new(bindable)));
	}
	pub fn binding(self, binding: MaybeUnbound) {
		let (receiver, name, member) = self.build_member(binding);
		let new = receiver.0.clone();
		*receiver.0 = new.extend_with_raw_member(name, member);
	}
}
