use crate::create_error;
use crate::error::{Error, Result};
use fs::File;
use std::fs;
use std::io::Read;
use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

pub trait ImportResolver {
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>>;
	fn load_file_contents(&self, resolved: &PathBuf) -> Result<Rc<str>>;
}

pub struct DummyImportResolver;
impl ImportResolver for DummyImportResolver {
	fn resolve_file(&self, from: &PathBuf, path: &PathBuf) -> Result<Rc<PathBuf>> {
		create_error(Error::ImportNotSupported(from.clone(), path.clone()))
	}
	fn load_file_contents(&self, _resolved: &PathBuf) -> Result<Rc<str>> {
		// Can be only caused by library direct consumer, not by supplied jsonnet
		panic!("dummy resolver can't load any file")
	}
}
impl Default for Box<dyn ImportResolver> {
	fn default() -> Self {
		Box::new(DummyImportResolver)
	}
}

pub struct FileImportResolver {
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
			create_error(Error::ImportFileNotFound(from.clone(), path.clone()))
		}
	}
	fn load_file_contents(&self, id: &PathBuf) -> Result<Rc<str>> {
		let mut file = File::open(id).map_err(|_e| {
			create_error::<()>(Error::ResolvedFileNotFound(id.clone()))
				.err()
				.unwrap()
		})?;
		let mut out = String::new();
		file.read_to_string(&mut out).map_err(|_e| {
			create_error::<()>(Error::ImportBadFileUtf8(id.clone()))
				.err()
				.unwrap()
		})?;
		Ok(out.into())
	}
}

pub struct CachingImportResolver {
	resolution_cache: RefCell<HashMap<(PathBuf, PathBuf), Result<Rc<PathBuf>>>>,
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
}
