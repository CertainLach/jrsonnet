use std::{
	fs,
	io::{BufWriter, Write},
	path::{Path, PathBuf},
	sync::Mutex,
	time::{Duration, Instant},
};

use anyhow::{Context, Result};
use rayon::prelude::*;
use tabwriter::TabWriter;

use crate::jpath;
use crate::spec::{Environment, Spec};

/// Options shared between env add and env set for building spec.
/// For env set, only `Some`/non-empty values are applied.
pub struct EnvSpecOptions {
	pub namespace: Option<String>,
	pub server: Option<String>,
	pub server_from_context: Option<String>,
	pub context_name: Vec<String>,
	pub diff_strategy: Option<String>,
	/// If None, do not update (env set). If Some(b), set spec.inject_labels (env add always sets).
	pub inject_labels: Option<bool>,
}

/// Resolve API server URL from a kubeconfig context name (for --server-from-context).
fn server_from_kubeconfig_context(context_name: &str) -> Result<String> {
	let kubeconfig =
		kube::config::Kubeconfig::read().context("reading KUBECONFIG for --server-from-context")?;
	let ctx = kubeconfig
		.contexts
		.iter()
		.find(|c| c.name == context_name)
		.with_context(|| format!("context {:?} not found in KUBECONFIG", context_name))?;
	let cluster_name = ctx
		.context
		.as_ref()
		.and_then(|c| Some(c.cluster.as_str()))
		.with_context(|| format!("context {:?} has no cluster", context_name))?;
	let cluster = kubeconfig
		.clusters
		.iter()
		.find(|c| c.name == cluster_name)
		.with_context(|| format!("cluster {:?} not found in KUBECONFIG", cluster_name))?;
	cluster
		.cluster
		.as_ref()
		.and_then(|c| c.server.clone())
		.with_context(|| format!("cluster {:?} has no server", cluster_name))
}

fn apply_spec_options(spec: &mut Spec, opts: &EnvSpecOptions) -> Result<()> {
	if let Some(ns) = &opts.namespace {
		spec.namespace = ns.clone();
	}
	if let Some(s) = &opts.server {
		spec.api_server = Some(s.clone());
	}
	if let Some(ctx) = &opts.server_from_context {
		let server = server_from_kubeconfig_context(ctx)?;
		spec.api_server = Some(server);
	}
	if !opts.context_name.is_empty() {
		spec.context_names = Some(opts.context_name.clone());
	}
	if let Some(d) = &opts.diff_strategy {
		spec.diff_strategy = Some(d.clone());
	}
	if let Some(b) = opts.inject_labels {
		spec.inject_labels = Some(b);
	}
	Ok(())
}

/// Add a new environment: create directory, main.jsonnet, and optionally spec.json.
pub fn env_add(path: &str, inline: bool, opts: &EnvSpecOptions) -> Result<()> {
	let path_buf = PathBuf::from(path);
	let (root, env_dir) = if path_buf.is_absolute() {
		let env_dir = if path_buf.exists() {
			path_buf.canonicalize().unwrap_or(path_buf)
		} else {
			path_buf
		};
		let root = jpath::find_root(env_dir.parent().unwrap_or(&env_dir))
			.context("could not find project root (no jsonnetfile.json or tkrc.yaml)")?;
		(root, env_dir)
	} else {
		let cwd = std::env::current_dir().context("current_dir")?;
		let root = jpath::find_root(&cwd)
			.context("could not find project root (no jsonnetfile.json or tkrc.yaml)")?;
		let env_dir = root.join(path);
		let env_dir = if env_dir.exists() {
			env_dir.canonicalize().unwrap_or(env_dir)
		} else {
			env_dir
		};
		(root, env_dir)
	};

	if env_dir.exists() {
		let has_marker =
			env_dir.join("main.jsonnet").exists() || env_dir.join("spec.json").exists();
		if has_marker {
			anyhow::bail!("environment already exists at {}", env_dir.display());
		}
	}
	fs::create_dir_all(&env_dir).context("create environment directory")?;

	let rel_path = env_dir
		.strip_prefix(&root)
		.map(|p| p.to_string_lossy().to_string())
		.unwrap_or_else(|_| env_dir.display().to_string());

	if inline {
		let namespace = opts.namespace.as_deref().unwrap_or("default");
		let server: String = opts
			.server
			.clone()
			.or_else(|| {
				opts.server_from_context
					.as_ref()
					.and_then(|c| server_from_kubeconfig_context(c).ok())
			})
			.unwrap_or_else(|| "https://localhost:6443".to_string());
		let name = rel_path.replace('/', "-");
		let main_content = format!(
			r#"{{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {{ name: '{}' }},
  spec: {{ namespace: '{}', apiServer: '{}' }},
  data: {{}},
}}"#,
			name,
			namespace,
			server.as_str()
		);
		fs::write(env_dir.join("main.jsonnet"), main_content).context("write main.jsonnet")?;
		return Ok(());
	}

	// Static environment: main.jsonnet + spec.json
	fs::write(env_dir.join("main.jsonnet"), "{}").context("write main.jsonnet")?;

	let mut env = Environment::new();
	env.metadata.name = Some(rel_path.clone());
	env.metadata.namespace = Some(format!("{}/main.jsonnet", rel_path));
	apply_spec_options(&mut env.spec, opts)?;

	let spec_json = serde_json::to_string_pretty(&env).context("serialize spec.json")?;
	fs::write(env_dir.join("spec.json"), spec_json).context("write spec.json")?;
	Ok(())
}

/// Remove environment(s) by path. Each path is resolved to an environment directory and removed.
pub fn env_remove(paths: &[String]) -> Result<()> {
	for path in paths {
		let base = crate::jpath::resolve(path)
			.map(|r| r.base)
			.with_context(|| {
				format!(
					"could not resolve environment at {} (not an environment or not found)",
					path
				)
			})?;
		if base.join("spec.json").exists() || base.join("main.jsonnet").exists() {
			fs::remove_dir_all(&base).with_context(|| format!("remove {}", base.display()))?;
		} else {
			anyhow::bail!(
				"not an environment directory (no spec.json or main.jsonnet): {}",
				base.display()
			);
		}
	}
	Ok(())
}

/// Update an existing environment's spec.json with the given options.
pub fn env_set(path: &str, opts: &EnvSpecOptions) -> Result<()> {
	let jpath_result = crate::jpath::resolve(path).context("resolve environment path")?;
	let spec_path = jpath_result.base.join("spec.json");
	if !spec_path.exists() {
		anyhow::bail!(
			"environment at {} has no spec.json (inline environment); use env add to create a static environment",
			jpath_result.base.display()
		);
	}
	let content = fs::read_to_string(&spec_path).context("read spec.json")?;
	let mut env: Environment = serde_json::from_str(&content).context("parse spec.json")?;
	apply_spec_options(&mut env.spec, opts)?;
	let spec_json = serde_json::to_string_pretty(&env).context("serialize spec.json")?;
	fs::write(&spec_path, spec_json).context("write spec.json")?;
	Ok(())
}

/// Recursively prune empty objects from a JSON value (mutates in place)
fn prune_empty_objects(value: &mut serde_json::Value) {
	match value {
		serde_json::Value::Object(map) => {
			// First, recursively prune nested objects
			for (_key, val) in map.iter_mut() {
				prune_empty_objects(val);
			}

			// Then remove keys with empty object values
			map.retain(
				|_key, val| !matches!(val, serde_json::Value::Object(obj) if obj.is_empty()),
			);
		}
		serde_json::Value::Array(arr) => {
			// Recursively prune objects in arrays
			for val in arr.iter_mut() {
				prune_empty_objects(val);
			}
		}
		_ => {}
	}
}

/// List environments in the given path, writing to the provided writer.
pub fn list_envs_to_writer<W: Write>(path: Option<String>, json: bool, writer: W) -> Result<()> {
	let search_path = path
		.map(PathBuf::from)
		.unwrap_or_else(|| std::env::current_dir().unwrap());
	let mut envs = find_environments(&search_path)?;

	if json {
		// Normalize: convert null resourceDefaults and expectVersions to empty objects,
		// and prune empty nested objects to match Tanka's behavior
		for env in &mut envs {
			env.spec
				.resource_defaults
				.get_or_insert_with(|| serde_json::json!({}));
			env.spec
				.expect_versions
				.get_or_insert_with(|| serde_json::json!({}));

			// Prune empty objects from resourceDefaults and expectVersions
			if let Some(ref mut rd) = env.spec.resource_defaults {
				prune_empty_objects(rd);
			}
			if let Some(ref mut ev) = env.spec.expect_versions {
				prune_empty_objects(ev);
			}
		}
		writeln!(
			&mut BufWriter::new(writer),
			"{}",
			serde_json::to_string(&envs)?
		)?;
	} else {
		print_table_to_writer(&envs, &search_path, writer)?;
	}

	Ok(())
}

fn print_table_to_writer<W: Write>(
	envs: &[Environment],
	search_path: &Path,
	writer: W,
) -> Result<()> {
	let mut tw = TabWriter::new(writer).padding(4);
	writeln!(tw, "NAME\tNAMESPACE\tSERVER")?;

	if envs.is_empty() {
		writeln!(tw, "No environments found in {}", search_path.display())?;
	} else {
		for env in envs {
			writeln!(
				tw,
				"{}\t{}\t{}",
				env.metadata.name.as_deref().unwrap_or("unnamed"),
				&env.spec.namespace,
				env.spec.api_server.as_deref().unwrap_or("-")
			)?;
		}
	}
	tw.flush()?;
	Ok(())
}

/// Find all environments recursively
fn find_environments(root: &Path) -> Result<Vec<Environment>> {
	// Handle the case where a main.jsonnet file is passed directly
	let main_files = if root.is_file()
		&& root
			.file_name()
			.map(|f| f == "main.jsonnet")
			.unwrap_or(false)
	{
		vec![root.to_path_buf()]
	} else {
		find_main_jsonnet_files(root)?
	};
	let profile = std::env::var("RTK_PROFILE").is_ok();

	// Track timing for each file if profiling is enabled
	let timings: Mutex<Vec<(PathBuf, Duration)>> = Mutex::new(Vec::new());

	// Process all files in parallel - Rayon handles work-stealing automatically
	let all_envs: Vec<Vec<Environment>> = main_files
		.par_iter()
		.filter_map(|main_file| {
			let start = Instant::now();
			let dir = main_file.parent()?;
			let spec_file = dir.join("spec.json");

			let result = if spec_file.exists() {
				// Static environment
				if let Ok(mut env) = load_static_env(dir) {
					if set_env_metadata(&mut env, dir).is_ok() {
						Some(vec![env])
					} else {
						None
					}
				} else {
					None
				}
			} else {
				// Inline environment
				match load_inline_envs(dir) {
					Ok(mut envs) => {
						for env in &mut envs {
							// For inline envs, preserve the name from jsonnet if it exists
							// Only generate name from path if metadata.name is missing
							if env.metadata.name.is_none() {
								let _ = set_env_metadata(env, dir);
							} else {
								// Name exists from jsonnet, just set the namespace relative to project root
								let _ = set_env_namespace(env, dir);
							}
						}
						Some(envs)
					}
					Err(_) => None,
				}
			};

			// Record timing if profiling
			if profile {
				let elapsed = start.elapsed();
				timings.lock().unwrap().push((dir.to_path_buf(), elapsed));
			}

			result
		})
		.collect();

	// Print slowest files if profiling
	if profile {
		let mut timing_vec = timings.into_inner().unwrap();
		timing_vec.sort_by(|a, b| b.1.cmp(&a.1)); // Sort descending by duration

		eprintln!("\n=== 20 Slowest Environment Files ===");
		for (path, duration) in timing_vec.iter().take(20) {
			eprintln!(
				"{:>8.2}ms  {}",
				duration.as_secs_f64() * 1000.0,
				path.display()
			);
		}
		eprintln!();
	}

	// Flatten results
	let mut final_envs: Vec<Environment> = all_envs.into_iter().flatten().collect();

	// Sort by name
	final_envs.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

	Ok(final_envs)
}

/// Set environment metadata (name and namespace) relative to the project root
fn set_env_metadata(env: &mut Environment, dir: &Path) -> Result<()> {
	let project_root = find_project_root(dir).unwrap_or_else(|| dir.to_path_buf());
	let env_path = compute_relative_env_path(dir, &project_root);
	env.metadata.name = Some(env_path.clone());
	env.metadata.namespace = Some(format!("{}/main.jsonnet", env_path));
	Ok(())
}

/// Set only the namespace without changing the name, relative to the project root
fn set_env_namespace(env: &mut Environment, dir: &Path) -> Result<()> {
	let project_root = find_project_root(dir).unwrap_or_else(|| dir.to_path_buf());
	let relative_path = compute_relative_env_path(dir, &project_root);
	env.metadata.namespace = Some(format!("{}/main.jsonnet", relative_path));
	Ok(())
}

/// Compute a relative path for environment metadata
fn compute_relative_env_path(dir: &Path, original_path: &Path) -> String {
	// First try: relative to original_path
	if let Ok(rel) = dir.strip_prefix(original_path) {
		let rel_str = rel.to_string_lossy().to_string();
		if !rel_str.is_empty() {
			return rel_str;
		}
	}

	// If dir equals original_path, use the directory name
	if dir == original_path {
		if let Some(name) = dir.file_name() {
			return name.to_string_lossy().to_string();
		}
	}

	// Fallback: try to extract from "environments/" path pattern
	let dir_str = dir.to_string_lossy();
	if let Some(pos) = dir_str.find("environments/") {
		return dir_str[pos..].to_string();
	}
	if let Some(stripped) = dir_str.strip_prefix("ksonnet/") {
		return stripped.to_string();
	}

	// Last resort: just use the directory name
	dir.file_name()
		.map(|n| n.to_string_lossy().to_string())
		.unwrap_or_else(|| dir_str.to_string())
}

/// Recursively find all main.jsonnet files
fn find_main_jsonnet_files(dir: &Path) -> Result<Vec<PathBuf>> {
	let mut results = Vec::new();
	find_main_jsonnet_impl(dir, &mut results)?;
	Ok(results)
}

fn find_main_jsonnet_impl(dir: &Path, results: &mut Vec<PathBuf>) -> Result<()> {
	if !dir.is_dir() {
		return Ok(());
	}

	let main_file = dir.join("main.jsonnet");
	if main_file.exists() {
		results.push(main_file);
		return Ok(()); // Don't recurse into subdirectories
	}

	for entry in fs::read_dir(dir)? {
		let path = entry?.path();
		if path.is_dir() {
			find_main_jsonnet_impl(&path, results)?;
		}
	}

	Ok(())
}

/// Load inline environments by evaluating Jsonnet
fn load_inline_envs(dir: &Path) -> Result<Vec<Environment>> {
	use jrsonnet_evaluator::{manifest::JsonFormat, State};

	let main_path = dir.join("main.jsonnet");
	if !main_path.exists() {
		return Ok(Vec::new());
	}

	let mut import_paths = vec![dir.to_path_buf()];

	// Add lib and vendor directories if they exist
	if let Some(root) = find_project_root(dir) {
		for subdir in &["lib", "vendor"] {
			let path = root.join(subdir);
			if path.is_dir() {
				import_paths.push(path);
			}
		}
	}

	let import_resolver = jrsonnet_evaluator::FileImportResolver::new(import_paths);
	let mut builder = State::builder();
	builder.import_resolver(import_resolver);

	use jrsonnet_evaluator::trace::PathResolver;
	// Use Absolute resolver so std.thisFile returns absolute paths (like tk does)
	let ctx_init = jrsonnet_stdlib::ContextInitializer::new(PathResolver::Absolute);
	builder.context_initializer(ctx_init);

	let state = builder.build();
	let _state_guard = state.enter();

	// Evaluate with noDataEnv wrapper to strip out .data field
	let eval_script = format!(
		r#"
local noDataEnv(object) =
  std.prune(
    if std.isObject(object)
    then
      if std.objectHas(object, 'apiVersion') && std.objectHas(object, 'kind')
      then
        if object.kind == 'Environment'
        then object {{ data+:: {{}} }}
        else {{}}
      else
        std.mapWithKey(function(key, obj) noDataEnv(obj), object)
    else if std.isArray(object)
    then
      std.map(function(obj) noDataEnv(obj), object)
    else {{}}
  );

local main = (import '{}');
noDataEnv(main)
"#,
		main_path.file_name().unwrap().to_string_lossy()
	);

	let result = state
		.evaluate_snippet("<metadata-eval>", &eval_script)
		.map_err(|e| {
			anyhow::anyhow!(
				"Failed to evaluate Jsonnet at {}: {}",
				main_path.display(),
				e
			)
		})?;

	let json_str = result
		.manifest(JsonFormat::cli(2))
		.map_err(|e| anyhow::anyhow!("Failed to manifest Jsonnet: {}", e))?;

	let json_value: serde_json::Value =
		serde_json::from_str(&json_str).context("Failed to parse manifested JSON")?;

	let environments = extract_environments(&json_value)?;

	Ok(environments)
}

/// Extract Environment objects from Jsonnet output
fn extract_environments(value: &serde_json::Value) -> Result<Vec<Environment>> {
	let mut environments = Vec::new();

	match value {
		serde_json::Value::Object(obj) => {
			// Check if this is a single Environment object
			if obj.contains_key("apiVersion") && obj.contains_key("kind") {
				if let Some("Environment") = obj.get("kind").and_then(|v| v.as_str()) {
					if let Ok(env) = serde_json::from_value::<Environment>(value.clone()) {
						environments.push(env);
						return Ok(environments);
					}
				}
			}

			// Otherwise, recursively extract from each field
			for (key, val) in obj {
				let mut extracted = extract_environments(val)?;
				for env in &mut extracted {
					if env.metadata.name.is_none() {
						env.metadata.name = Some(key.clone());
					}
				}
				environments.extend(extracted);
			}
		}
		serde_json::Value::Array(arr) => {
			for val in arr {
				environments.extend(extract_environments(val)?);
			}
		}
		_ => {}
	}

	Ok(environments)
}

/// Find the project root by looking for the outermost jsonnetfile.json or tkrc.yaml.
/// Environments may have their own jsonnetfile.json for vendoring, so we walk all the
/// way up and return the outermost match.
fn find_project_root(start_path: &Path) -> Option<PathBuf> {
	let mut current = start_path;
	let mut root = None;
	loop {
		if current.join("jsonnetfile.json").exists() || current.join("tkrc.yaml").exists() {
			root = Some(current.to_path_buf());
		}
		match current.parent() {
			Some(parent) if parent != current => current = parent,
			_ => break,
		}
	}
	root
}

/// Load a static environment from spec.json
fn load_static_env(path: &Path) -> Result<Environment> {
	let spec_path = path.join("spec.json");
	let content = fs::read_to_string(&spec_path)
		.with_context(|| format!("Failed to read {}", spec_path.display()))?;
	serde_json::from_str(&content)
		.with_context(|| format!("Failed to parse {}", spec_path.display()))
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use tempfile::TempDir;

	use super::*;

	fn create_inline_env_fixture(dir: &Path) {
		// Create jsonnetfile.json at root (required for project root detection)
		fs::write(
			dir.join("jsonnetfile.json"),
			r#"{"version": 1, "dependencies": [], "legacyImports": true}"#,
		)
		.unwrap();

		// Create inline environment directory
		let env_dir = dir.join("my-env");
		fs::create_dir_all(&env_dir).unwrap();

		// Create main.jsonnet with inline environment
		fs::write(
			env_dir.join("main.jsonnet"),
			r#"{
  env1: {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: { name: 'test-env-1' },
    spec: { namespace: 'default', apiServer: 'https://localhost:6443' },
    data: {},
  },
  env2: {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: { name: 'test-env-2' },
    spec: { namespace: 'other', apiServer: 'https://localhost:6443' },
    data: {},
  },
}"#,
		)
		.unwrap();
	}

	#[test]
	fn test_list_envs_with_directory_path() {
		let temp_dir = TempDir::new().unwrap();
		create_inline_env_fixture(temp_dir.path());

		let env_dir = temp_dir.path().join("my-env");
		let mut output = Cursor::new(Vec::new());

		list_envs_to_writer(
			Some(env_dir.to_string_lossy().to_string()),
			true,
			&mut output,
		)
		.unwrap();

		let output_str = String::from_utf8(output.into_inner()).unwrap();
		let envs: Vec<serde_json::Value> = serde_json::from_str(&output_str).unwrap();

		assert_eq!(envs.len(), 2, "Should find 2 environments");
	}

	#[test]
	fn test_list_envs_with_main_jsonnet_file_path() {
		let temp_dir = TempDir::new().unwrap();
		create_inline_env_fixture(temp_dir.path());

		// Pass the main.jsonnet file path directly instead of the directory
		let file_path = temp_dir.path().join("my-env").join("main.jsonnet");
		let mut output = Cursor::new(Vec::new());

		list_envs_to_writer(
			Some(file_path.to_string_lossy().to_string()),
			true,
			&mut output,
		)
		.unwrap();

		let output_str = String::from_utf8(output.into_inner()).unwrap();
		let envs: Vec<serde_json::Value> = serde_json::from_str(&output_str).unwrap();

		assert_eq!(
			envs.len(),
			2,
			"Should find 2 environments when passing main.jsonnet file path"
		);
	}

	#[test]
	fn test_list_envs_directory_and_file_path_produce_same_count() {
		let temp_dir = TempDir::new().unwrap();
		create_inline_env_fixture(temp_dir.path());

		let env_dir = temp_dir.path().join("my-env");
		let file_path = env_dir.join("main.jsonnet");

		// List with directory path
		let mut dir_output = Cursor::new(Vec::new());
		list_envs_to_writer(
			Some(env_dir.to_string_lossy().to_string()),
			true,
			&mut dir_output,
		)
		.unwrap();
		let dir_envs: Vec<serde_json::Value> =
			serde_json::from_str(&String::from_utf8(dir_output.into_inner()).unwrap()).unwrap();

		// List with file path
		let mut file_output = Cursor::new(Vec::new());
		list_envs_to_writer(
			Some(file_path.to_string_lossy().to_string()),
			true,
			&mut file_output,
		)
		.unwrap();
		let file_envs: Vec<serde_json::Value> =
			serde_json::from_str(&String::from_utf8(file_output.into_inner()).unwrap()).unwrap();

		assert_eq!(
			dir_envs.len(),
			file_envs.len(),
			"Directory path and file path should return the same number of environments"
		);
	}

	/// Helper to extract metadata.namespace values from env list --json output
	fn list_env_namespaces(path: Option<String>) -> Vec<String> {
		let mut output = Cursor::new(Vec::new());
		list_envs_to_writer(path, true, &mut output).unwrap();
		let output_str = String::from_utf8(output.into_inner()).unwrap();
		let envs: Vec<serde_json::Value> = serde_json::from_str(&output_str).unwrap();
		let mut namespaces: Vec<String> = envs
			.iter()
			.filter_map(|env| env["metadata"]["namespace"].as_str().map(|s| s.to_string()))
			.collect();
		namespaces.sort();
		namespaces
	}

	/// Create an inline env fixture nested under a "project" subdirectory.
	/// Returns the project root path.
	fn create_nested_inline_env_fixture(base: &Path) -> PathBuf {
		let project_root = base.join("project");
		fs::create_dir_all(&project_root).unwrap();
		create_inline_env_fixture(&project_root);
		project_root
	}

	fn create_static_env_fixture(dir: &Path) {
		// Create jsonnetfile.json at root (required for project root detection)
		fs::write(
			dir.join("jsonnetfile.json"),
			r#"{"version": 1, "dependencies": [], "legacyImports": true}"#,
		)
		.unwrap();

		// Create static environment directory
		let env_dir = dir.join("environments").join("dev");
		fs::create_dir_all(&env_dir).unwrap();

		fs::write(env_dir.join("main.jsonnet"), "{}").unwrap();

		fs::write(
			env_dir.join("spec.json"),
			r#"{
  "apiVersion": "tanka.dev/v1alpha1",
  "kind": "Environment",
  "metadata": {},
  "spec": {
    "apiServer": "https://localhost:6443",
    "namespace": "default"
  }
}"#,
		)
		.unwrap();
	}

	/// Create a static env fixture nested under a "project" subdirectory.
	/// Returns the project root path.
	fn create_nested_static_env_fixture(base: &Path) -> PathBuf {
		let project_root = base.join("project");
		fs::create_dir_all(&project_root).unwrap();
		create_static_env_fixture(&project_root);
		project_root
	}

	/// metadata.namespace should always be relative to the project root
	/// (where jsonnetfile.json lives), regardless of what path is passed.
	#[test]
	fn test_list_envs_json_inline_namespace_stable_from_root_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_inline_env_fixture(temp_dir.path());

		let expected_namespace = "my-env/main.jsonnet";
		let ns = list_env_namespaces(Some(project_root.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 2);
		assert!(
			ns.iter().all(|n| n == expected_namespace),
			"From root dir: expected all namespaces to be '{}', got {:?}",
			expected_namespace,
			ns
		);
	}

	#[test]
	fn test_list_envs_json_inline_namespace_stable_from_parent_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_inline_env_fixture(temp_dir.path());

		let expected_namespace = "my-env/main.jsonnet";
		let parent = project_root.parent().unwrap();
		let ns = list_env_namespaces(Some(parent.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 2);
		assert!(
			ns.iter().all(|n| n == expected_namespace),
			"From parent dir: expected all namespaces to be '{}', got {:?}",
			expected_namespace,
			ns
		);
	}

	#[test]
	fn test_list_envs_json_inline_namespace_stable_from_child_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_inline_env_fixture(temp_dir.path());

		let expected_namespace = "my-env/main.jsonnet";
		let child = project_root.join("my-env");
		let ns = list_env_namespaces(Some(child.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 2);
		assert!(
			ns.iter().all(|n| n == expected_namespace),
			"From child dir: expected all namespaces to be '{}', got {:?}",
			expected_namespace,
			ns
		);
	}

	#[test]
	fn test_list_envs_json_inline_namespace_stable_from_file_path() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_inline_env_fixture(temp_dir.path());

		let expected_namespace = "my-env/main.jsonnet";
		let file_path = project_root.join("my-env").join("main.jsonnet");
		let ns = list_env_namespaces(Some(file_path.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 2);
		assert!(
			ns.iter().all(|n| n == expected_namespace),
			"From file path: expected all namespaces to be '{}', got {:?}",
			expected_namespace,
			ns
		);
	}

	/// Same tests for static environments (spec.json based)
	#[test]
	fn test_list_envs_json_static_namespace_stable_from_root_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_static_env_fixture(temp_dir.path());

		let expected_namespace = "environments/dev/main.jsonnet";
		let ns = list_env_namespaces(Some(project_root.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 1);
		assert_eq!(
			ns[0], expected_namespace,
			"From root dir: expected namespace '{}'",
			expected_namespace
		);
	}

	#[test]
	fn test_list_envs_json_static_namespace_stable_from_parent_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_static_env_fixture(temp_dir.path());

		let expected_namespace = "environments/dev/main.jsonnet";
		let parent = project_root.parent().unwrap();
		let ns = list_env_namespaces(Some(parent.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 1);
		assert_eq!(
			ns[0], expected_namespace,
			"From parent dir: expected namespace '{}'",
			expected_namespace
		);
	}

	#[test]
	fn test_list_envs_json_static_namespace_stable_from_child_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_static_env_fixture(temp_dir.path());

		let expected_namespace = "environments/dev/main.jsonnet";
		let child = project_root.join("environments");
		let ns = list_env_namespaces(Some(child.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 1);
		assert_eq!(
			ns[0], expected_namespace,
			"From child dir: expected namespace '{}'",
			expected_namespace
		);
	}

	#[test]
	fn test_list_envs_json_static_namespace_stable_from_env_dir() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = create_nested_static_env_fixture(temp_dir.path());

		let expected_namespace = "environments/dev/main.jsonnet";
		let env_dir = project_root.join("environments").join("dev");
		let ns = list_env_namespaces(Some(env_dir.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 1);
		assert_eq!(
			ns[0], expected_namespace,
			"From env dir: expected namespace '{}'",
			expected_namespace
		);
	}

	/// Env directories can have their own jsonnetfile.json for vendoring.
	/// find_project_root must walk past these to the outermost project root.
	fn create_static_env_with_nested_jsonnetfile(dir: &Path) {
		// Create jsonnetfile.json at the real project root
		fs::write(
			dir.join("jsonnetfile.json"),
			r#"{"version": 1, "dependencies": [], "legacyImports": true}"#,
		)
		.unwrap();

		// Create static environment directory
		let env_dir = dir.join("environments").join("ge-logs").join("dev.ge-logs");
		fs::create_dir_all(&env_dir).unwrap();

		fs::write(env_dir.join("main.jsonnet"), "{}").unwrap();

		fs::write(
			env_dir.join("spec.json"),
			r#"{
  "apiVersion": "tanka.dev/v1alpha1",
  "kind": "Environment",
  "metadata": {},
  "spec": {
    "apiServer": "https://localhost:6443",
    "namespace": "ge-logs"
  }
}"#,
		)
		.unwrap();

		// The env dir also has its own jsonnetfile.json (for vendoring)
		fs::write(
			env_dir.join("jsonnetfile.json"),
			r#"{"version": 1, "dependencies": [], "legacyImports": true}"#,
		)
		.unwrap();
	}

	#[test]
	fn test_list_envs_json_static_namespace_with_nested_jsonnetfile() {
		let temp_dir = TempDir::new().unwrap();
		let project_root = temp_dir.path().join("project");
		fs::create_dir_all(&project_root).unwrap();
		create_static_env_with_nested_jsonnetfile(&project_root);

		// The namespace should be relative to the outermost project root,
		// not the env's own jsonnetfile.json
		let expected_namespace = "environments/ge-logs/dev.ge-logs/main.jsonnet";

		// From root
		let ns = list_env_namespaces(Some(project_root.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 1);
		assert_eq!(
			ns[0], expected_namespace,
			"Nested jsonnetfile.json should not affect namespace; got '{}'",
			ns[0]
		);

		// From environments/ subdirectory
		let child = project_root.join("environments");
		let ns = list_env_namespaces(Some(child.to_string_lossy().to_string()));
		assert_eq!(ns.len(), 1);
		assert_eq!(
			ns[0], expected_namespace,
			"Nested jsonnetfile.json from child dir; got '{}'",
			ns[0]
		);
	}

	// -----------------------------------------------------------------------
	// env add / remove / set tests (mirror Tanka behavior)
	// -----------------------------------------------------------------------

	fn create_project_root(dir: &Path) {
		fs::write(
			dir.join("jsonnetfile.json"),
			r#"{"version": 1, "dependencies": [], "legacyImports": true}"#,
		)
		.unwrap();
	}

	#[test]
	fn test_env_add_creates_static_environment() {
		let temp_dir = TempDir::new().unwrap();
		let root = temp_dir.path();
		create_project_root(root);
		let env_path = root.join("environments/dev");

		let opts = EnvSpecOptions {
			namespace: Some("my-namespace".to_string()),
			server: Some("https://kube.example.com".to_string()),
			server_from_context: None,
			context_name: vec![],
			diff_strategy: None,
			inject_labels: Some(true),
		};
		env_add(env_path.to_str().unwrap(), false, &opts).unwrap();

		assert!(env_path.is_dir(), "environments/dev should exist");
		assert!(env_path.join("main.jsonnet").exists());
		assert!(env_path.join("spec.json").exists());

		let main = fs::read_to_string(env_path.join("main.jsonnet")).unwrap();
		assert_eq!(main.trim(), "{}");

		let spec_content = fs::read_to_string(env_path.join("spec.json")).unwrap();
		let spec_value: serde_json::Value = serde_json::from_str(&spec_content).unwrap();
		assert_eq!(spec_value["spec"]["namespace"], "my-namespace");
		assert_eq!(spec_value["spec"]["apiServer"], "https://kube.example.com");
		assert_eq!(spec_value["spec"]["injectLabels"], true);
	}

	#[test]
	fn test_env_add_inline_creates_main_only() {
		let temp_dir = TempDir::new().unwrap();
		let root = temp_dir.path();
		create_project_root(root);
		let env_path = root.join("env-inline");

		let opts = EnvSpecOptions {
			namespace: Some("inline-ns".to_string()),
			server: Some("https://inline.example.com".to_string()),
			server_from_context: None,
			context_name: vec![],
			diff_strategy: None,
			inject_labels: None,
		};
		env_add(env_path.to_str().unwrap(), true, &opts).unwrap();

		assert!(env_path.is_dir());
		assert!(env_path.join("main.jsonnet").exists());
		assert!(
			!env_path.join("spec.json").exists(),
			"inline env should not have spec.json"
		);

		let main = fs::read_to_string(env_path.join("main.jsonnet")).unwrap();
		assert!(main.contains("tanka.dev/v1alpha1"));
		assert!(main.contains("inline-ns"));
		assert!(main.contains("https://inline.example.com"));
	}

	#[test]
	fn test_env_add_fails_when_already_exists() {
		let temp_dir = TempDir::new().unwrap();
		let root = temp_dir.path();
		create_project_root(root);
		let env_path = root.join("environments/dev");

		let opts = EnvSpecOptions {
			namespace: None,
			server: None,
			server_from_context: None,
			context_name: vec![],
			diff_strategy: None,
			inject_labels: Some(false),
		};
		env_add(env_path.to_str().unwrap(), false, &opts).unwrap();
		let err = env_add(env_path.to_str().unwrap(), false, &opts).unwrap_err();
		assert!(err.to_string().contains("already exists"));
	}

	#[test]
	fn test_env_set_updates_spec() {
		let temp_dir = TempDir::new().unwrap();
		let root = temp_dir.path();
		create_project_root(root);
		let env_path = root.join("environments/dev");

		let add_opts = EnvSpecOptions {
			namespace: Some("original-ns".to_string()),
			server: Some("https://original.example.com".to_string()),
			server_from_context: None,
			context_name: vec![],
			diff_strategy: None,
			inject_labels: Some(false),
		};
		env_add(env_path.to_str().unwrap(), false, &add_opts).unwrap();

		let set_opts = EnvSpecOptions {
			namespace: Some("updated-ns".to_string()),
			server: None,
			server_from_context: None,
			context_name: vec!["my-context".to_string()],
			diff_strategy: Some("server".to_string()),
			inject_labels: Some(true),
		};
		env_set(env_path.to_str().unwrap(), &set_opts).unwrap();

		let spec_content = fs::read_to_string(env_path.join("spec.json")).unwrap();
		let spec_value: serde_json::Value = serde_json::from_str(&spec_content).unwrap();
		assert_eq!(spec_value["spec"]["namespace"], "updated-ns");
		assert_eq!(
			spec_value["spec"]["contextNames"],
			serde_json::json!(["my-context"])
		);
		assert_eq!(spec_value["spec"]["diffStrategy"], "server");
		assert_eq!(spec_value["spec"]["injectLabels"], true);
	}

	#[test]
	fn test_env_remove_deletes_environment() {
		let temp_dir = TempDir::new().unwrap();
		let root = temp_dir.path();
		create_project_root(root);
		let env_path = root.join("environments/to-remove");

		let opts = EnvSpecOptions {
			namespace: Some("default".to_string()),
			server: None,
			server_from_context: None,
			context_name: vec![],
			diff_strategy: None,
			inject_labels: Some(false),
		};
		env_add(env_path.to_str().unwrap(), false, &opts).unwrap();
		assert!(env_path.exists());

		env_remove(&[env_path.to_string_lossy().to_string()]).unwrap();
		assert!(!env_path.exists());
	}

	#[test]
	fn test_env_remove_multiple() {
		let temp_dir = TempDir::new().unwrap();
		let root = temp_dir.path();
		create_project_root(root);
		let env_a = root.join("environments/a");
		let env_b = root.join("environments/b");

		let opts = EnvSpecOptions {
			namespace: Some("default".to_string()),
			server: None,
			server_from_context: None,
			context_name: vec![],
			diff_strategy: None,
			inject_labels: Some(false),
		};
		env_add(env_a.to_str().unwrap(), false, &opts).unwrap();
		env_add(env_b.to_str().unwrap(), false, &opts).unwrap();

		env_remove(&[
			env_a.to_string_lossy().to_string(),
			env_b.to_string_lossy().to_string(),
		])
		.unwrap();
		assert!(!env_a.exists());
		assert!(!env_b.exists());
	}
}
