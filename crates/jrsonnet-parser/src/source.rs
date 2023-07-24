use std::{
	any::Any,
	fmt::{self, Debug, Display},
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	rc::Rc,
};

use jrsonnet_gcmodule::{Trace, Tracer};
use jrsonnet_interner::IStr;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "structdump")]
use structdump::Codegen;

use crate::location::{location_to_offset, offset_to_location, CodeLocation};

macro_rules! any_ext_methods {
	($T:ident) => {
		fn as_any(&self) -> &dyn Any;
		fn dyn_hash(&self, hasher: &mut dyn Hasher);
		fn dyn_eq(&self, other: &dyn $T) -> bool;
		fn dyn_debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
	};
}
macro_rules! any_ext_impl {
	($T:ident) => {
		fn as_any(&self) -> &dyn Any {
			self
		}
		fn dyn_hash(&self, mut hasher: &mut dyn Hasher) {
			self.hash(&mut hasher)
		}
		fn dyn_eq(&self, other: &dyn $T) -> bool {
			let Some(other) = other.as_any().downcast_ref::<Self>() else { return false };
			let this = <Self as $T>::as_any(self)
				.downcast_ref::<Self>()
				.expect("restricted by impl");
			this == other
		}
		fn dyn_debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			<Self as std::fmt::Debug>::fmt(self, fmt)
		}
	};
}
macro_rules! any_ext {
	($T:ident) => {
		impl Hash for dyn $T {
			fn hash<H: Hasher>(&self, state: &mut H) {
				self.dyn_hash(state)
			}
		}
		impl PartialEq for dyn $T {
			fn eq(&self, other: &Self) -> bool {
				self.dyn_eq(other)
			}
		}
		impl Eq for dyn $T {}
	};
}
pub trait SourcePathT: Trace + Debug + Display {
	/// This method should be checked by resolver before panicking with bad SourcePath input
	/// if `true` - then resolver may threat this path as default, and default is usally a CWD
	fn is_default(&self) -> bool;
	fn path(&self) -> Option<&Path>;
	any_ext_methods!(SourcePathT);
}
any_ext!(SourcePathT);

/// Represents location of a file
///
/// Standard CLI only operates using
/// - [`SourceFile`] - for any file
/// - [`SourceDirectory`] - for resolution from CWD
/// - [`SourceVirtual`] - for stdlib/ext-str
///
/// From all of those, only [`SourceVirtual`] may be constructed manually, any other path kind should be only obtained
/// from assigned `ImportResolver`
/// However, you should always check `is_default` method return, as it will return true for any paths, where default
/// search location is applicable
///
/// Resolver may also return custom implementations of this trait, for example it may return http url in case of remotely loaded files
#[derive(Eq, Debug, Clone)]
pub struct SourcePath(Rc<dyn SourcePathT>);
impl SourcePath {
	pub fn new(inner: impl SourcePathT) -> Self {
		Self(Rc::new(inner))
	}
	pub fn downcast_ref<T: SourcePathT>(&self) -> Option<&T> {
		self.0.as_any().downcast_ref()
	}
	pub fn is_default(&self) -> bool {
		self.0.is_default()
	}
	pub fn path(&self) -> Option<&Path> {
		self.0.path()
	}
}
impl Hash for SourcePath {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.hash(state);
	}
}
impl PartialEq for SourcePath {
	#[allow(clippy::op_ref)]
	fn eq(&self, other: &Self) -> bool {
		&*self.0 == &*other.0
	}
}
impl Trace for SourcePath {
	fn trace(&self, tracer: &mut Tracer) {
		(*self.0).trace(tracer)
	}

	fn is_type_tracked() -> bool
	where
		Self: Sized,
	{
		true
	}
}
impl Display for SourcePath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}
impl Default for SourcePath {
	fn default() -> Self {
		Self(Rc::new(SourceDefault))
	}
}

#[cfg(feature = "structdump")]
impl Codegen for SourcePath {
	fn gen_code(
		&self,
		res: &mut structdump::CodegenResult,
		unique: bool,
	) -> structdump::TokenStream {
		let source_virtual = self
			.0
			.as_any()
			.downcast_ref::<SourceVirtual>()
			.expect("can only codegen for virtual source paths!")
			.0
			.clone();
		let val = res.add_value(source_virtual, false);
		res.add_code(
			structdump::quote! {
				structdump_import::SourcePath::new(structdump_import::SourceVirtual(#val))
			},
			Some(structdump::quote!(SourcePath)),
			unique,
		)
	}
}

#[derive(Trace, Hash, PartialEq, Eq, Debug)]
struct SourceDefault;
impl Display for SourceDefault {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "<default>")
	}
}
impl SourcePathT for SourceDefault {
	fn is_default(&self) -> bool {
		true
	}
	fn path(&self) -> Option<&Path> {
		None
	}
	any_ext_impl!(SourcePathT);
}

/// Represents path to the file on the disk
/// Directories shouldn't be put here, as resolution for files differs from resolution for directories:
///
/// When `file` is being resolved from `SourceFile(a/b/c)`, it should be resolved to `SourceFile(a/b/file)`,
/// however if it is being resolved from `SourceDirectory(a/b/c)`, then it should be resolved to `SourceDirectory(a/b/c/file)`
#[derive(Trace, Hash, PartialEq, Eq, Debug)]
pub struct SourceFile(PathBuf);
impl SourceFile {
	pub fn new(path: PathBuf) -> Self {
		Self(path)
	}
	pub fn path(&self) -> &Path {
		&self.0
	}
}
impl Display for SourceFile {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0.display())
	}
}
impl SourcePathT for SourceFile {
	fn is_default(&self) -> bool {
		false
	}
	fn path(&self) -> Option<&Path> {
		Some(&self.0)
	}
	any_ext_impl!(SourcePathT);
}

/// Represents path to the directory on the disk
///
/// See also [`SourceFile`]
#[derive(Trace, Hash, PartialEq, Eq, Debug)]
pub struct SourceDirectory(PathBuf);
impl SourceDirectory {
	pub fn new(path: PathBuf) -> Self {
		Self(path)
	}
	pub fn path(&self) -> &Path {
		&self.0
	}
}
impl Display for SourceDirectory {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0.display())
	}
}
impl SourcePathT for SourceDirectory {
	fn is_default(&self) -> bool {
		false
	}
	fn path(&self) -> Option<&Path> {
		Some(&self.0)
	}
	any_ext_impl!(SourcePathT);
}

/// Represents virtual file, whose are located in memory, and shouldn't be cached
///
/// It is used for --ext-code=.../--tla-code=.../standard library source code by default,
/// and user can construct arbitrary values by hand, without asking import resolver
#[cfg_attr(feature = "structdump", derive(Codegen))]
#[derive(Trace, Hash, PartialEq, Eq, Debug, Clone)]
pub struct SourceVirtual(pub IStr);
impl Display for SourceVirtual {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}
impl SourcePathT for SourceVirtual {
	fn is_default(&self) -> bool {
		true
	}
	fn path(&self) -> Option<&Path> {
		None
	}
	any_ext_impl!(SourcePathT);
}

/// Either real file, or virtual
/// Hash of FileName always have same value as raw Path, to make it possible to use with raw_entry_mut
#[cfg_attr(feature = "structdump", derive(Codegen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Source(pub Rc<(SourcePath, IStr)>);

impl Trace for Source {
	fn trace(&self, _tracer: &mut Tracer) {}

	fn is_type_tracked() -> bool {
		false
	}
}

impl Source {
	pub fn new(path: SourcePath, code: IStr) -> Self {
		Self(Rc::new((path, code)))
	}

	pub fn new_virtual(name: IStr, code: IStr) -> Self {
		Self::new(SourcePath::new(SourceVirtual(name)), code)
	}

	pub fn code(&self) -> &str {
		&self.0 .1
	}

	pub fn source_path(&self) -> &SourcePath {
		&self.0 .0
	}

	pub fn map_source_locations<const S: usize>(&self, locs: &[u32; S]) -> [CodeLocation; S] {
		offset_to_location(&self.0 .1, locs)
	}
	pub fn map_from_source_location(&self, line: usize, column: usize) -> Option<usize> {
		location_to_offset(&self.0 .1, line, column)
	}
}
