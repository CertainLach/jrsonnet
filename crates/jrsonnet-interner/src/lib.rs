#![deny(
	unsafe_op_in_unsafe_fn,
	clippy::missing_safety_doc,
	clippy::undocumented_unsafe_blocks
)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]
use std::{
	borrow::Cow,
	cell::RefCell,
	fmt::{self, Display},
	hash::{BuildHasherDefault, Hash, Hasher},
	ops::Deref,
	str,
};

use hashbrown::HashMap;
use jrsonnet_gcmodule::Trace;
use rustc_hash::FxHasher;

mod inner;
use inner::Inner;

/// Interned string
///
/// Provides O(1) comparsions and hashing, cheap copy, and cheap conversion to [`IBytes`]
#[derive(Clone, PartialOrd, Ord, Eq)]
pub struct IStr(Inner);
impl Trace for IStr {
	fn is_type_tracked() -> bool {
		false
	}
}

impl IStr {
	#[must_use]
	pub fn empty() -> Self {
		"".into()
	}
	#[must_use]
	pub fn as_str(&self) -> &str {
		self as &str
	}

	#[must_use]
	pub fn cast_bytes(self) -> IBytes {
		IBytes(self.0.clone())
	}
}

impl Deref for IStr {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		// SAFETY: Inner::check_utf8 is called on IStr construction, data is utf-8
		unsafe { self.0.as_str_unchecked() }
	}
}

impl PartialEq for IStr {
	fn eq(&self, other: &Self) -> bool {
		// all IStr should be inlined into same pool
		Inner::ptr_eq(&self.0, &other.0)
	}
}

impl PartialEq<str> for IStr {
	fn eq(&self, other: &str) -> bool {
		self as &str == other
	}
}

impl Hash for IStr {
	fn hash<H: Hasher>(&self, state: &mut H) {
		// IStr is always obtained from pool, where no string have duplicate, thus every unique string has unique address
		state.write_usize(Inner::as_ptr(&self.0).cast::<()>() as usize);
	}
}

impl Drop for IStr {
	fn drop(&mut self) {
		maybe_unpool(&self.0)
	}
}

impl fmt::Debug for IStr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self as &str, f)
	}
}

impl Display for IStr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(self as &str, f)
	}
}

/// Interned byte array
#[derive(Clone, PartialOrd, Ord, Eq)]
pub struct IBytes(Inner);
impl Trace for IBytes {
	fn is_type_tracked() -> bool {
		false
	}
}

impl IBytes {
	#[must_use]
	pub fn cast_str(self) -> Option<IStr> {
		if Inner::check_utf8(&self.0) {
			Some(IStr(self.0.clone()))
		} else {
			None
		}
	}
	/// # Safety
	/// data should be valid utf8
	unsafe fn cast_str_unchecked(self) -> IStr {
		// SAFETY: data is utf8
		unsafe { Inner::assume_utf8(&self.0) };
		IStr(self.0.clone())
	}

	#[must_use]
	pub fn as_slice(&self) -> &[u8] {
		self.0.as_slice()
	}
}

impl Deref for IBytes {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		self.0.as_slice()
	}
}

impl PartialEq for IBytes {
	fn eq(&self, other: &Self) -> bool {
		// all IStr should be inlined into same pool
		Inner::ptr_eq(&self.0, &other.0)
	}
}

impl Hash for IBytes {
	fn hash<H: Hasher>(&self, state: &mut H) {
		// IBytes is always obtained from pool, where no string have duplicate, thus every unique string has unique address
		state.write_usize(Inner::as_ptr(&self.0).cast::<()>() as usize);
	}
}

impl Drop for IBytes {
	fn drop(&mut self) {
		maybe_unpool(&self.0)
	}
}

fn maybe_unpool(inner: &Inner) {
	#[cold]
	#[inline(never)]
	fn unpool(inner: &Inner) {
		// May fail on program termination
		let _ = POOL.try_with(|pool| {
			let mut pool = pool.borrow_mut();

			if pool.remove(inner).is_none() {
				// On some platforms (i.e i686-windows), try_with will not fail after TLS
				// destructor is called, but instead re-initialize the TLS with the empty pool.
				// Allow non-pooled Drop in this case.
				// https://github.com/CertainLach/jrsonnet/issues/98#issuecomment-1591624016
				//
				// However, if pool is not empty, most likely this is issue #113, and then I don't
				// have any explainations for now.
				assert!(pool.is_empty(), "received an unpooled string not during the program termination, please write any info regarding this crash to https://github.com/CertainLach/jrsonnet/issues/113, thanks!");
			}
		});
	}
	// First reference - current object, second - POOL
	if Inner::strong_count(&inner) <= 2 {
		unpool(&inner);
	}
}

impl fmt::Debug for IBytes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self as &[u8], f)
	}
}

impl<'c> From<Cow<'c, str>> for IStr {
	fn from(v: Cow<'c, str>) -> Self {
		intern_str(&v)
	}
}
impl From<&str> for IStr {
	fn from(v: &str) -> Self {
		intern_str(v)
	}
}
impl From<String> for IStr {
	fn from(s: String) -> Self {
		s.as_str().into()
	}
}
impl From<&String> for IStr {
	fn from(s: &String) -> Self {
		s.as_str().into()
	}
}
impl From<char> for IStr {
	fn from(value: char) -> Self {
		let mut buf = [0; 5];
		Self::from(&*value.encode_utf8(&mut buf))
	}
}
impl From<&[u8]> for IBytes {
	fn from(v: &[u8]) -> Self {
		intern_bytes(v)
	}
}

#[cfg(feature = "serde")]
impl serde::Serialize for IStr {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.as_str().serialize(serializer)
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for IStr {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let str = <&str>::deserialize(deserializer)?;
		Ok(intern_str(str))
	}
}

#[cfg(feature = "structdump")]
impl structdump::Codegen for IStr {
	fn gen_code(
		&self,
		res: &mut structdump::CodegenResult,
		_unique: bool,
	) -> structdump::TokenStream {
		let s: &str = self;
		res.add_code(
			structdump::quote! {
				structdump_import::IStr::from(#s)
			},
			Some(structdump::quote![structdump_import::IStr]),
			false,
		)
	}
}

thread_local! {
	static POOL: RefCell<HashMap<Inner, (), BuildHasherDefault<FxHasher>>> = RefCell::new(HashMap::with_capacity_and_hasher(200, BuildHasherDefault::default()));
}

#[must_use]
pub fn intern_bytes(bytes: &[u8]) -> IBytes {
	POOL.with(|pool| {
		let mut pool = pool.borrow_mut();
		let entry = pool.raw_entry_mut().from_key(bytes);
		match entry {
			hashbrown::hash_map::RawEntryMut::Occupied(i) => IBytes(i.get_key_value().0.clone()),
			hashbrown::hash_map::RawEntryMut::Vacant(e) => {
				let (k, _) = e.insert(Inner::new_bytes(bytes), ());
				IBytes(k.clone())
			}
		}
	})
}

#[must_use]
pub fn intern_str(str: &str) -> IStr {
	// SAFETY: Rust strings always utf8
	unsafe { intern_bytes(str.as_bytes()).cast_str_unchecked() }
}

#[cfg(test)]
mod tests {
	use crate::IStr;

	#[test]
	fn simple() {
		let a = IStr::from("a");
		let b = IStr::from("a");

		assert_eq!(a.as_ptr(), b.as_ptr());
	}
}
