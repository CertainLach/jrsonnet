use std::{
	borrow::Cow,
	fmt,
	path::{Component, Path, PathBuf},
	rc::Rc,
};

use jrsonnet_gcmodule::{Trace, Tracer};
use jrsonnet_interner::IStr;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "structdump")]
use structdump::Codegen;

use crate::location::{location_to_offset, offset_to_location, CodeLocation};

#[cfg_attr(feature = "structdump", derive(Codegen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub enum SourcePath {
	/// This file is located on disk
	Path(PathBuf),
	/// This file is located somewhere else (I.e http), but it can refer to relative paths, and is egilible for caching
	Custom(String),
	/// This file is only located in memory, and can't be cached
	Virtual(Cow<'static, str>),
}
impl Trace for SourcePath {
	fn trace(&self, _tracer: &mut Tracer) {}

	fn is_type_tracked() -> bool {
		false
	}
}

impl SourcePath {
	/// Should import resolver be able to read file by this path?
	pub fn can_load(&self) -> bool {
		matches!(self, Self::Path(_) | Self::Custom(_))
	}
}

/// Either real file, or virtual
/// Hash of FileName always have same value as raw Path, to make it possible to use with raw_entry_mut
#[cfg_attr(feature = "structdump", derive(Codegen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Source(pub Rc<(SourcePath, IStr)>);
static_assertions::assert_eq_size!(Source, *const ());

impl Trace for Source {
	fn trace(&self, _tracer: &mut Tracer) {}

	fn is_type_tracked() -> bool {
		false
	}
}

impl Source {
	/// Fails when path contains inner /../ or /./ references, or not absolute
	pub fn new(path: SourcePath, code: IStr) -> Option<Self> {
		if let SourcePath::Path(path) = &path {
			if !path.is_absolute()
				|| path
					.components()
					.any(|c| matches!(c, Component::CurDir | Component::ParentDir))
			{
				return None;
			}
		}
		Some(Self(Rc::new((path, code))))
	}

	pub fn new_virtual(n: Cow<'static, str>, code: IStr) -> Self {
		Self(Rc::new((SourcePath::Virtual(n), code)))
	}

	pub fn short_display(&self) -> ShortDisplay {
		ShortDisplay(self.clone())
	}

	/// Returns Some if this file is loaded from FS
	pub fn path(&self) -> Option<&Path> {
		match self.source_path() {
			SourcePath::Path(r) => Some(r),
			SourcePath::Custom(_) => None,
			SourcePath::Virtual(_) => None,
		}
	}
	pub fn code(&self) -> &str {
		&self.0 .1
	}

	pub fn source_path(&self) -> &SourcePath {
		&self.0 .0 as &SourcePath
	}

	pub fn map_source_locations(&self, locs: &[u32]) -> Vec<CodeLocation> {
		offset_to_location(&self.0 .1, locs)
	}
	pub fn map_from_source_location(&self, line: usize, column: usize) -> Option<usize> {
		location_to_offset(&self.0 .1, line, column)
	}
}
pub struct ShortDisplay(Source);
impl fmt::Display for ShortDisplay {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.0 .0 .0 as &SourcePath {
			SourcePath::Path(r) => {
				write!(
					f,
					"{}",
					r.file_name().expect("path is valid").to_string_lossy()
				)
			}
			SourcePath::Custom(r) => write!(f, "{}", r),
			SourcePath::Virtual(n) => write!(f, "{}", n),
		}
	}
}
