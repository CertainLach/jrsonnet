use crate::{
	error::{Error::*, Result},
	throw,
};
use fs::File;
use std::fs;
use std::io::Read;
use std::{any::Any, cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

/// Implements file resolution logic for `import` and `importStr`
pub trait ImportResolver {
	/// Resolve real file path, i.e
	/// `(/home/user/manifests, b.libsonnet)` can resolve to both `/home/user/manifests/b.libsonnet` and to `/home/user/vendor/b.libsonnet`
	/// (Where vendor is a library path)
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>>;
	/// Reads file from filesystem, should be used only with path received from `resolve_file`
	fn load_file_contents(&self, resolved: &PathBuf) -> Result<Rc<str>>;
	/// # Safety
	///
	/// For use in bindings, do not try to use it elsewhere
	/// Implementations, which are not intended to be
	/// used in bindings, should panic in this method
	unsafe fn as_any(&self) -> &dyn Any;
}

/// Dummy resolver, can't resolve/load any file
pub struct DummyImportResolver;
impl ImportResolver for DummyImportResolver {
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>> {
		throw!(ImportNotSupported(from.clone(), path.clone()))
	}
	fn load_file_contents(&self, _resolved: &PathBuf) -> Result<Rc<str>> {
		// Can be only caused by library direct consumer, not by supplied jsonnet
		panic!("dummy resolver can't load any file")
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}
impl Default for Box<dyn ImportResolver> {
	fn default() -> Self {
		Box::new(DummyImportResolver)
	}
}

/// File resolver, can load file from both FS and library paths
#[derive(Default)]
pub struct FileImportResolver {
	/// Library directories to search for file
	/// In original jsonnet referred as jpath
	pub library_paths: Vec<PathBuf>,
}
impl ImportResolver for FileImportResolver {
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>> {
		let mut new_path = from.clone();
		new_path.push(path);
		if new_path.exists() {
			Ok(Rc::new(new_path))
		} else {
			for library_path in self.library_paths.iter() {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(Rc::new(cloned));
				}
			}
			throw!(ImportFileNotFound(from.clone(), path.clone()))
		}
	}
	fn load_file_contents(&self, id: &PathBuf) -> Result<Rc<str>> {
		let mut file = File::open(id).map_err(|_e| ResolvedFileNotFound(id.clone()))?;
		let mut out = String::new();
		file.read_to_string(&mut out)
			.map_err(|_e| ImportBadFileUtf8(id.clone()))?;
		Ok(out.into())
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}

type ResolutionData = (PathBuf, PathBuf);

/// Caches results of underlying resolver implementation
pub struct CachingImportResolver {
	resolution_cache: RefCell<HashMap<ResolutionData, Result<Rc<PathBuf>>>>,
	loading_cache: RefCell<HashMap<PathBuf, Result<Rc<str>>>>,
	inner: Box<dyn ImportResolver>,
}
impl ImportResolver for CachingImportResolver {
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>> {
		self.resolution_cache
			.borrow_mut()
			.entry((from.clone(), path.clone()))
			.or_insert_with(|| self.inner.resolve_file(from, path))
			.clone()
	}
	fn load_file_contents(&self, resolved: &PathBuf) -> Result<Rc<str>> {
		self.loading_cache
			.borrow_mut()
			.entry(resolved.clone())
			.or_insert_with(|| self.inner.load_file_contents(resolved))
			.clone()
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}
