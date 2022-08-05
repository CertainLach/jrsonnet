use std::{
	any::Any,
	fs,
	io::Read,
	path::{Path, PathBuf},
};

use fs::File;
use jrsonnet_parser::SourcePath;

use crate::{
	error::{Error::*, Result},
	throw,
};

/// Implements file resolution logic for `import` and `importStr`
pub trait ImportResolver {
	/// Resolves real file path, e.g. `(/home/user/manifests, b.libjsonnet)` can correspond
	/// both to `/home/user/manifests/b.libjsonnet` and to `/home/user/${vendor}/b.libjsonnet`
	/// where `${vendor}` is a library path.
	fn resolve_file_relative(&self, from: &Path, path: &str) -> Result<SourcePath>;

	/// Load resolved file
	/// This should only be called with value returned from `resolve_file`, this cannot be resolved using associated type,
	/// as evaluator uses object instead of generic for [`ImportResolver`]
	fn load_file_contents(&self, resolved: &SourcePath) -> Result<Vec<u8>>;

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
	fn resolve_file_relative(&self, from: &Path, path: &str) -> Result<SourcePath> {
		throw!(ImportNotSupported(from.into(), path.into()))
	}

	fn load_file_contents(&self, _resolved: &SourcePath) -> Result<Vec<u8>> {
		panic!("dummy resolver can't load any file")
	}

	unsafe fn as_any(&self) -> &dyn Any {
		panic!("`as_any(&self)` is not supported by dummy resolver")
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
	fn resolve_file_relative(&self, from: &Path, path: &str) -> Result<SourcePath> {
		let mut direct = from.to_path_buf();
		direct.push(path);
		if direct.exists() {
			Ok(SourcePath::Path(
				direct.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
			))
		} else {
			for library_path in &self.library_paths {
				let mut cloned = library_path.clone();
				cloned.push(path);
				if cloned.exists() {
					return Ok(SourcePath::Path(
						cloned.canonicalize().map_err(|e| ImportIo(e.to_string()))?,
					));
				}
			}
			throw!(ImportFileNotFound(from.to_owned(), path.to_owned()))
		}
	}

	fn load_file_contents(&self, id: &SourcePath) -> Result<Vec<u8>> {
		let path = match id {
			SourcePath::Path(path) => path,
			_ => {
				panic!("this resolver can only resolve to path")
			}
		};
		let mut file = File::open(path).map_err(|_e| ResolvedFileNotFound(id.clone()))?;
		let mut out = Vec::new();
		file.read_to_end(&mut out)
			.map_err(|e| ImportIo(e.to_string()))?;
		Ok(out)
	}
	unsafe fn as_any(&self) -> &dyn Any {
		panic!("this resolver can't be used as any")
	}
}
