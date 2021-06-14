use crate::{
	error::{Error::*, Result},
	throw,
};
use fs::File;
use jrsonnet_interner::IStr;
use std::fs;
use std::io::Read;
use std::{
	any::Any,
	cell::RefCell,
	collections::HashMap,
	path::{Path, PathBuf},
	rc::Rc,
};

/// Implements file resolution logic for `import` and `importStr`
pub trait ImportResolver {
	/// Resolves real file path, e.g. `(/home/user/manifests, b.libjsonnet)` can correspond
	/// both to `/home/user/manifests/b.libjsonnet` and to `/home/user/${vendor}/b.libjsonnet`
	/// where `${vendor}` is a library path.
	fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>>;

	/// Reads file from filesystem, should be used only with path received from `resolve_file`
	fn load_file_contents(&self, resolved: &Path) -> Result<IStr>;

	/// # Safety
	///
	/// For use only in bindings, should not be used elsewhere.
	/// Implementations which are not intended to be used in bindings
	/// should panic on call to this method.
	unsafe fn as_any(&self) -> &dyn Any;
}

/// Dummy resolver, can't resolve/load any file
pub struct DummyImportResolver;
impl ImportResolver for DummyImportResolver {
	fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>> {
		throw!(ImportNotSupported(from.into(), path.into()))
	}

	fn load_file_contents(&self, _resolved: &Path) -> Result<IStr> {
		// Can be only caused by library direct consumer, not by supplied jsonnet
		panic!("dummy resolver can't load any file")
	}

	unsafe fn as_any(&self) -> &dyn Any {
		panic!("`as_any($self)` is not supported by dummy resolver")
	}
}
#[allow(clippy::use_self)]
impl Default for Box<dyn ImportResolver> {
	fn default() -> Self {
		Box::new(DummyImportResolver)
	}
}

/// File resolver, can load file from both FS and library paths
#[derive(Default)]
pub struct FileImportResolver {
	/// Library directories to search for file.
	/// Referred to as `jpath` in original jsonnet implementation.
	pub library_paths: Vec<PathBuf>,
}
impl ImportResolver for FileImportResolver {
	fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>> {
		let mut direct = from.to_path_buf();
		direct.push(path);
		if direct.exists() {
			Ok(direct.into())
		} else {
			for library_path in self.library_paths.iter() {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(cloned.into());
				}
			}
			throw!(ImportFileNotFound(from.to_owned(), path.to_owned()))
		}
	}
	fn load_file_contents(&self, id: &Path) -> Result<IStr> {
		let mut file = File::open(id).map_err(|_e| ResolvedFileNotFound(id.to_owned()))?;
		let mut out = String::new();
		file.read_to_string(&mut out)
			.map_err(|_e| ImportBadFileUtf8(id.to_owned()))?;
		Ok(out.into())
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}

type ResolutionData = (PathBuf, PathBuf);

/// Caches results of the underlying resolver
pub struct CachingImportResolver {
	resolution_cache: RefCell<HashMap<ResolutionData, Result<Rc<Path>>>>,
	loading_cache: RefCell<HashMap<PathBuf, Result<IStr>>>,
	inner: Box<dyn ImportResolver>,
}
impl ImportResolver for CachingImportResolver {
	fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>> {
		self.resolution_cache
			.borrow_mut()
			.entry((from.to_owned(), path.to_owned()))
			.or_insert_with(|| self.inner.resolve_file(from, path))
			.clone()
	}

	fn load_file_contents(&self, resolved: &Path) -> Result<IStr> {
		self.loading_cache
			.borrow_mut()
			.entry(resolved.to_owned())
			.or_insert_with(|| self.inner.load_file_contents(resolved))
			.clone()
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}
