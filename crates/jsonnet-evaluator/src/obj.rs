use crate::{dummy_debug, evaluate_binary_op, BoxedBinding, Val};
use jsonnet_parser::{BinaryOpType, Visibility};
use std::{
	collections::{BTreeMap, BTreeSet},
	rc::Rc,
};

#[derive(Debug)]
pub struct ObjMember {
	pub add: bool,
	pub visibility: Visibility,
	pub invoke: BoxedBinding,
}

#[derive(Debug)]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	this_entries: Rc<BTreeMap<String, ObjMember>>,
}
pub struct ObjValue(Rc<ObjValueInternals>);
dummy_debug!(ObjValue);
impl ObjValue {
	pub fn new(
		super_obj: Option<ObjValue>,
		this_entries: Rc<BTreeMap<String, ObjMember>>,
	) -> ObjValue {
		ObjValue(Rc::new(ObjValueInternals {
			super_obj,
			this_entries,
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
	pub fn get_raw(&self, key: &str, real_this: Option<ObjValue>) -> Option<Val> {
		match (self.0.this_entries.get(key), &self.0.super_obj) {
			(Some(k), None) => Some(k.invoke.evaluate(
				real_this.or_else(|| Some(self.clone())),
				self.0.super_obj.clone().map(|e| e.clone()),
			)),
			(Some(k), Some(s)) => {
				let our = k
					.invoke
					.evaluate(Some(self.clone()), self.0.super_obj.clone());
				if k.add {
					s.get_raw(key, Some(self.clone()))
						.map_or(Some(our.clone()), |v| {
							Some(evaluate_binary_op(&v, BinaryOpType::Add, &our))
						})
				} else {
					Some(our)
				}
			}
			(None, Some(s)) => s.get_raw(key, Some(self.clone())),
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
