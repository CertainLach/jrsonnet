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

use crate::eval::EvalOpts;

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
	/// Labels from the environment metadata (for selector filtering)
	pub labels: std::collections::HashMap<String, String>,
}

/// Find all Tanka environments in the given paths
///
/// This walks the directory tree looking for environments.
/// When an environment is found, its subdirectories are not searched.
///
/// The optional `eval_opts` parameter allows passing TLAs and external variables
/// that may be needed to evaluate main.jsonnet when it's a function.
#[allow(dead_code)]
pub fn find_environments(paths: &[String]) -> Result<Vec<DiscoveredEnv>> {
	find_environments_with_opts(paths, &EvalOpts::default())
}

/// Find all Tanka environments in the given paths with eval options
///
/// This variant allows passing TLAs and external variables needed for discovery.
pub fn find_environments_with_opts(
	paths: &[String],
	eval_opts: &EvalOpts,
) -> Result<Vec<DiscoveredEnv>> {
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
					// Static environment - read spec data from spec.json
					let spec_data = read_spec_data(&abs_path);
					envs.push(DiscoveredEnv {
						is_static: true,
						path: abs_path,
						env_name: None,
						export_jsonnet_implementation: spec_data.export_jsonnet_implementation,
						labels: spec_data.labels,
					});
				} else {
					// Inline environment(s) - discover sub-environments
					match discover_inline_environments(&abs_path, eval_opts) {
						Ok(inline_envs) => envs.extend(inline_envs),
						Err(_) => {
							// If discovery fails, add as single env with no name
							envs.push(DiscoveredEnv {
								is_static: false,
								path: abs_path,
								env_name: None,
								export_jsonnet_implementation: None,
								labels: std::collections::HashMap::new(),
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
						// Static environment - read spec data from spec.json
						let spec_data = read_spec_data(&canonical);
						envs.push(DiscoveredEnv {
							is_static: true,
							path: canonical,
							env_name: None,
							export_jsonnet_implementation: spec_data.export_jsonnet_implementation,
							labels: spec_data.labels,
						});
					} else {
						// Inline environment(s) - discover sub-environments
						match discover_inline_environments(&canonical, eval_opts) {
							Ok(inline_envs) => envs.extend(inline_envs),
							Err(_) => {
								// If discovery fails, add as single env with no name
								envs.push(DiscoveredEnv {
									is_static: false,
									path: canonical,
									env_name: None,
									export_jsonnet_implementation: None,
									labels: std::collections::HashMap::new(),
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

/// Data read from spec.json for static environments
struct SpecData {
	export_jsonnet_implementation: Option<String>,
	labels: std::collections::HashMap<String, String>,
}

/// Read spec data from spec.json if it exists
fn read_spec_data(path: &Path) -> SpecData {
	let spec_path = path.join("spec.json");
	if !spec_path.exists() {
		return SpecData {
			export_jsonnet_implementation: None,
			labels: std::collections::HashMap::new(),
		};
	}

	let content = match std::fs::read_to_string(&spec_path) {
		Ok(c) => c,
		Err(_) => {
			return SpecData {
				export_jsonnet_implementation: None,
				labels: std::collections::HashMap::new(),
			}
		}
	};

	let json: serde_json::Value = match serde_json::from_str(&content) {
		Ok(j) => j,
		Err(_) => {
			return SpecData {
				export_jsonnet_implementation: None,
				labels: std::collections::HashMap::new(),
			}
		}
	};

	let export_impl = json
		.get("spec")
		.and_then(|s| s.get("exportJsonnetImplementation"))
		.and_then(|v| v.as_str())
		.map(|s| s.to_string());

	let labels = json
		.get("metadata")
		.and_then(|m| m.get("labels"))
		.and_then(|l| l.as_object())
		.map(|obj| {
			obj.iter()
				.filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
				.collect()
		})
		.unwrap_or_default();

	SpecData {
		export_jsonnet_implementation: export_impl,
		labels,
	}
}

/// Discover inline environments within a main.jsonnet file
/// Returns a list of DiscoveredEnv, one for each sub-environment found
fn discover_inline_environments(path: &Path, eval_opts: &EvalOpts) -> Result<Vec<DiscoveredEnv>> {
	use jrsonnet_evaluator::{manifest::JsonFormat, State, Val};
	use jrsonnet_stdlib::ContextInitializer;

	let main_path = path.join("main.jsonnet");
	if !main_path.exists() {
		return Ok(vec![DiscoveredEnv {
			is_static: false,
			path: path.to_path_buf(),
			env_name: None,
			export_jsonnet_implementation: None,
			labels: std::collections::HashMap::new(),
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

	// Add external variables
	for (key, value) in &eval_opts.ext_str {
		ctx_init.add_ext_var(key.as_str().into(), Val::Str(value.as_str().into()));
	}
	for (key, code) in &eval_opts.ext_code {
		let _ = ctx_init.add_ext_code(key.as_str().into(), code.as_str());
	}

	builder.context_initializer(ctx_init);

	let state = builder.build();

	// Check if we need TLAs - if so, we have to fully evaluate main.jsonnet first
	let has_tlas = !eval_opts.tla_str.is_empty() || !eval_opts.tla_code.is_empty();

	// Build the evaluation script based on whether TLAs are needed
	let eval_script = if has_tlas {
		// Slow path: TLAs require full evaluation first, then manifest, then metadata extraction
		// This is because main.jsonnet might be a function that needs TLA arguments
		let tla_args = build_tla_args(eval_opts)?;

		let main_result = state
			.import(main_path.to_string_lossy().as_ref())
			.map_err(|e| anyhow::anyhow!("importing main.jsonnet for discovery: {}", e))?;

		let main_value = jrsonnet_evaluator::apply_tla(state.clone(), &tla_args, main_result)
			.map_err(|e| anyhow::anyhow!("applying TLAs for discovery: {}", e))?;

		let main_json = main_value
			.manifest(JsonFormat::cli(2))
			.map_err(|e| anyhow::anyhow!("manifesting main.jsonnet for discovery: {}", e))?;

		format!("local main = {};\n{}", main_json, METADATA_EVAL_SCRIPT)
	} else {
		// Fast path: use lazy evaluation by embedding the import directly in the script
		// This allows the evaluator to only extract what's needed for metadata
		format!(
			"local main = (import '{}');\n{}",
			main_path.file_name().unwrap().to_string_lossy(),
			METADATA_EVAL_SCRIPT
		)
	};

	let result = state
		.evaluate_snippet("<metadata-eval>", &eval_script)
		.map_err(|e| anyhow::anyhow!("evaluating metadata extraction: {}", e))?;

	let json_str = result
		.manifest(JsonFormat::cli(2))
		.map_err(|e| anyhow::anyhow!("manifesting jsonnet: {}", e))?;

	let json_value: serde_json::Value =
		serde_json::from_str(&json_str).context("parsing manifested JSON")?;

	// Extract environment metadata (names, exportJsonnetImplementation, labels)
	let env_metadata = extract_environment_metadata(&json_value);

	if env_metadata.is_empty() {
		// No valid Tanka environments found - return empty list
		return Ok(vec![]);
	}

	// Get shared exportJsonnetImplementation (use first non-None value found)
	let shared_export_impl = env_metadata
		.iter()
		.find_map(|m| m.export_jsonnet_implementation.clone());

	if env_metadata.len() == 1 {
		// Single environment - no need to specify name
		let meta = &env_metadata[0];
		return Ok(vec![DiscoveredEnv {
			is_static: false,
			path: path.to_path_buf(),
			env_name: None,
			export_jsonnet_implementation: shared_export_impl,
			labels: meta.labels.clone(),
		}]);
	}

	// Multiple environments - create one DiscoveredEnv per sub-environment
	Ok(env_metadata
		.into_iter()
		.map(|meta| DiscoveredEnv {
			is_static: false,
			path: path.to_path_buf(),
			env_name: Some(meta.name),
			// Use per-env export_impl if set, otherwise fall back to shared
			export_jsonnet_implementation: meta
				.export_jsonnet_implementation
				.or_else(|| shared_export_impl.clone()),
			labels: meta.labels,
		})
		.collect())
}

/// Build TLA arguments from eval options
fn build_tla_args(
	eval_opts: &EvalOpts,
) -> Result<
	jrsonnet_evaluator::gc::GcHashMap<
		jrsonnet_evaluator::IStr,
		jrsonnet_evaluator::function::TlaArg,
	>,
> {
	use jrsonnet_evaluator::{function::TlaArg, gc::GcHashMap, IStr};

	let mut tla_args: GcHashMap<IStr, TlaArg> = GcHashMap::new();

	// Add string TLAs
	for (key, value) in &eval_opts.tla_str {
		tla_args.insert(key.as_str().into(), TlaArg::String(value.as_str().into()));
	}

	// Add code TLAs (need to parse as jsonnet)
	for (key, value) in &eval_opts.tla_code {
		let source = jrsonnet_parser::Source::new_virtual(
			format!("<tla:{}>", key).into(),
			value.as_str().into(),
		);
		let parsed = jrsonnet_parser::parse(
			value,
			&jrsonnet_parser::ParserSettings {
				source: source.clone(),
			},
		)
		.map_err(|e| anyhow::anyhow!("failed to parse TLA code '{}':\n{}", key, e))?;

		tla_args.insert(key.as_str().into(), TlaArg::Code(parsed));
	}

	Ok(tla_args)
}

/// Metadata extracted from an inline environment
struct EnvMetadata {
	name: String,
	export_jsonnet_implementation: Option<String>,
	labels: std::collections::HashMap<String, String>,
}

/// Extract environment metadata (name, exportJsonnetImplementation, labels) from a JSON value
fn extract_environment_metadata(value: &serde_json::Value) -> Vec<EnvMetadata> {
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

						// Extract labels from metadata
						let labels = meta
							.get("labels")
							.and_then(|l| l.as_object())
							.map(|obj| {
								obj.iter()
									.filter_map(|(k, v)| {
										v.as_str().map(|s| (k.clone(), s.to_string()))
									})
									.collect()
							})
							.unwrap_or_default();

						metadata.push(EnvMetadata {
							name: name.to_string(),
							export_jsonnet_implementation: export_impl,
							labels,
						});
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
