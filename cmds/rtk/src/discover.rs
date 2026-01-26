//! discover - Find Tanka environments in directory trees
//!
//! This module handles discovering all Tanka environments within given paths.
//! An environment is identified by the presence of either:
//! - `spec.json` (static environment)
//! - `main.jsonnet` with inline environment definition

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tracing::trace;
use walkdir::WalkDir;

/// Files that indicate a Tanka environment
const ENV_MARKERS: &[&str] = &["spec.json", "main.jsonnet"];

/// Directories to skip during discovery
const SKIP_DIRS: &[&str] = &["vendor", "node_modules", ".git", "lib"];

/// MetadataEvalScript finds Environment objects (without their .data field)
/// This matches Tanka's MetadataEvalScript in pkg/tanka/evaluators.go
const METADATA_EVAL_SCRIPT: &str = r#"
local noDataEnv(object) =
  std.prune(
    if std.isObject(object)
    then
      if std.objectHas(object, 'apiVersion')
         && std.objectHas(object, 'kind')
      then
        if object.kind == 'Environment'
        then object { data+:: {} }
        else {}
      else
        std.mapWithKey(
          function(key, obj)
            noDataEnv(obj),
          object
        )
    else if std.isArray(object)
    then
      std.map(
        function(obj)
          noDataEnv(obj),
        object
      )
    else {}
  );

noDataEnv(main)
"#;

/// Result of environment discovery
#[derive(Debug, Clone)]
pub struct DiscoveredEnv {
	/// Path to the environment directory
	pub path: PathBuf,
	/// Whether this is a static environment (has spec.json)
	#[allow(dead_code)]
	pub is_static: bool,
	/// For inline environments with multiple sub-environments, this is the name of the specific environment
	/// For static environments or single inline environments, this is None
	pub env_name: Option<String>,
	/// The exportJsonnetImplementation from the inline environment spec, if present
	/// This is used to determine whether to use jrsonnet-compatible output formatting
	pub export_jsonnet_implementation: Option<String>,
}

/// Find all Tanka environments in the given paths
///
/// This walks the directory tree looking for environments.
/// When an environment is found, its subdirectories are not searched.
pub fn find_environments(paths: &[String]) -> Result<Vec<DiscoveredEnv>> {
	let mut envs = Vec::new();
	let mut seen_dirs: HashSet<PathBuf> = HashSet::new();

	for path in paths {
		trace!("Processing path: {}", path);
		let path = PathBuf::from(path);
		let abs_path = if path.is_absolute() {
			trace!("Path is absolute");
			path
		} else {
			let cwd = std::env::current_dir()?;
			trace!("Path is relative, cwd={}", cwd.display());
			cwd.join(path)
		};

		// If the path is a file (e.g., main.jsonnet), use its parent directory
		let abs_path = if abs_path.is_file() {
			let parent = abs_path
				.parent()
				.map(|p| p.to_path_buf())
				.unwrap_or(abs_path);
			trace!(
				"Path is a file, using parent directory: {}",
				parent.display()
			);
			parent
		} else {
			abs_path
		};

		trace!(
			"Resolved abs_path={}, exists={}, is_dir={}",
			abs_path.display(),
			abs_path.exists(),
			abs_path.is_dir()
		);

		// If path is directly an environment, add it
		if is_environment(&abs_path) {
			trace!("Path is directly an environment: {}", abs_path.display());
			if seen_dirs.insert(abs_path.clone()) {
				let is_static = abs_path.join("spec.json").exists();
				if is_static {
					// Static environment - read exportJsonnetImplementation from spec.json
					let export_impl = read_export_impl_from_spec(&abs_path);
					envs.push(DiscoveredEnv {
						is_static: true,
						path: abs_path,
						env_name: None,
						export_jsonnet_implementation: export_impl,
					});
				} else {
					// Inline environment(s) - discover sub-environments
					match discover_inline_environments(&abs_path) {
						Ok(inline_envs) => envs.extend(inline_envs),
						Err(_) => {
							// If discovery fails, add as single env with no name
							envs.push(DiscoveredEnv {
								is_static: false,
								path: abs_path,
								env_name: None,
								export_jsonnet_implementation: None,
							});
						}
					}
				}
			}
			continue;
		} else {
			trace!(
				"Path is NOT directly an environment, will walk directory tree: {}",
				abs_path.display()
			);
		}

		// Walk the directory tree, filtering out directories we want to skip
		let walker = WalkDir::new(&abs_path)
			.follow_links(true)
			.into_iter()
			.filter_entry(|e| {
				// Only filter directories
				if !e.file_type().is_dir() {
					return true;
				}
				// Skip certain directory names
				if let Some(name) = e.file_name().to_str() {
					if SKIP_DIRS.contains(&name) || name.starts_with('.') {
						return false;
					}
				}
				true
			});

		for entry in walker {
			let entry = match entry {
				Ok(e) => e,
				Err(_) => continue,
			};

			let entry_path = entry.path();

			if entry.file_type().is_dir() && is_environment(entry_path) {
				let canonical = entry_path.to_path_buf();
				if seen_dirs.insert(canonical.clone()) {
					let is_static = canonical.join("spec.json").exists();
					if is_static {
						// Static environment - read exportJsonnetImplementation from spec.json
						let export_impl = read_export_impl_from_spec(&canonical);
						envs.push(DiscoveredEnv {
							is_static: true,
							path: canonical,
							env_name: None,
							export_jsonnet_implementation: export_impl,
						});
					} else {
						// Inline environment(s) - discover sub-environments
						match discover_inline_environments(&canonical) {
							Ok(inline_envs) => envs.extend(inline_envs),
							Err(_) => {
								// If discovery fails, add as single env with no name
								envs.push(DiscoveredEnv {
									is_static: false,
									path: canonical,
									env_name: None,
									export_jsonnet_implementation: None,
								});
							}
						}
					}
				}
			}
		}
	}

	Ok(envs)
}

/// Check if a directory is a Tanka environment
fn is_environment(path: &Path) -> bool {
	if !path.is_dir() {
		trace!(
			"is_environment: {} is not a directory (exists={})",
			path.display(),
			path.exists()
		);
		return false;
	}

	// Check for environment markers
	for marker in ENV_MARKERS {
		let marker_path = path.join(marker);
		if marker_path.exists() {
			trace!(
				"is_environment: {} has marker {} -> true",
				path.display(),
				marker
			);
			return true;
		}
	}

	trace!(
		"is_environment: {} has no markers (spec.json or main.jsonnet) -> false",
		path.display()
	);
	false
}

/// Read exportJsonnetImplementation from spec.json if it exists
fn read_export_impl_from_spec(path: &Path) -> Option<String> {
	let spec_path = path.join("spec.json");
	if !spec_path.exists() {
		return None;
	}

	let content = std::fs::read_to_string(&spec_path).ok()?;
	let json: serde_json::Value = serde_json::from_str(&content).ok()?;
	json.get("spec")?
		.get("exportJsonnetImplementation")?
		.as_str()
		.map(|s| s.to_string())
}

/// Discover inline environments within a main.jsonnet file
/// Returns a list of DiscoveredEnv, one for each sub-environment found
fn discover_inline_environments(path: &Path) -> Result<Vec<DiscoveredEnv>> {
	use jrsonnet_evaluator::{manifest::JsonFormat, State};
	use jrsonnet_stdlib::ContextInitializer;

	let main_path = path.join("main.jsonnet");
	if !main_path.exists() {
		return Ok(vec![DiscoveredEnv {
			is_static: false,
			path: path.to_path_buf(),
			env_name: None,
			export_jsonnet_implementation: None,
		}]);
	}

	// Set up import paths
	let mut import_paths = vec![path.to_path_buf()];

	// Find project root and add lib/vendor directories
	if let Ok(root) = crate::jpath::find_root(path) {
		for subdir in &["lib", "vendor"] {
			let dir_path = root.join(subdir);
			if dir_path.is_dir() {
				import_paths.push(dir_path);
			}
		}
	}

	// Create evaluator state
	let import_resolver = jrsonnet_evaluator::FileImportResolver::new(import_paths);
	let mut builder = State::builder();
	builder.import_resolver(import_resolver);

	use jrsonnet_evaluator::trace::PathResolver;
	// Use Absolute resolver so std.thisFile returns absolute paths (like tk does)
	let ctx_init = ContextInitializer::new(PathResolver::Absolute);

	// Register native functions (helmTemplate, parseYaml, etc.) for discovery
	crate::eval::register_native_functions(&ctx_init);

	builder.context_initializer(ctx_init);

	let state = builder.build();

	// Evaluate with MetadataEvalScript to get environment list without data
	let eval_script = format!(
		"local main = (import '{}');\n{}",
		main_path.file_name().unwrap().to_string_lossy(),
		METADATA_EVAL_SCRIPT
	);

	let result = state
		.evaluate_snippet("<metadata-eval>", &eval_script)
		.map_err(|e| anyhow::anyhow!("evaluating jsonnet for environment discovery: {}", e))?;

	let json_str = result
		.manifest(JsonFormat::cli(2))
		.map_err(|e| anyhow::anyhow!("manifesting jsonnet: {}", e))?;

	let json_value: serde_json::Value =
		serde_json::from_str(&json_str).context("parsing manifested JSON")?;

	// Extract environment metadata (names and exportJsonnetImplementation)
	let env_metadata = extract_environment_metadata(&json_value);

	if env_metadata.is_empty() {
		// No valid Tanka environments found - return empty list
		return Ok(vec![]);
	}

	// Get shared exportJsonnetImplementation (use first non-None value found)
	let shared_export_impl = env_metadata
		.iter()
		.find_map(|(_, impl_opt)| impl_opt.clone());

	if env_metadata.len() == 1 {
		// Single environment - no need to specify name
		return Ok(vec![DiscoveredEnv {
			is_static: false,
			path: path.to_path_buf(),
			env_name: None,
			export_jsonnet_implementation: shared_export_impl,
		}]);
	}

	// Multiple environments - create one DiscoveredEnv per sub-environment
	Ok(env_metadata
		.into_iter()
		.map(|(name, export_impl)| DiscoveredEnv {
			is_static: false,
			path: path.to_path_buf(),
			env_name: Some(name),
			// Use per-env export_impl if set, otherwise fall back to shared
			export_jsonnet_implementation: export_impl.or_else(|| shared_export_impl.clone()),
		})
		.collect())
}

/// Extract environment metadata (name, exportJsonnetImplementation) from a JSON value
/// Returns a list of (name, Option<exportJsonnetImplementation>) tuples
fn extract_environment_metadata(value: &serde_json::Value) -> Vec<(String, Option<String>)> {
	let mut metadata = Vec::new();

	match value {
		serde_json::Value::Object(obj) => {
			// Check if this is an Environment object
			if obj.get("kind").and_then(|v| v.as_str()) == Some("Environment") {
				if let Some(meta) = obj.get("metadata") {
					if let Some(name) = meta.get("name").and_then(|v| v.as_str()) {
						// Extract exportJsonnetImplementation from spec if present
						let export_impl = obj
							.get("spec")
							.and_then(|s| s.get("exportJsonnetImplementation"))
							.and_then(|v| v.as_str())
							.map(|s| s.to_string());
						metadata.push((name.to_string(), export_impl));
					}
				}
			}
			// Recurse into object values
			for v in obj.values() {
				metadata.extend(extract_environment_metadata(v));
			}
		}
		serde_json::Value::Array(arr) => {
			for v in arr {
				metadata.extend(extract_environment_metadata(v));
			}
		}
		_ => {}
	}

	metadata
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn test_find_single_environment() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		// Create a single static environment
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		fs::write(root.join("env/main.jsonnet"), "{}").unwrap();
		fs::write(
			root.join("env/spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let envs = find_environments(&[root.join("env").to_string_lossy().to_string()]).unwrap();
		assert_eq!(envs.len(), 1);
		assert!(envs[0].is_static);
		assert!(envs[0].env_name.is_none());
		assert!(envs[0].export_jsonnet_implementation.is_none());
	}

	#[test]
	fn test_find_static_environment() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		fs::write(root.join("env/main.jsonnet"), "{}").unwrap();
		fs::write(
			root.join("env/spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment"}"#,
		)
		.unwrap();

		let envs = find_environments(&[root.join("env").to_string_lossy().to_string()]).unwrap();
		assert_eq!(envs.len(), 1);
		assert!(envs[0].is_static);
		assert!(envs[0].env_name.is_none());
	}

	#[test]
	fn test_find_multiple_environments() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create multiple static environments
		for name in ["dev", "staging", "prod"] {
			fs::create_dir_all(root.join(format!("environments/{}", name))).unwrap();
			fs::write(
				root.join(format!("environments/{}/main.jsonnet", name)),
				"{}",
			)
			.unwrap();
			fs::write(
				root.join(format!("environments/{}/spec.json", name)),
				format!(
					r#"{{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{{"name":"{}"}},"spec":{{"namespace":"default"}}}}"#,
					name
				),
			)
			.unwrap();
		}

		let envs =
			find_environments(&[root.join("environments").to_string_lossy().to_string()]).unwrap();
		assert_eq!(envs.len(), 3);
		// All should have no env_name since they're separate directories
		for env in &envs {
			assert!(env.env_name.is_none());
		}
	}

	#[test]
	fn test_skip_vendor_directory() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create env in vendor (should be skipped)
		fs::create_dir_all(root.join("vendor/somelib")).unwrap();
		fs::write(root.join("vendor/somelib/main.jsonnet"), "{}").unwrap();
		fs::write(
			root.join("vendor/somelib/spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment"}"#,
		)
		.unwrap();

		// Create actual env at root level (not inside environments subdir)
		fs::write(root.join("main.jsonnet"), "{}").unwrap();
		fs::write(
			root.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"root"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let envs = find_environments(&[root.to_string_lossy().to_string()]).unwrap();
		assert_eq!(envs.len(), 1);
		// Root itself should be the environment
		assert_eq!(envs[0].path, root);
	}

	#[test]
	fn test_no_duplicate_environments() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();

		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		fs::write(root.join("env/main.jsonnet"), "{}").unwrap();
		fs::write(
			root.join("env/spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		// Pass the same path twice
		let envs = find_environments(&[
			root.join("env").to_string_lossy().to_string(),
			root.join("env").to_string_lossy().to_string(),
		])
		.unwrap();
		assert_eq!(envs.len(), 1);
	}
}
