use crate::{evaluate_add_op, LazyBinding, Result, Val};
use indexmap::IndexMap;
use jrsonnet_parser::Visibility;
use std::{
	cell::RefCell,
	collections::{BTreeMap, HashMap},
	fmt::Debug,
	rc::Rc,
};

#[derive(Debug)]
pub struct ObjMember {
	pub add: bool,
	pub visibility: Visibility,
	pub invoke: LazyBinding,
}

#[derive(Debug)]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	this_entries: Rc<BTreeMap<Rc<str>, ObjMember>>,
	value_cache: RefCell<HashMap<Rc<str>, Val>>,
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
			debug.field(name, member);
		}
		debug.finish_non_exhaustive()
	}
}

impl ObjValue {
	pub fn new(
		super_obj: Option<ObjValue>,
		this_entries: Rc<BTreeMap<Rc<str>, ObjMember>>,
	) -> ObjValue {
		ObjValue(Rc::new(ObjValueInternals {
			super_obj,
			this_entries,
			value_cache: RefCell::new(HashMap::new()),
		}))
	}
	pub fn new_empty() -> ObjValue {
		Self::new(None, Rc::new(BTreeMap::new()))
	}
	pub fn with_super(&self, super_obj: ObjValue) -> ObjValue {
		match &self.0.super_obj {
			None => ObjValue::new(Some(super_obj), self.0.this_entries.clone()),
			Some(v) => ObjValue::new(Some(v.with_super(super_obj)), self.0.this_entries.clone()),
		}
	}
	pub fn enum_fields(&self, handler: &impl Fn(&Rc<str>, &Visibility)) {
		if let Some(s) = &self.0.super_obj {
			s.enum_fields(handler);
		}
		for (name, member) in self.0.this_entries.iter() {
			handler(&name, &member.visibility);
		}
	}
	pub fn fields_visibility(&self) -> IndexMap<Rc<str>, bool> {
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
	pub fn visible_fields(&self) -> Vec<Rc<str>> {
		self.fields_visibility()
			.into_iter()
			.filter(|(_k, v)| *v)
			.map(|(k, _)| k)
			.collect()
	}
	pub fn get(&self, key: Rc<str>) -> Result<Option<Val>> {
		if let Some(v) = self.0.value_cache.borrow().get(&key) {
			return Ok(Some(v.clone()));
		}
		if let Some(v) = self.get_raw(&key, self)? {
			let v = v.unwrap_if_lazy()?;
			self.0.value_cache.borrow_mut().insert(key, v.clone());
			Ok(Some(v))
		} else {
			Ok(None)
		}
	}
	pub(crate) fn get_raw(&self, key: &str, real_this: &ObjValue) -> Result<Option<Val>> {
		match (self.0.this_entries.get(key), &self.0.super_obj) {
			(Some(k), None) => Ok(Some(self.evaluate_this(k, real_this)?)),
			(Some(k), Some(s)) => {
				let our = self.evaluate_this(k, real_this)?;
				if k.add {
					s.get_raw(key, real_this)?
						.map_or(Ok(Some(our.clone())), |v| {
							Ok(Some(evaluate_add_op(&v, &our)?))
						})
				} else {
					Ok(Some(our))
				}
			}
			(None, Some(s)) => s.get_raw(key, real_this),
			(None, None) => Ok(None),
		}
	}
	fn evaluate_this(&self, v: &ObjMember, real_this: &ObjValue) -> Result<Val> {
		Ok(v.invoke
			.evaluate(Some(real_this.clone()), self.0.super_obj.clone())?
			.evaluate()?)
	}
}
impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.0, &other.0)
	}
}
