//! jpath - Jsonnet import path resolution for Tanka environments
//!
//! This module handles finding the project root, environment base directory,
//! and constructing the import paths needed by the jsonnet evaluator.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

/// Default entrypoint filename for environments
pub const DEFAULT_ENTRYPOINT: &str = "main.jsonnet";

/// Files that indicate a project root (in order of precedence)
const ROOT_MARKERS: &[&str] = &["tkrc.yaml", "jsonnetfile.json"];

/// Result of resolving jpath for an environment
#[derive(Debug)]
pub struct JpathResult {
	/// The project root directory (contains jsonnetfile.json or tkrc.yaml)
	/// Used by export command for output path calculation
	#[allow(dead_code)]
	pub root: PathBuf,
	/// The environment base directory (contains main.jsonnet)
	pub base: PathBuf,
	/// The entrypoint file path (absolute)
	pub entrypoint: PathBuf,
	/// Import paths for jsonnet evaluation (in order: base, lib, base/vendor, root/vendor)
	pub import_paths: Vec<PathBuf>,
}

/// Resolve jpath for the given path (file or directory)
///
/// This finds:
/// - Project root: directory containing tkrc.yaml or jsonnetfile.json
/// - Environment base: directory containing main.jsonnet
/// - Import paths: [base, lib, base/vendor, root/vendor]
pub fn resolve(path: &str) -> Result<JpathResult> {
	let path = PathBuf::from(path);
	let abs_path = if path.is_absolute() {
		path
	} else {
		std::env::current_dir()?.join(path)
	};

	// Find the project root
	let root = find_root(&abs_path)?;

	// Find the environment base
	let base = find_base(&abs_path, &root)?;

	// Get the entrypoint filename
	let filename = get_filename(&abs_path)?;
	let entrypoint = base.join(&filename);

	// Build import paths (order matters - jrsonnet searches in reverse order from the list,
	// so we put higher priority paths at the end)
	// Tanka Go uses: [vendor, base/vendor, lib, base] searched in reverse
	// So we provide: [base, lib, base/vendor, root/vendor]
	let import_paths = vec![
		base.clone(),
		root.join("lib"),
		base.join("vendor"),
		root.join("vendor"),
	];

	Ok(JpathResult {
		root,
		base,
		entrypoint,
		import_paths,
	})
}

/// Find the project root directory by looking for marker files
pub fn find_root(start: &Path) -> Result<PathBuf> {
	let start_dir = fs_dir(start)?;

	for marker in ROOT_MARKERS {
		if let Some(root) = find_parent_with_file(&start_dir, marker) {
			return Ok(root);
		}
	}

	bail!(
		"could not find project root (no {} found in parent directories of {})",
		ROOT_MARKERS.join(" or "),
		start.display()
	)
}

/// Find the environment base directory (contains the entrypoint file)
fn find_base(path: &Path, root: &Path) -> Result<PathBuf> {
	let start_dir = fs_dir(path)?;
	let filename = get_filename(path)?;

	find_parent_with_file_bounded(&start_dir, &filename, root).ok_or_else(|| {
		anyhow::anyhow!(
			"could not find environment base (no {} found between {} and {})",
			filename,
			start_dir.display(),
			root.display()
		)
	})
}

/// Get the entrypoint filename from a path
fn get_filename(path: &Path) -> Result<String> {
	if path.is_dir() {
		return Ok(DEFAULT_ENTRYPOINT.to_string());
	}

	path.file_name()
		.and_then(|n| n.to_str())
		.map(|s| s.to_string())
		.ok_or_else(|| anyhow::anyhow!("invalid path: {}", path.display()))
}

/// Get the directory for a path (if path is file, returns parent; if dir, returns itself)
fn fs_dir(path: &Path) -> Result<PathBuf> {
	let abs = if path.is_absolute() {
		path.to_path_buf()
	} else {
		std::env::current_dir()?.join(path)
	};

	// Handle case where path doesn't exist yet - get the most-likely dir
	if !abs.exists() {
		// If path looks like a file (has extension), use parent
		if abs.extension().is_some() {
			return abs
				.parent()
				.map(|p| p.to_path_buf())
				.ok_or_else(|| anyhow::anyhow!("invalid path: {}", abs.display()));
		}
		return Ok(abs);
	}

	if abs.is_dir() {
		Ok(abs)
	} else {
		abs.parent()
			.map(|p| p.to_path_buf())
			.ok_or_else(|| anyhow::anyhow!("invalid path: {}", abs.display()))
	}
}

/// Find a parent directory containing the specified file
fn find_parent_with_file(start: &Path, filename: &str) -> Option<PathBuf> {
	let mut current = start.to_path_buf();
	loop {
		if current.join(filename).exists() {
			return Some(current);
		}
		if !current.pop() {
			return None;
		}
	}
}

/// Find a parent directory containing the specified file, bounded by a root directory
fn find_parent_with_file_bounded(start: &Path, filename: &str, root: &Path) -> Option<PathBuf> {
	let mut current = start.to_path_buf();
	loop {
		if current.join(filename).exists() {
			return Some(current);
		}
		// Stop if we've reached the root or can't go higher
		if current == root || !current.pop() {
			return None;
		}
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn test_default_entrypoint() {
		assert_eq!(DEFAULT_ENTRYPOINT, "main.jsonnet");
	}

	#[test]
	fn test_resolve_finds_root_and_base() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		// Create project structure
		fs::write(root.join("jsonnetfile.json"), r#"{"version": 1}"#).unwrap();
		fs::create_dir_all(root.join("environments/test")).unwrap();
		fs::write(root.join("environments/test/main.jsonnet"), "{}").unwrap();

		let result = resolve(root.join("environments/test").to_str().unwrap()).unwrap();

		assert_eq!(result.root, root);
		assert_eq!(result.base, root.join("environments/test"));
		assert_eq!(
			result.entrypoint,
			root.join("environments/test/main.jsonnet")
		);
	}

	#[test]
	fn test_resolve_uses_tkrc_over_jsonnetfile() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		// Create both marker files - tkrc.yaml should take precedence
		fs::write(root.join("tkrc.yaml"), "").unwrap();
		fs::write(root.join("jsonnetfile.json"), r#"{"version": 1}"#).unwrap();
		fs::write(root.join("main.jsonnet"), "{}").unwrap();

		let result = resolve(root.to_str().unwrap()).unwrap();
		assert_eq!(result.root, root);
	}

	#[test]
	fn test_resolve_import_paths_order() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		fs::write(root.join("env/main.jsonnet"), "{}").unwrap();

		let result = resolve(root.join("env").to_str().unwrap()).unwrap();

		// Import paths should be: [base, lib, base/vendor, root/vendor]
		assert_eq!(result.import_paths.len(), 4);
		assert_eq!(result.import_paths[0], root.join("env"));
		assert_eq!(result.import_paths[1], root.join("lib"));
		assert_eq!(result.import_paths[2], root.join("env/vendor"));
		assert_eq!(result.import_paths[3], root.join("vendor"));
	}

	#[test]
	fn test_resolve_no_root_fails() {
		let temp = TempDir::new().unwrap();
		// Don't create jsonnetfile.json or tkrc.yaml
		fs::write(temp.path().join("main.jsonnet"), "{}").unwrap();

		let result = resolve(temp.path().to_str().unwrap());
		assert!(result.is_err());
		assert!(result
			.unwrap_err()
			.to_string()
			.contains("could not find project root"));
	}

	#[test]
	fn test_resolve_custom_entrypoint() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::write(root.join("custom.jsonnet"), "{}").unwrap();

		let result = resolve(root.join("custom.jsonnet").to_str().unwrap()).unwrap();
		assert_eq!(result.entrypoint, root.join("custom.jsonnet"));
	}

	#[test]
	fn test_resolve_deeply_nested_env() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		// Create deeply nested structure like deployment_tools
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("ksonnet/environments/cortex/ops-us-west")).unwrap();
		fs::write(
			root.join("ksonnet/environments/cortex/ops-us-west/main.jsonnet"),
			"{}",
		)
		.unwrap();

		let result = resolve(
			root.join("ksonnet/environments/cortex/ops-us-west")
				.to_str()
				.unwrap(),
		)
		.unwrap();

		assert_eq!(result.root, root);
		assert_eq!(
			result.base,
			root.join("ksonnet/environments/cortex/ops-us-west")
		);
	}

	#[test]
	fn test_resolve_with_vendor_directories() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("vendor")).unwrap();
		fs::create_dir_all(root.join("lib")).unwrap();
		fs::create_dir_all(root.join("env/vendor")).unwrap();
		fs::write(root.join("env/main.jsonnet"), "{}").unwrap();

		let result = resolve(root.join("env").to_str().unwrap()).unwrap();

		// Verify all expected paths are in import_paths
		assert!(result.import_paths.contains(&root.join("vendor")));
		assert!(result.import_paths.contains(&root.join("lib")));
		assert!(result.import_paths.contains(&root.join("env/vendor")));
		assert!(result.import_paths.contains(&root.join("env")));
	}

	#[test]
	fn test_resolve_file_path_directly() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		fs::write(root.join("env/main.jsonnet"), "{}").unwrap();

		// Pass the file path directly instead of directory
		let result = resolve(root.join("env/main.jsonnet").to_str().unwrap()).unwrap();

		assert_eq!(result.base, root.join("env"));
		assert_eq!(result.entrypoint, root.join("env/main.jsonnet"));
	}

	#[test]
	fn test_resolve_no_main_jsonnet_fails() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		// Don't create main.jsonnet

		let result = resolve(root.join("env").to_str().unwrap());
		assert!(result.is_err());
		assert!(result
			.unwrap_err()
			.to_string()
			.contains("could not find environment base"));
	}
}
