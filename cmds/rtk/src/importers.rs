use std::{
	collections::{HashMap, HashSet},
	fs,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use regex::Regex;

const DEFAULT_ENTRYPOINT: &str = "main.jsonnet";

struct CachedJsonnetFile {
	base: String,
	imports: Vec<String>,
	is_main_file: bool,
}

/// Pre-computed data for the importers search, including jsonnet files and canonical path cache.
struct ImportersContext {
	/// Map of file path -> CachedJsonnetFile
	jsonnet_files: HashMap<String, CachedJsonnetFile>,
	/// Map of path -> canonical path (for symlink resolution without syscalls)
	canonical_cache: HashMap<String, String>,
}

pub fn find_importers(root: &str, files: Vec<String>) -> Result<Vec<String>> {
	let root = fs::canonicalize(root).context("resolving root")?;
	let root_str = root.to_string_lossy().to_string();

	// Pre-compute the jsonnet files cache and canonical path cache ONCE before any recursion
	let context = build_importers_context(&root_str)?;

	find_importers_with_context(&root_str, files, &context)
}

/// Find importers for multiple files, returning a map of file -> list of importing environments.
/// This is more efficient than calling find_importers repeatedly because it builds the context once.
pub fn find_importers_batch(
	root: &str,
	files: Vec<String>,
) -> Result<HashMap<String, Vec<String>>> {
	let root = fs::canonicalize(root).context("resolving root")?;
	let root_str = root.to_string_lossy().to_string();

	// Pre-compute the jsonnet files cache and canonical path cache ONCE
	let context = build_importers_context(&root_str)?;

	let mut result = HashMap::new();
	for file in files {
		let importers = find_importers_with_context(&root_str, vec![file.clone()], &context)?;
		result.insert(file, importers);
	}

	Ok(result)
}

fn find_importers_with_context(
	root_str: &str,
	files: Vec<String>,
	context: &ImportersContext,
) -> Result<Vec<String>> {
	let root = Path::new(root_str);
	let mut importers_set = HashSet::new();

	// Handle files prefixed with `deleted:`. They need to be made absolute and we shouldn't try to find symlinks for them
	let mut files_to_check = Vec::new();
	let mut existing_files = Vec::new();

	for file in files {
		if file.starts_with("deleted:") {
			let deleted_file = file.trim_start_matches("deleted:");
			let deleted_path = Path::new(deleted_file);

			if !deleted_path.is_absolute() {
				// Try with both the absolute path and the path relative to the root
				if let Ok(abs_path) = fs::canonicalize(deleted_file) {
					files_to_check.push(abs_path.to_string_lossy().to_string());
				}
				let root_relative = root.join(deleted_file);
				files_to_check.push(root_relative.to_string_lossy().to_string());
			} else {
				files_to_check.push(deleted_file.to_string());
			}
			continue;
		}

		if !Path::new(&file).exists() {
			anyhow::bail!("file {:?} does not exist", file);
		}

		existing_files.push(file);
	}

	// Expand symlinks for existing files
	let expanded_files = expand_symlinks_in_files(root_str, existing_files)?;
	files_to_check.extend(expanded_files);

	// Cache for importers results to avoid recomputation
	let mut importers_cache: HashMap<String, Vec<String>> = HashMap::new();

	// Loop through all given files and add their importers to the list
	for file in &files_to_check {
		importers_set.insert(file.clone());
		let new_importers = find_importers_recursive(
			root_str,
			file,
			&mut HashSet::new(),
			context,
			&mut importers_cache,
		)?;

		for importer in new_importers {
			// Use cached canonical lookup instead of syscall
			let eval_importer = context
				.canonical_cache
				.get(&importer)
				.cloned()
				.unwrap_or_else(|| importer.clone());
			importers_set.insert(eval_importer);
		}
	}

	// Filter to only main files
	let mut main_files: Vec<String> = importers_set
		.into_iter()
		.filter(|path| {
			Path::new(path)
				.file_name()
				.and_then(|n| n.to_str())
				.map(|n| n == DEFAULT_ENTRYPOINT)
				.unwrap_or(false)
		})
		.collect();

	main_files.sort();
	Ok(main_files)
}

fn expand_symlinks_in_files(root: &str, files: Vec<String>) -> Result<Vec<String>> {
	let mut files_map = HashSet::new();

	// Build symlink map once for all files
	let symlink_map = build_symlink_map(root)?;

	for file in files {
		let abs_file = fs::canonicalize(&file).context("making file absolute")?;
		let abs_file_str = abs_file.to_string_lossy().to_string();
		files_map.insert(abs_file_str.clone());

		// Add the file after evaluating symlinks
		let symlink_eval = eval_symlinks(&abs_file_str)?;
		if symlink_eval != abs_file_str {
			files_map.insert(symlink_eval);
		}

		// Find all symlinks that point to this file using the pre-built map
		let symlinks = find_symlinks_from_map(&abs_file_str, &symlink_map);
		for symlink in symlinks {
			files_map.insert(symlink);
		}
	}

	let mut result: Vec<String> = files_map.into_iter().collect();
	result.sort();
	Ok(result)
}

fn eval_symlinks(path: &str) -> Result<String> {
	let path_buf = Path::new(path);
	if !path_buf.exists() {
		return Ok(path.to_string());
	}

	match fs::canonicalize(path) {
		Ok(p) => Ok(p.to_string_lossy().to_string()),
		Err(_) => Ok(path.to_string()),
	}
}

/// Recursively collect directories up to a certain depth for better parallelization
fn collect_directories_recursive(path: &Path, max_depth: usize) -> Vec<PathBuf> {
	if max_depth == 0 {
		return vec![path.to_path_buf()];
	}

	let Ok(entries) = fs::read_dir(path) else {
		return vec![path.to_path_buf()];
	};

	let mut dirs = Vec::new();
	let mut has_subdirs = false;

	for entry in entries.filter_map(|e| e.ok()) {
		let entry_path = entry.path();
		if entry_path.is_dir() {
			has_subdirs = true;
			dirs.extend(collect_directories_recursive(&entry_path, max_depth - 1));
		}
	}

	// If no subdirectories were found, return this directory itself
	if !has_subdirs {
		dirs.push(path.to_path_buf());
	}

	dirs
}

/// Build a map of canonical targets to their symlink paths
/// This is done once to avoid repeated walkdir traversals
fn build_symlink_map(root: &str) -> Result<HashMap<String, Vec<(String, String)>>> {
	use rayon::prelude::*;

	let root_path = Path::new(root);

	// Collect directories at multiple levels for better parallelization
	// This ensures work is evenly distributed even if directory structure is unbalanced
	let dirs_to_walk = collect_directories_recursive(root_path, 2);

	// Process each directory in parallel
	let symlink_entries: Vec<(String, (String, String))> = dirs_to_walk
		.par_iter()
		.flat_map(|dir| {
			walkdir::WalkDir::new(dir)
				.follow_links(false)
				.into_iter()
				.filter_map(|entry| {
					let entry = entry.ok()?;
					let path = entry.path();

					if !path.is_symlink() {
						return None;
					}

					let link_target = fs::read_link(path).ok()?;

					// Resolve the link target
					let resolved = if link_target.is_absolute() {
						link_target
					} else {
						path.parent().unwrap_or(Path::new("/")).join(link_target)
					};

					let canonical_target = fs::canonicalize(&resolved).ok()?;
					let canonical_target_str = canonical_target.to_string_lossy().to_string();
					let symlink_path = path.to_string_lossy().to_string();

					Some((
						canonical_target_str.clone(),
						(symlink_path, canonical_target_str),
					))
				})
				.collect::<Vec<_>>()
		})
		.collect();

	// Build the final map
	let mut symlink_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
	for (key, value) in symlink_entries {
		symlink_map.entry(key).or_insert_with(Vec::new).push(value);
	}

	Ok(symlink_map)
}

/// Find symlinks for a file using a pre-built symlink map
fn find_symlinks_from_map(
	file: &str,
	symlink_map: &HashMap<String, Vec<(String, String)>>,
) -> Vec<String> {
	let mut symlinks = Vec::new();

	// Check all entries in the symlink map
	for (canonical_target, links) in symlink_map {
		if file.contains(canonical_target) {
			for (symlink_path, _) in links {
				let result = file.replace(canonical_target, symlink_path);
				symlinks.push(result);
			}
		}
	}

	symlinks
}

fn find_importers_recursive<'a>(
	root: &str,
	search_for_file: &str,
	chain: &mut HashSet<String>,
	context: &ImportersContext,
	importers_cache: &'a mut HashMap<String, Vec<String>>,
) -> Result<Vec<String>> {
	// If we've already looked through this file in the current execution, don't do it again
	if chain.contains(search_for_file) {
		return Ok(Vec::new());
	}
	chain.insert(search_for_file.to_string());

	// Check cache - return clone only if found (this is much cheaper than cloning the entire jsonnet_files HashMap)
	let cache_key = format!("{}:{}", root, search_for_file);
	if let Some(cached) = importers_cache.get(&cache_key) {
		return Ok(cached.clone());
	}

	// Pre-compute canonical path for search_for_file once (avoid repeated lookups in path_matches)
	// Use cache if available, otherwise compute via syscall (only done once per recursive call)
	let search_for_file_canonical = context
		.canonical_cache
		.get(search_for_file)
		.cloned()
		.unwrap_or_else(|| {
			fs::canonicalize(search_for_file)
				.map(|p| p.to_string_lossy().to_string())
				.unwrap_or_else(|_| search_for_file.to_string())
		});

	let mut importers = Vec::new();
	let mut intermediate_importers = Vec::new();

	// Optimization: if the file is not in vendor/ or lib/, assume it's in an environment
	let root_vendor = Path::new(root).join("vendor");
	let root_lib = Path::new(root).join("lib");

	let is_file_lib_or_vendored = |file: &str| -> bool {
		let file_path = Path::new(file);
		file_path.starts_with(&root_vendor) || file_path.starts_with(&root_lib)
	};

	let searched_file_is_lib_or_vendored = is_file_lib_or_vendored(search_for_file);

	if !searched_file_is_lib_or_vendored {
		let searched_dir = Path::new(search_for_file)
			.parent()
			.unwrap_or(Path::new("/"));

		if let Some(entrypoint) = find_entrypoint(searched_dir) {
			// Found the main file for the searched file, add it as an importer
			importers.push(entrypoint);
		} else if searched_dir.exists() {
			// No main file found, add all main files in child dirs as importers
			let files = find_jsonnet_files(searched_dir)?;
			for file in files {
				if Path::new(&file)
					.file_name()
					.and_then(|n| n.to_str())
					.map(|n| n == DEFAULT_ENTRYPOINT)
					.unwrap_or(false)
				{
					importers.push(file);
				}
			}
		}
	}

	// Check all jsonnet files for imports in parallel
	use rayon::prelude::*;

	let search_basename = Path::new(search_for_file)
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or("")
		.to_string();

	let found_importers: Vec<(String, bool)> = context
		.jsonnet_files
		.par_iter()
		.filter_map(|(jsonnet_file_path, jsonnet_file_content)| {
			if jsonnet_file_content.imports.is_empty() {
				return None;
			}

			for import_path in &jsonnet_file_content.imports {
				// If the filename is not the same as the file we are looking for, skip it
				let import_basename = Path::new(import_path)
					.file_name()
					.and_then(|n| n.to_str())
					.unwrap_or("");

				if import_basename != search_basename {
					continue;
				}

				// Clean the import path
				let import_path_clean = Path::new(import_path)
					.components()
					.collect::<PathBuf>()
					.to_string_lossy()
					.to_string();

				let mut is_importer = false;

				// Match on relative imports with ..
				if import_path.starts_with("..") {
					let jsonnet_dir = Path::new(jsonnet_file_path)
						.parent()
						.unwrap_or(Path::new("/"));

					// Shallow import (one less level of ..)
					let shallow_import = import_path_clean.replacen("../", "", 1);
					let shallow_import_path = jsonnet_dir.join(&shallow_import);
					let shallow_import_clean = shallow_import_path
						.components()
						.collect::<PathBuf>()
						.to_string_lossy()
						.to_string();

					// Full import
					let import_full_path = jsonnet_dir.join(&import_path_clean);
					let import_full_clean = import_full_path
						.components()
						.collect::<PathBuf>()
						.to_string_lossy()
						.to_string();

					is_importer = path_matches_cached(
						&search_for_file_canonical,
						&import_full_clean,
						&context.canonical_cache,
					) || path_matches_cached(
						&search_for_file_canonical,
						&shallow_import_clean,
						&context.canonical_cache,
					);
				}

				// Match on imports to lib/ or vendor/
				// Skip this check if the import path starts with ../ (those are handled above as relative imports)
				if !is_importer && !import_path.starts_with("..") {
					let vendor_path = root_vendor.join(&import_path_clean);
					let lib_path = root_lib.join(&import_path_clean);
					is_importer = path_matches_cached(
						&search_for_file_canonical,
						&vendor_path.to_string_lossy(),
						&context.canonical_cache,
					) || path_matches_cached(
						&search_for_file_canonical,
						&lib_path.to_string_lossy(),
						&context.canonical_cache,
					);
				}

				// Match on imports to the base dir where the file is located
				if !is_importer {
					let base = if jsonnet_file_content.base.is_empty() {
						match find_base(jsonnet_file_path, root) {
							Ok(b) => b,
							Err(_) => continue,
						}
					} else {
						jsonnet_file_content.base.clone()
					};

					// Check if the search file is in the base directory and ends with the import path
					// But also ensure that the path segment before the import path in search_for_file
					// matches the path segment in the base (to avoid false positives)
					if search_for_file.starts_with(&base) && search_for_file.ends_with(import_path)
					{
						// Extract the part between base and the file
						let relative_to_base = search_for_file.strip_prefix(&base).unwrap_or("");
						let relative_to_base = relative_to_base.trim_start_matches('/');

						// The relative path should match the import path exactly
						is_importer = relative_to_base == import_path;
					}
				}

				// Also check if the import is relative to the directory of the importing file
				// This handles cases like 'text-file.txt' imported from 'vendor/vendored/main.libsonnet'
				if !is_importer {
					let importer_dir = Path::new(jsonnet_file_path)
						.parent()
						.unwrap_or(Path::new("/"));
					let import_full_path = importer_dir.join(import_path);
					let import_full_str = import_full_path.to_string_lossy().to_string();
					is_importer = path_matches_cached(
						&search_for_file_canonical,
						&import_full_str,
						&context.canonical_cache,
					);
				}

				if is_importer {
					return Some((jsonnet_file_path.clone(), jsonnet_file_content.is_main_file));
				}
			}
			None
		})
		.collect();

	// Process the results
	for (jsonnet_file_path, is_main_file) in found_importers {
		if is_main_file {
			importers.push(jsonnet_file_path.clone());
		}
		intermediate_importers.push(jsonnet_file_path);
	}

	// Process intermediate importers recursively
	if !intermediate_importers.is_empty() {
		for intermediate_importer in &intermediate_importers {
			importers.push(intermediate_importer.clone());
			let new_importers = find_importers_recursive(
				root,
				intermediate_importer,
				chain,
				context,
				importers_cache,
			)?;
			importers.extend(new_importers);
		}
	}

	// Filter out vendored files that are overridden
	let filtered_importers = if search_for_file.starts_with(root_vendor.to_str().unwrap_or("")) {
		let mut filtered = Vec::new();
		for importer in &importers {
			if let Ok(rel_path) = Path::new(search_for_file).strip_prefix(&root_vendor) {
				let vendored_in_env = Path::new(importer)
					.parent()
					.unwrap_or(Path::new("/"))
					.join("vendor")
					.join(rel_path);
				let vendored_in_env_str = vendored_in_env.to_string_lossy().to_string();

				if !context.jsonnet_files.contains_key(&vendored_in_env_str) {
					filtered.push(importer.clone());
				}
			}
		}
		filtered
	} else {
		importers
	};

	importers_cache.insert(cache_key, filtered_importers.clone());
	Ok(filtered_importers)
}

/// Build the importers context once for all files in the root directory.
/// This includes the jsonnet file cache and a canonical path cache for symlink resolution.
fn build_importers_context(root: &str) -> Result<ImportersContext> {
	let files = find_jsonnet_files(Path::new(root))?;

	// Compile regex once (thread-safe to share across threads)
	let imports_regexp = Regex::new(r#"import(str)?\s+['"]([^'"%()]+)['"]"#)?;

	// Process files in parallel, also computing canonical paths
	use rayon::prelude::*;
	let results: Result<Vec<_>> = files
		.par_iter()
		.map(|file| {
			let content = fs::read_to_string(file).context(format!("reading file {}", file))?;
			let is_main_file = file.ends_with(DEFAULT_ENTRYPOINT);

			let mut imports = Vec::new();
			for cap in imports_regexp.captures_iter(&content) {
				if let Some(import_path) = cap.get(2) {
					imports.push(import_path.as_str().to_string());
				}
			}

			// Compute canonical path for this file
			let canonical = fs::canonicalize(file)
				.map(|p| p.to_string_lossy().to_string())
				.unwrap_or_else(|_| file.clone());

			Ok((
				file.clone(),
				CachedJsonnetFile {
					base: String::new(),
					imports,
					is_main_file,
				},
				canonical,
			))
		})
		.collect();

	let results = results?;

	// Build both maps from the results
	let mut jsonnet_files = HashMap::with_capacity(results.len());
	let mut canonical_cache = HashMap::with_capacity(results.len() * 2);

	for (file, cached, canonical) in results {
		// Store original -> canonical mapping
		canonical_cache.insert(file.clone(), canonical.clone());
		// Also store canonical -> canonical (for lookups when we already have canonical)
		if file != canonical {
			canonical_cache.insert(canonical.clone(), canonical.clone());
		}
		jsonnet_files.insert(file, cached);
	}

	Ok(ImportersContext {
		jsonnet_files,
		canonical_cache,
	})
}

fn find_jsonnet_files(dir: &Path) -> Result<Vec<String>> {
	use rayon::prelude::*;

	// Collect directories at multiple levels for better parallelization
	let dirs_to_walk = collect_directories_recursive(dir, 2);

	// Process each directory in parallel
	let files: Vec<String> = dirs_to_walk
		.par_iter()
		.flat_map(|path| {
			walkdir::WalkDir::new(path)
				.into_iter()
				.filter_map(|entry| {
					let entry = entry.ok()?;
					let path = entry.path();

					if !path.is_file() {
						return None;
					}

					let ext = path.extension()?.to_string_lossy();
					if ext == "jsonnet" || ext == "libsonnet" {
						Some(path.to_string_lossy().to_string())
					} else {
						None
					}
				})
				.collect::<Vec<_>>()
		})
		.collect();

	Ok(files)
}

fn find_entrypoint(dir: &Path) -> Option<String> {
	let mut current_dir = dir;

	// Walk up the directory tree
	loop {
		if !current_dir.exists() {
			if let Some(parent) = current_dir.parent() {
				current_dir = parent;
				continue;
			} else {
				break;
			}
		}

		let entrypoint = current_dir.join(DEFAULT_ENTRYPOINT);
		if entrypoint.exists() {
			return Some(entrypoint.to_string_lossy().to_string());
		}

		// Try to go to parent
		if let Some(parent) = current_dir.parent() {
			current_dir = parent;
		} else {
			break;
		}
	}

	None
}

fn find_base(path: &str, root: &str) -> Result<String> {
	let path_buf = Path::new(path);
	let root_buf = Path::new(root);

	// Start from the file's directory and walk up
	let mut current = if path_buf.is_file() {
		path_buf.parent().unwrap_or(Path::new("/"))
	} else {
		path_buf
	};

	while current.starts_with(root_buf) {
		let main_file = current.join(DEFAULT_ENTRYPOINT);
		if main_file.exists() {
			return Ok(current.to_string_lossy().to_string());
		}

		if let Some(parent) = current.parent() {
			current = parent;
		} else {
			break;
		}
	}

	// If no main.jsonnet found, return the root
	Ok(root.to_string())
}

/// Check if two paths match, using a pre-computed canonical cache to avoid syscalls.
/// `path1_canonical` should already be the canonical version of the first path.
fn path_matches_cached(
	path1_canonical: &str,
	path2: &str,
	canonical_cache: &HashMap<String, String>,
) -> bool {
	if path1_canonical == path2 {
		return true;
	}

	// Look up path2's canonical form from cache
	if let Some(path2_canonical) = canonical_cache.get(path2) {
		return path1_canonical == path2_canonical;
	}

	// For paths not in cache, try to canonicalize if the file exists.
	// This handles text files and other non-jsonnet files that aren't in our cache.
	// The syscall is acceptable here since:
	// 1. Most paths are filtered by basename matching before reaching this point
	// 2. This is only needed for files not in the jsonnet cache (rare case)
	if let Ok(path2_canonical) = fs::canonicalize(path2) {
		return path1_canonical == path2_canonical.to_string_lossy();
	}

	// Path doesn't exist, no match
	false
}
