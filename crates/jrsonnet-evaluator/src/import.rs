use std::{
	any::Any,
	cell::RefCell,
	env::current_dir,
	fs,
	io::{ErrorKind, Read},
	path::{Path, PathBuf},
};

use boa_gc::{Finalize, Trace};
use fs::File;
use jrsonnet_parser::{SourceDirectory, SourceFile, SourcePath};

use crate::{
	bail,
	error::{ErrorKind::*, Result},
	DynGcBox,
};

/// Implements file resolution logic for `import` and `importStr`
pub trait ImportResolver: Trace + 'static {
	/// Resolves file path, e.g. `(/home/user/manifests, b.libjsonnet)` can correspond
	/// both to `/home/user/manifests/b.libjsonnet` and to `/home/user/${vendor}/b.libjsonnet`
	/// where `${vendor}` is a library path.
	///
	/// `from` should only be returned from [`ImportResolver::resolve`], or from other defined file, any other value
	/// may result in panic
	fn resolve_from(&self, from: &SourcePath, path: &str) -> Result<SourcePath> {
		bail!(ImportNotSupported(from.clone(), path.into()))
	}
	fn resolve_from_default(&self, path: &str) -> Result<SourcePath> {
		self.resolve_from(&SourcePath::default(), path)
	}
	/// Resolves absolute path, doesn't supports jpath and other fancy things
	fn resolve(&self, path: &Path) -> Result<SourcePath> {
		bail!(AbsoluteImportNotSupported(path.to_owned()))
	}

	/// Load resolved file
	/// This should only be called with value returned from [`ImportResolver::resolve_file`]/[`ImportResolver::resolve`],
	/// this cannot be resolved using associated type, as evaluator uses object instead of generic for [`ImportResolver`]
	fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>>;

	/// For downcasts
	fn as_any(&self) -> &dyn Any;
}
impl<T: ImportResolver> ImportResolver for DynGcBox<T> {
	fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>> {
		self.value().load_file_contents(resolved)
	}

	fn as_any(&self) -> &dyn Any {
		self.value().as_any()
	}
}

/// Dummy resolver, can't resolve/load any file
#[derive(Trace, Finalize)]
pub struct DummyImportResolver;
impl ImportResolver for DummyImportResolver {
	fn load_file_contents(&self, _resolved: &SourcePath) -> Result<Vec<u8>> {
		panic!("dummy resolver can't load any file")
	}

	fn as_any(&self) -> &dyn Any {
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
#[derive(Default, Trace, Finalize)]
pub struct FileImportResolver {
	/// Library directories to search for file.
	/// Referred to as `jpath` in original jsonnet implementation.
	#[unsafe_ignore_trace]
	library_paths: RefCell<Vec<PathBuf>>,
}
impl FileImportResolver {
	pub fn new(jpath: Vec<PathBuf>) -> Self {
		Self {
			library_paths: RefCell::new(jpath),
		}
	}
	/// Dynamically add new jpath, used by bindings
	pub fn add_jpath(&self, path: PathBuf) {
		self.library_paths.borrow_mut().push(path);
	}
}

impl ImportResolver for FileImportResolver {
	fn resolve_from(&self, from: &SourcePath, path: &str) -> Result<SourcePath> {
		let mut direct = if let Some(f) = from.downcast_ref::<SourceFile>() {
			let mut o = f.path().to_owned();
			o.pop();
			o
		} else if let Some(d) = from.downcast_ref::<SourceDirectory>() {
			d.path().to_owned()
		} else if from.is_default() {
			current_dir().map_err(|e| ImportIo(e.to_string()))?
		} else {
			unreachable!("resolver can't return this path")
		};
		direct.push(path);
		if direct.is_file() {
			Ok(SourcePath::new(SourceFile::new(
				direct.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
			)))
		} else {
			for library_path in self.library_paths.borrow().iter() {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(SourcePath::new(SourceFile::new(
						cloned.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
					)));
				}
			}
			bail!(ImportFileNotFound(from.clone(), path.to_owned()))
		}
	}
	fn resolve(&self, path: &Path) -> Result<SourcePath> {
		let meta = match fs::metadata(path) {
			Ok(v) => v,
			Err(e) if e.kind() == ErrorKind::NotFound => {
				bail!(AbsoluteImportFileNotFound(path.to_owned()))
			}
			Err(e) => bail!(ImportIo(e.to_string())),
		};
		if meta.is_file() {
			Ok(SourcePath::new(SourceFile::new(
				path.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
			)))
		} else if meta.is_dir() {
			Ok(SourcePath::new(SourceDirectory::new(
				path.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
			)))
		} else {
			unreachable!("this can't be a symlink")
		}
	}

	fn load_file_contents(&self, id: &SourcePath) -> Result<Vec<u8>> {
		let path = if let Some(f) = id.downcast_ref::<SourceFile>() {
			f.path()
		} else if id.downcast_ref::<SourceDirectory>().is_some() || id.is_default() {
			bail!(ImportIsADirectory(id.clone()))
		} else {
			unreachable!("other types are not supported in resolve");
		};
		let mut file = File::open(path).map_err(|_e| ResolvedFileNotFound(id.clone()))?;
		let mut out = Vec::new();
		file.read_to_end(&mut out)
			.map_err(|e| ImportIo(e.to_string()))?;
		Ok(out)
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn resolve_from_default(&self, path: &str) -> Result<SourcePath> {
		self.resolve_from(&SourcePath::default(), path)
	}
}
