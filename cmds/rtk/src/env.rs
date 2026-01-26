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

use crate::spec::Environment;

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
	let mut envs = find_environments(&search_path, &search_path)?;

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
fn find_environments(root: &Path, original_path: &Path) -> Result<Vec<Environment>> {
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
					if set_env_metadata(&mut env, dir, original_path).is_ok() {
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
								let _ = set_env_metadata(env, dir, original_path);
							} else {
								// Name exists from jsonnet, just set the namespace relative to search path
								let _ = set_env_namespace(env, dir, original_path);
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

/// Set environment metadata (name and namespace)
fn set_env_metadata(env: &mut Environment, dir: &Path, original_path: &Path) -> Result<()> {
	let env_path = compute_relative_env_path(dir, original_path);
	env.metadata.name = Some(env_path.clone());
	env.metadata.namespace = Some(format!("{}/main.jsonnet", env_path));
	Ok(())
}

/// Set only the namespace without changing the name
fn set_env_namespace(env: &mut Environment, dir: &Path, original_path: &Path) -> Result<()> {
	let relative_path = compute_relative_env_path(dir, original_path);
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

/// Find the project root by looking for jsonnetfile.json or tkrc.yaml
fn find_project_root(start_path: &Path) -> Option<PathBuf> {
	let mut current = start_path;
	loop {
		if current.join("jsonnetfile.json").exists() || current.join("tkrc.yaml").exists() {
			return Some(current.to_path_buf());
		}
		current = current.parent()?;
	}
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
	use super::*;
	use std::io::Cursor;
	use tempfile::TempDir;

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
}
