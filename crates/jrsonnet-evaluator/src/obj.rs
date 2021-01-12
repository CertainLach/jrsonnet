use crate::{evaluate_add_op, LazyBinding, Result, Val};
use indexmap::IndexMap;
use jrsonnet_interner::IStr;
use jrsonnet_parser::{ExprLocation, Visibility};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

#[derive(Debug)]
pub struct ObjMember {
	pub add: bool,
	pub visibility: Visibility,
	pub invoke: LazyBinding,
	pub location: Option<ExprLocation>,
}

// Field => This
type CacheKey = (IStr, usize);
#[derive(Debug)]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	this_entries: Rc<HashMap<IStr, ObjMember>>,
	value_cache: RefCell<HashMap<CacheKey, Option<Val>>>,
}
#[derive(Clone)]
pub struct ObjValue(pub(crate) Rc<ObjValueInternals>);
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
			debug.field(&name, member);
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
	pub fn new(super_obj: Option<Self>, this_entries: Rc<HashMap<IStr, ObjMember>>) -> Self {
		Self(Rc::new(ObjValueInternals {
			super_obj,
			this_entries,
			value_cache: RefCell::new(HashMap::new()),
		}))
	}
	pub fn new_empty() -> Self {
		Self::new(None, Rc::new(HashMap::new()))
	}
	pub fn with_super(&self, super_obj: Self) -> Self {
		match &self.0.super_obj {
			None => Self::new(Some(super_obj), self.0.this_entries.clone()),
			Some(v) => Self::new(Some(v.with_super(super_obj)), self.0.this_entries.clone()),
		}
	}
	pub fn enum_fields(&self, handler: &impl Fn(&IStr, &Visibility)) {
		if let Some(s) = &self.0.super_obj {
			s.enum_fields(handler);
		}
		for (name, member) in self.0.this_entries.iter() {
			handler(name, &member.visibility);
		}
	}
	pub fn fields_visibility(&self) -> IndexMap<IStr, bool> {
		let out = Rc::new(RefCell::new(IndexMap::new()));
		self.enum_fields(&|name, visibility| {
			let mut out = out.borrow_mut();
			match visibility {
				Visibility::Normal => {
					if !out.contains_key(name) {
						out.insert(name.to_owned(), true);
					}
				}
				Visibility::Hidden => {
					out.insert(name.to_owned(), false);
				}
				Visibility::Unhide => {
					out.insert(name.to_owned(), true);
				}
			};
		});
		Rc::try_unwrap(out).unwrap().into_inner()
	}
	pub fn visible_fields(&self) -> Vec<IStr> {
		let mut visible_fields: Vec<_> = self
			.fields_visibility()
			.into_iter()
			.filter(|(_k, v)| *v)
			.map(|(k, _)| k)
			.collect();
		visible_fields.sort();
		visible_fields
	}
	pub fn get(&self, key: IStr) -> Result<Option<Val>> {
		Ok(self.get_raw(key, None)?)
	}
	pub(crate) fn get_raw(&self, key: IStr, real_this: Option<&Self>) -> Result<Option<Val>> {
		let real_this = real_this.unwrap_or(self);
		let cache_key = (key.clone(), Rc::as_ptr(&real_this.0) as usize);

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
		Ok(v.invoke
			.evaluate(Some(real_this.clone()), self.0.super_obj.clone())?
			.evaluate()?)
	}

	pub fn ptr_eq(a: &ObjValue, b: &ObjValue) -> bool {
		Rc::ptr_eq(&a.0, &b.0)
	}
}
impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.0, &other.0)
	}
}
