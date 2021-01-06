use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{
	cell::RefCell,
	fmt::{self, Display},
	hash::{BuildHasherDefault, Hash, Hasher},
	ops::Deref,
	rc::Rc,
};

#[derive(Clone, PartialOrd, Ord, Eq)]
pub struct IStr(Rc<str>);

impl Deref for IStr {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl PartialEq for IStr {
	fn eq(&self, other: &Self) -> bool {
		// It is ok, since all IStr should be inlined into same pool
		Rc::ptr_eq(&self.0, &other.0)
	}
}

impl PartialEq<str> for IStr {
	fn eq(&self, other: &str) -> bool {
		&self.0 as &str == other
	}
}

impl Hash for IStr {
	fn hash<H: Hasher>(&self, state: &mut H) {
		state.write_usize(Rc::as_ptr(&self.0) as *const () as usize)
	}
}

impl Drop for IStr {
	fn drop(&mut self) {
		// First reference - current object, second - POOL
		if Rc::strong_count(&self.0) <= 2 {
			STR_POOL.with(|pool| pool.borrow_mut().remove(&self.0));
		}
	}
}

impl fmt::Debug for IStr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", &self.0)
	}
}

impl Display for IStr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.0)
	}
}

thread_local! {
	static STR_POOL: RefCell<FxHashMap<Rc<str>, ()>> = RefCell::new(FxHashMap::with_capacity_and_hasher(200, BuildHasherDefault::default()));
}

impl From<&str> for IStr {
	fn from(str: &str) -> Self {
		IStr(STR_POOL.with(|pool| {
			let mut pool = pool.borrow_mut();
			if let Some((k, _)) = pool.get_key_value(str) {
				return k.clone();
			} else {
				let rc: Rc<str> = str.into();
				pool.insert(rc.clone(), ());
				rc
			}
		}))
	}
}

impl From<String> for IStr {
	fn from(str: String) -> Self {
		(&str as &str).into()
	}
}

impl Serialize for IStr {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		(&self.0 as &str).serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for IStr {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let s = <&str>::deserialize(deserializer)?;
		Ok(s.into())
	}
}
