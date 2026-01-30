//! imports - Find all transitive imports of a Tanka environment
//!
//! This module provides functionality to discover all files that are
//! transitively imported by a Tanka environment's main.jsonnet file.

use std::{
	collections::HashSet,
	fs,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use regex::Regex;

use crate::jpath;

/// Find all transitive imports of an environment at the given path.
///
/// Returns a sorted list of file paths relative to the project root,
/// including the entrypoint file itself.
pub fn transitive_imports(dir: &str) -> Result<Vec<String>> {
	let dir = fs::canonicalize(dir).context("resolving directory")?;
	let dir = dir.to_string_lossy().to_string();

	// Resolve jpath to find root, base, and import paths
	let jpath_result = jpath::resolve(&dir)?;

	let root = &jpath_result.root;
	let entrypoint = &jpath_result.entrypoint;

	// Read the entrypoint file
	let content =
		fs::read_to_string(entrypoint).context(format!("reading {}", entrypoint.display()))?;

	// Build import paths for resolution
	let import_paths: Vec<PathBuf> = jpath_result.import_paths.clone();

	// Track all imports
	let mut imports: HashSet<PathBuf> = HashSet::new();

	// Recursively find all imports
	import_recursive(&mut imports, entrypoint, &content, &import_paths, root)?;

	// Add the entrypoint itself
	imports.insert(entrypoint.clone());

	// Convert to paths relative to root and sort
	let mut paths: Vec<String> = imports
		.into_iter()
		.filter_map(|p| {
			// Try to get path relative to root
			p.strip_prefix(root)
				.ok()
				.map(|rel| rel.to_string_lossy().to_string())
		})
		.collect();

	// Normalize path separators (for cross-platform compatibility)
	for path in &mut paths {
		*path = path.replace('\\', "/");
	}

	paths.sort();
	Ok(paths)
}

/// Recursively find all imports from a file
fn import_recursive(
	imports: &mut HashSet<PathBuf>,
	current_file: &Path,
	content: &str,
	import_paths: &[PathBuf],
	root: &Path,
) -> Result<()> {
	let imports_regexp = Regex::new(r#"import(str)?\s+['"]([^'"%()]+)['"]"#)?;

	let current_dir = current_file.parent().unwrap_or(Path::new("/"));

	for cap in imports_regexp.captures_iter(content) {
		let is_importstr = cap.get(1).is_some();
		let import_path_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");

		if import_path_str.is_empty() {
			continue;
		}

		// Try to resolve the import
		let resolved = resolve_import(import_path_str, current_dir, import_paths, root);

		if let Some(resolved_path) = resolved {
			// Check if we've already processed this file
			if imports.contains(&resolved_path) {
				continue;
			}

			// Resolve symlinks to get canonical path
			let canonical = fs::canonicalize(&resolved_path).unwrap_or(resolved_path.clone());
			if imports.contains(&canonical) {
				continue;
			}

			imports.insert(canonical.clone());

			// For importstr, we don't recurse (it's just loading text)
			if is_importstr {
				continue;
			}

			// Only recurse into jsonnet/libsonnet files
			let ext = canonical.extension().and_then(|e| e.to_str()).unwrap_or("");
			if ext == "jsonnet" || ext == "libsonnet" {
				// Read and recurse
				if let Ok(file_content) = fs::read_to_string(&canonical) {
					import_recursive(imports, &canonical, &file_content, import_paths, root)?;
				}
			}
		}
	}

	Ok(())
}

/// Resolve an import path to an absolute file path
fn resolve_import(
	import_path: &str,
	current_dir: &Path,
	import_paths: &[PathBuf],
	root: &Path,
) -> Option<PathBuf> {
	// Try relative to current directory first
	let relative = current_dir.join(import_path);
	if relative.exists() {
		return Some(relative);
	}

	// Try each import path (jpath)
	for jpath in import_paths {
		let candidate = jpath.join(import_path);
		if candidate.exists() {
			return Some(candidate);
		}
	}

	// For paths starting with ../, also try stripping and searching from import paths
	// (Go jsonnet compatibility)
	if import_path.starts_with("../") {
		let stripped = import_path.trim_start_matches("../");
		for jpath in import_paths {
			let candidate = jpath.join(stripped);
			if candidate.exists() {
				return Some(candidate);
			}
		}
	}

	// Try relative to root as a last resort
	let root_relative = root.join(import_path);
	if root_relative.exists() {
		return Some(root_relative);
	}

	None
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use super::*;

	fn test_root() -> PathBuf {
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/importTree")
	}

	#[test]
	fn test_transitive_imports() {
		let result = transitive_imports(test_root().to_str().unwrap()).unwrap();

		assert_eq!(
			result,
			vec![
				"main.jsonnet",
				"trees.jsonnet",
				"trees/apple.jsonnet",
				"trees/cherry.jsonnet",
				"trees/generic.libsonnet",
				"trees/peach.jsonnet",
			]
		);
	}

	#[test]
	fn test_transitive_imports_from_subdirectory() {
		// Passing the main.jsonnet file directly should work too
		let entrypoint = test_root().join("main.jsonnet");
		let result = transitive_imports(entrypoint.to_str().unwrap()).unwrap();

		assert_eq!(
			result,
			vec![
				"main.jsonnet",
				"trees.jsonnet",
				"trees/apple.jsonnet",
				"trees/cherry.jsonnet",
				"trees/generic.libsonnet",
				"trees/peach.jsonnet",
			]
		);
	}
}
