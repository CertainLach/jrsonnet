use crate::{evaluate_add_op, LazyBinding, Result, Val};
use gc::{Finalize, Gc, Trace};
use indexmap::IndexMap;
use jrsonnet_parser::{ExprLocation, GcStr, Visibility};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};

#[derive(Debug, Trace, Finalize)]
pub struct ObjMember {
	pub add: bool,
	#[unsafe_ignore_trace]
	pub visibility: Visibility,
	pub invoke: LazyBinding,
	#[unsafe_ignore_trace]
	pub location: Option<ExprLocation>,
}

#[derive(Debug, Trace, Finalize)]
pub struct ObjValueInternals {
	super_obj: Option<ObjValue>,
	this_entries: Gc<HashMap<GcStr, ObjMember>>,
}
#[derive(Clone, Trace, Finalize)]
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
	pub fn new(super_obj: Option<Self>, this_entries: Gc<HashMap<GcStr, ObjMember>>) -> Self {
		Self(Gc::new(ObjValueInternals {
			super_obj,
			this_entries,
		}))
	}
	pub fn new_empty() -> Self {
		Self::new(None, Gc::new(HashMap::new()))
	}
	pub fn with_super(&self, super_obj: Self) -> Self {
		match &self.0.super_obj {
			None => Self::new(Some(super_obj), self.0.this_entries.clone()),
			Some(v) => Self::new(Some(v.with_super(super_obj)), self.0.this_entries.clone()),
		}
	}
	pub fn enum_fields(&self, handler: &impl Fn(&GcStr, &Visibility)) {
		if let Some(s) = &self.0.super_obj {
			s.enum_fields(handler);
		}
		for (name, member) in self.0.this_entries.iter() {
			handler(name, &member.visibility);
		}
	}
	pub fn fields_visibility(&self) -> IndexMap<GcStr, bool> {
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
	pub fn visible_fields(&self) -> Vec<GcStr> {
		let mut visible_fields: Vec<_> = self
			.fields_visibility()
			.into_iter()
			.filter(|(_k, v)| *v)
			.map(|(k, _)| k)
			.collect();
		visible_fields.sort();
		visible_fields
	}
	pub fn get(&self, key: GcStr) -> Result<Option<Val>> {
		Ok(self.get_raw(key, self)?)
	}
	// TODO: Return value cache
	pub(crate) fn get_raw(&self, key: GcStr, real_this: &Self) -> Result<Option<Val>> {
		let value = match (self.0.this_entries.get(&key), &self.0.super_obj) {
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
		}?;
		Ok(value)
	}
	fn evaluate_this(&self, v: &ObjMember, real_this: &Self) -> Result<Val> {
		Ok(v.invoke
			.evaluate(Some(real_this.clone()), self.0.super_obj.clone())?
			.evaluate()?)
	}
}
impl PartialEq for ObjValue {
	fn eq(&self, other: &Self) -> bool {
		Gc::ptr_eq(&self.0, &other.0)
	}
}
