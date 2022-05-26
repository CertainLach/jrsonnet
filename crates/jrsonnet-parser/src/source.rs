use std::{
	borrow::Cow,
	fmt,
	path::{Component, Path, PathBuf},
	rc::Rc,
};

use gcmodule::{Trace, Tracer};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq, Eq, Debug, Hash)]
enum Inner {
	Real(PathBuf),
	Virtual(Cow<'static, str>),
}

/// Either real file, or virtual
/// Hash of FileName always have same value as raw Path, to make it possible to use with raw_entry_mut
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Source(Rc<Inner>);
static_assertions::assert_eq_size!(Source, *const ());

impl Trace for Source {
	fn trace(&self, _tracer: &mut Tracer) {}

	fn is_type_tracked() -> bool {
		false
	}
}

impl Source {
	/// Fails when path contains inner /../ or /./ references, or not absolute
	pub fn new(path: PathBuf) -> Option<Self> {
		if !path.is_absolute()
			|| path
				.components()
				.any(|c| matches!(c, Component::CurDir | Component::ParentDir))
		{
			return None;
		}
		Some(Self(Rc::new(Inner::Real(path))))
	}

	pub fn new_virtual(n: Cow<'static, str>) -> Self {
		Self(Rc::new(Inner::Virtual(n)))
	}

	pub fn short_display(&self) -> ShortDisplay {
		ShortDisplay(self.clone())
	}
	pub fn full_path(&self) -> String {
		match self.inner() {
			Inner::Real(r) => r.display().to_string(),
			Inner::Virtual(v) => v.to_string(),
		}
	}

	/// Returns None if file is virtual
	pub fn path(&self) -> Option<&Path> {
		match self.inner() {
			Inner::Real(r) => Some(r),
			Inner::Virtual(_) => None,
		}
	}
	pub fn repr(&self) -> Result<&Path, &str> {
		match self.inner() {
			Inner::Real(r) => Ok(r),
			Inner::Virtual(v) => Err(v.as_ref()),
		}
	}

	fn inner(&self) -> &Inner {
		&self.0 as &Inner
	}
}
pub struct ShortDisplay(Source);
impl fmt::Display for ShortDisplay {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.0 .0 as &Inner {
			Inner::Real(r) => {
				write!(
					f,
					"{}",
					r.file_name().expect("path is valid").to_string_lossy()
				)
			}
			Inner::Virtual(n) => write!(f, "{}", n),
		}
	}
}
