use crate::{evaluate_add_op, Binding, Val};
use jsonnet_parser::Visibility;
use std::{
	cell::RefCell,
	collections::{BTreeMap, BTreeSet, HashMap},
	fmt::Debug,
	rc::Rc,
};

#[derive(Debug)]
pub struct ObjMember {
	pub add: bool,
	pub visibility: Visibility,
	pub invoke: Binding,
}

#[derive(Debug)]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	this_entries: Rc<BTreeMap<String, ObjMember>>,
	value_cache: RefCell<HashMap<String, Val>>,
}
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
		debug.field("$ptr", &Rc::as_ptr(&self.0));
		for (name, member) in self.0.this_entries.iter() {
			debug.field(name, member);
		}
		debug.finish_non_exhaustive()
		// .field("fields", &self.fields())
		// .finish_non_exhaustive()
	}
}

impl ObjValue {
	pub fn new(
		super_obj: Option<ObjValue>,
		this_entries: Rc<BTreeMap<String, ObjMember>>,
	) -> ObjValue {
		ObjValue(Rc::new(ObjValueInternals {
			super_obj,
			this_entries,
			value_cache: RefCell::new(HashMap::new()),
		}))
	}
	pub fn with_super(&self, super_obj: ObjValue) -> ObjValue {
		match &self.0.super_obj {
			None => ObjValue::new(Some(super_obj), self.0.this_entries.clone()),
			Some(v) => ObjValue::new(Some(v.with_super(super_obj)), self.0.this_entries.clone()),
		}
	}
	pub fn fields(&self) -> BTreeSet<String> {
		let mut fields = BTreeSet::new();
		self.0.this_entries.keys().for_each(|k| {
			fields.insert(k.clone());
		});
		if self.0.super_obj.is_some() {
			for field in self.0.super_obj.clone().unwrap().fields() {
				fields.insert(field);
			}
		}
		fields
	}
	pub fn get(&self, key: &str) -> Option<Val> {
		if let Some(v) = self.0.value_cache.borrow().get(key) {
			return Some(v.clone());
		}
		let v = self.get_raw(key, self).map(|v| v.unwrap_if_lazy());
		if let Some(v) = v.clone() {
			self.0.value_cache.borrow_mut().insert(key.to_owned(), v);
		}
		v
	}
	fn get_raw(&self, key: &str, real_this: &ObjValue) -> Option<Val> {
		match (self.0.this_entries.get(key), &self.0.super_obj) {
			(Some(k), None) => Some(k.invoke.0(
				Some(real_this.clone()),
				self.0.super_obj.clone(),
			)),
			(Some(k), Some(s)) => {
				let our = k.invoke.0(Some(real_this.clone()), self.0.super_obj.clone());
				if k.add {
					s.get_raw(key, real_this)
						.map_or(Some(our.clone()), |v| Some(evaluate_add_op(&v, &our)))
				} else {
					Some(our)
				}
			}
			(None, Some(s)) => s.get_raw(key, real_this),
			(None, None) => None,
		}
	}
}
impl Clone for ObjValue {
	fn clone(&self) -> Self {
		ObjValue(self.0.clone())
	}
}
impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.0, &other.0)
	}
}
