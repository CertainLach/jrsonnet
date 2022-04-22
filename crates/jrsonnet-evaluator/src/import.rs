use std::{
	any::Any,
	convert::TryFrom,
	fs,
	io::Read,
	path::{Path, PathBuf},
	rc::Rc,
};

use fs::File;
use jrsonnet_interner::IStr;

use crate::{
	error::{Error::*, Result},
	throw,
};

/// Implements file resolution logic for `import` and `importStr`
pub trait ImportResolver {
	/// Resolves real file path, e.g. `(/home/user/manifests, b.libjsonnet)` can correspond
	/// both to `/home/user/manifests/b.libjsonnet` and to `/home/user/${vendor}/b.libjsonnet`
	/// where `${vendor}` is a library path.
	fn resolve_file(&self, from: &Path, path: &Path) -> Result<Rc<Path>>;

	fn load_file_contents(&self, resolved: &Path) -> Result<Vec<u8>>;

	/// Reads file from filesystem, should be used only with path received from `resolve_file`
	fn load_file_str(&self, resolved: &Path) -> Result<IStr> {
		Ok(IStr::try_from(&self.load_file_contents(resolved)? as &[u8])
			.map_err(|_| ImportBadFileUtf8(resolved.to_path_buf()))?)
	}

	/// Reads file from filesystem, should be used only with path received from `resolve_file`
	fn load_file_bin(&self, resolved: &Path) -> Result<Rc<[u8]>> {
		Ok(self.load_file_contents(resolved)?.into())
	}

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

	fn load_file_contents(&self, _resolved: &Path) -> Result<Vec<u8>> {
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
			for library_path in &self.library_paths {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(cloned.into());
				}
			}
			throw!(ImportFileNotFound(from.to_owned(), path.to_owned()))
		}
	}
	fn load_file_contents(&self, id: &Path) -> Result<Vec<u8>> {
		let mut file = File::open(id).map_err(|_e| ResolvedFileNotFound(id.to_owned()))?;
		let mut out = Vec::new();
		file.read_to_end(&mut out)
			.map_err(|e| ImportIo(e.to_string()))?;
		Ok(out)
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}
