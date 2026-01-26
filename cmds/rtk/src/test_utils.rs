//! Common test utilities.

use std::{
	any::Any,
	collections::HashMap,
	io::{self, ErrorKind, Write},
	path::{Path, PathBuf},
};

use jrsonnet_evaluator::{
	error::{ErrorKind::*, Result as JrsonnetResult},
	ImportResolver,
};
use jrsonnet_gcmodule::Trace;
use jrsonnet_parser::{SourceFile, SourcePath};

/// A writer that simulates a broken pipe (SIGPIPE scenario).
///
/// This writer immediately returns `ErrorKind::BrokenPipe` on any write attempt,
/// simulating what happens when stdout is connected to a process that has exited.
pub struct BrokenPipeWriter;

impl Write for BrokenPipeWriter {
	fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
		Err(io::Error::new(ErrorKind::BrokenPipe, "broken pipe"))
	}

	fn flush(&mut self) -> io::Result<()> {
		Err(io::Error::new(ErrorKind::BrokenPipe, "broken pipe"))
	}
}

/// An in-memory import resolver for testing.
///
/// Stores files in a HashMap and resolves imports from memory,
/// avoiding the need for filesystem access in tests.
#[derive(Default, Trace)]
pub struct MemoryImportResolver {
	#[trace(skip)]
	files: HashMap<PathBuf, Vec<u8>>,
}

impl MemoryImportResolver {
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a file to the in-memory filesystem.
	pub fn add_file(&mut self, path: impl Into<PathBuf>, content: impl Into<Vec<u8>>) {
		self.files.insert(path.into(), content.into());
	}

	/// Builder-style method to add a file.
	pub fn with_file(mut self, path: impl Into<PathBuf>, content: impl Into<Vec<u8>>) -> Self {
		self.add_file(path, content);
		self
	}
}

impl ImportResolver for MemoryImportResolver {
	fn resolve_from(&self, from: &SourcePath, path: &str) -> JrsonnetResult<SourcePath> {
		// Get the directory of the "from" file
		let base_dir = if let Some(f) = from.downcast_ref::<SourceFile>() {
			f.path().parent().map(|p| p.to_path_buf())
		} else if from.is_default() {
			Some(PathBuf::from("/"))
		} else {
			None
		};

		let base_dir = base_dir.unwrap_or_else(|| PathBuf::from("/"));

		// Try resolving relative to the base directory
		let resolved = base_dir.join(path);
		if self.files.contains_key(&resolved) {
			return Ok(SourcePath::new(SourceFile::new(resolved)));
		}

		// Try as absolute path
		let absolute = PathBuf::from(path);
		if self.files.contains_key(&absolute) {
			return Ok(SourcePath::new(SourceFile::new(absolute)));
		}

		Err(ImportFileNotFound(from.clone(), path.into()).into())
	}

	fn resolve(&self, path: &Path) -> JrsonnetResult<SourcePath> {
		if self.files.contains_key(path) {
			Ok(SourcePath::new(SourceFile::new(path.to_path_buf())))
		} else {
			Err(ResolvedFileNotFound(SourcePath::new(SourceFile::new(path.to_path_buf()))).into())
		}
	}

	fn load_file_contents(&self, resolved: &SourcePath) -> JrsonnetResult<Vec<u8>> {
		let path = if let Some(f) = resolved.downcast_ref::<SourceFile>() {
			f.path()
		} else {
			return Err(ImportIo(format!("unsupported source path type: {:?}", resolved)).into());
		};

		self.files
			.get(path)
			.cloned()
			.ok_or_else(|| ResolvedFileNotFound(resolved.clone()).into())
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}
