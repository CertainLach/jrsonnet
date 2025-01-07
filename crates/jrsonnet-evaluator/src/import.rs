use std::{
	any::Any,
	borrow::Cow,
	env::current_dir,
	fmt, fs,
	io::{ErrorKind, Read},
	path::{Path, PathBuf},
};

use fs::File;
use jrsonnet_gcmodule::{cc_dyn, Trace};
use jrsonnet_interner::IBytes;
use jrsonnet_parser::{
	IStr, SourceDefaultIgnoreJpath, SourceDirectory, SourceFifo, SourceFile, SourcePath,
};

use crate::{
	bail,
	error::{ErrorKind::*, Result},
};
#[derive(Clone, Debug, Trace)]
pub enum ResolvePathOwned {
	Str(String),
	Path(PathBuf),
}
impl fmt::Display for ResolvePathOwned {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Str(s) => write!(f, "{s}"),
			Self::Path(p) => write!(f, "{}", p.display()),
		}
	}
}
#[derive(Clone, Copy)]
pub enum ResolvePath<'s> {
	Str(&'s str),
	Path(&'s Path),
}
impl ResolvePath<'_> {
	fn to_owned(self) -> ResolvePathOwned {
		match self {
			ResolvePath::Str(s) => ResolvePathOwned::Str(s.to_owned()),
			ResolvePath::Path(p) => ResolvePathOwned::Path(p.to_owned()),
		}
	}
}
impl AsRef<Path> for ResolvePath<'_> {
	fn as_ref(&self) -> &Path {
		match self {
			ResolvePath::Str(s) => s.as_ref(),
			ResolvePath::Path(p) => p,
		}
	}
}
pub trait AsPathLike {
	fn as_path(&self) -> ResolvePath<'_>;
}
impl<T> AsPathLike for &T
where
	T: AsPathLike + ?Sized,
{
	fn as_path(&self) -> ResolvePath<'_> {
		(*self).as_path()
	}
}
impl AsPathLike for str {
	fn as_path(&self) -> ResolvePath<'_> {
		ResolvePath::Str(self)
	}
}
impl AsPathLike for IStr {
	fn as_path(&self) -> ResolvePath<'_> {
		ResolvePath::Str(self)
	}
}
impl AsPathLike for Path {
	fn as_path(&self) -> ResolvePath<'_> {
		ResolvePath::Path(self)
	}
}
impl AsPathLike for Cow<'_, Path> {
	fn as_path(&self) -> ResolvePath<'_> {
		ResolvePath::Path(self.as_ref())
	}
}

cc_dyn!(CcImportResolver, ImportResolver);
/// Implements file resolution logic for `import` and `importStr`
pub trait ImportResolver: Trace {
	/// Resolves file path, e.g. `(/home/user/manifests, b.libjsonnet)` can correspond
	/// both to `/home/user/manifests/b.libjsonnet` and to `/home/user/${vendor}/b.libjsonnet`
	/// where `${vendor}` is a library path.
	///
	/// `from` should only be returned from [`ImportResolver::resolve`], or from other defined file, any other value
	/// may result in panic
	fn resolve_from(&self, from: &SourcePath, path: &dyn AsPathLike) -> Result<SourcePath> {
		bail!(ImportNotSupported(from.clone(), path.as_path().to_owned()))
	}
	fn resolve_from_default(&self, path: &dyn AsPathLike) -> Result<SourcePath> {
		self.resolve_from(&SourcePath::default(), path)
	}

	/// Load resolved file
	/// This should only be called with value returned from [`ImportResolver::resolve_file`]/[`ImportResolver::resolve`],
	/// this cannot be resolved using associated type, as evaluator uses object instead of generic for [`ImportResolver`]
	fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>>;

	// For downcasts, will be removed after trait_upcasting_coercion
	// stabilization.
	fn as_any(&self) -> &dyn Any;
	fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Dummy resolver, can't resolve/load any file
#[derive(Trace)]
pub struct DummyImportResolver;
impl ImportResolver for DummyImportResolver {
	fn load_file_contents(&self, _resolved: &SourcePath) -> Result<Vec<u8>> {
		panic!("dummy resolver can't load any file")
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}
#[allow(clippy::use_self)]
impl Default for Box<dyn ImportResolver> {
	fn default() -> Self {
		Box::new(DummyImportResolver)
	}
}

/// File resolver, can load file from both FS and library paths
#[derive(Default, Trace)]
pub struct FileImportResolver {
	/// Library directories to search for file.
	/// Referred to as `jpath` in original jsonnet implementation.
	library_paths: Vec<PathBuf>,
}
impl FileImportResolver {
	pub fn new(library_paths: Vec<PathBuf>) -> Self {
		Self { library_paths }
	}
	/// Dynamically add new jpath, used by bindings
	pub fn add_jpath(&mut self, path: PathBuf) {
		self.library_paths.push(path);
	}
}

/// Create `SourcePath` from path, handling directories/Fifo files (on unix)/etc
fn check_path(path: &Path) -> Result<Option<SourcePath>> {
	let meta = match fs::metadata(path) {
		Ok(v) => v,
		Err(e) if e.kind() == ErrorKind::NotFound => {
			return Ok(None);
		}
		Err(e) => bail!(ImportIo(e.to_string())),
	};
	let ty = meta.file_type();
	if ty.is_file() {
		return Ok(Some(SourcePath::new(SourceFile::new(
			path.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
		))));
	}
	let ty = meta.file_type();
	#[cfg(unix)]
	{
		use std::os::unix::fs::FileTypeExt;
		if ty.is_fifo() {
			let file = fs::read(path).map_err(|e| ImportIo(format!("FIFO read failed: {e}")))?;
			return Ok(Some(SourcePath::new(SourceFifo(
				format!("{}", path.display()),
				IBytes::from(file.as_slice()),
			))));
		}
	}
	// Block device/some other magic thing.
	Err(RuntimeError("special file can't be imported".into()).into())
}

impl ImportResolver for FileImportResolver {
	fn resolve_from(&self, from: &SourcePath, path: &dyn AsPathLike) -> Result<SourcePath> {
		let path = path.as_path();
		let mut direct = if let Some(f) = from.downcast_ref::<SourceFile>() {
			let mut o = f.path().to_owned();
			o.pop();
			o
		} else if let Some(d) = from.downcast_ref::<SourceDirectory>() {
			d.path().to_owned()
		} else if from.downcast_ref::<SourceDefaultIgnoreJpath>().is_some() {
			let mut direct = current_dir().map_err(|e| ImportIo(e.to_string()))?;
			direct.push(path);
			if let Some(direct) = check_path(&direct)? {
				return Ok(direct);
			}
			bail!(ImportFileNotFound(from.clone(), path.to_owned()))
		} else if from.is_default() {
			current_dir().map_err(|e| ImportIo(e.to_string()))?
		} else {
			unreachable!("resolver can't return this path")
		};

		direct.push(path);
		if let Some(direct) = check_path(&direct)? {
			return Ok(direct);
		}
		for library_path in &self.library_paths {
			let mut cloned = library_path.clone();
			cloned.push(path);
			if let Some(cloned) = check_path(&cloned)? {
				return Ok(cloned);
			}
		}
		bail!(ImportFileNotFound(from.clone(), path.to_owned()))
	}

	fn load_file_contents(&self, id: &SourcePath) -> Result<Vec<u8>> {
		let path = if let Some(f) = id.downcast_ref::<SourceFile>() {
			f.path()
		} else if id.downcast_ref::<SourceDirectory>().is_some() {
			bail!(ImportIsADirectory(id.clone()))
		} else if let Some(f) = id.downcast_ref::<SourceFifo>() {
			return Ok(f.1.to_vec());
		} else {
			unreachable!("other types are not supported in resolve");
		};
		let mut file = File::open(path).map_err(|_e| ResolvedFileNotFound(id.clone()))?;
		let mut out = Vec::new();
		file.read_to_end(&mut out)
			.map_err(|e| ImportIo(e.to_string()))?;
		Ok(out)
	}

	fn resolve_from_default(&self, path: &dyn AsPathLike) -> Result<SourcePath> {
		self.resolve_from(&SourcePath::default(), path)
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}
