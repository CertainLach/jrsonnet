//! export - Export Tanka environments to files
//!
//! This module handles exporting multiple Tanka environments to files in parallel.
//! It evaluates environments and writes the resulting Kubernetes manifests to disk.

use std::{
	collections::{BTreeMap, HashMap, HashSet},
	fs,
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, Ordering},
		mpsc, Arc,
	},
	thread,
};

use anyhow::{bail, Context, Result};
use gtmpl::{FuncError, Value};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Serialize;
use serde_json::Value as JsonValue;
use tracing::{debug, trace};

use crate::{
	discover::{find_environments, DiscoveredEnv},
	eval::{eval, EvalOpts},
	jpath,
};

/// When exporting manifests to files, it becomes increasingly hard to map manifests back to its environment.
/// This file can be used to map the files back to their environment.
/// This is aimed to be used by CI/CD but can also be used for debugging purposes.
const MANIFEST_FILE: &str = "manifest.json";

/// BEL character used as placeholder for intentional path separators in templates
/// This matches Go Tanka's approach in pkg/tanka/export.go
const BEL_RUNE: char = '\x07';

/// Custom template function: default
/// Returns the first non-empty argument, mimicking Sprig's default function
/// Usage in templates: {{ .value | default "fallback" }}
/// Note: In Go templates, piped values become the LAST argument, so:
/// {{ .value | default "fallback" }} results in args = ["fallback", .value]
fn tmpl_default(args: &[Value]) -> Result<Value, FuncError> {
	// Iterate in REVERSE order because piped values come last
	// This way we check the piped value first, then fall back to explicit defaults
	for arg in args.iter().rev() {
		if !is_empty_value(arg) {
			return Ok(arg.clone());
		}
	}
	// If all are empty, return the first explicit default (or empty Value if no args)
	Ok(args.first().cloned().unwrap_or(Value::NoValue))
}

/// Helper function to check if a Value is empty (for default function)
fn is_empty_value(v: &Value) -> bool {
	match v {
		Value::NoValue | Value::Nil => true,
		Value::Bool(b) => !b,
		Value::String(s) => s.is_empty(),
		Value::Number(n) => n.as_f64().map(|f| f == 0.0).unwrap_or(false),
		Value::Array(a) => a.is_empty(),
		Value::Map(m) => m.is_empty(),
		Value::Object(o) => o.is_empty(),
		_ => false,
	}
}

/// Merge strategy for exporting to existing directories
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportMergeStrategy {
	/// Fail if output directory is not empty (default)
	None,
	/// Allow exporting to non-empty directory, but fail if any file would be overwritten
	FailOnConflicts,
	/// Delete files previously exported by the targeted environments and re-export them
	ReplaceEnvs,
}

impl Default for ExportMergeStrategy {
	fn default() -> Self {
		Self::None
	}
}

impl std::str::FromStr for ExportMergeStrategy {
	type Err = anyhow::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"" | "none" => Ok(Self::None),
			"fail-on-conflicts" => Ok(Self::FailOnConflicts),
			"replace-envs" => Ok(Self::ReplaceEnvs),
			_ => bail!("invalid merge strategy: {}", s),
		}
	}
}

/// Options for the export command
#[derive(Debug, Clone)]
pub struct ExportOpts {
	/// Output directory
	pub output_dir: PathBuf,
	/// File extension (yaml or json)
	pub extension: String,
	/// Filename format template (Go text/template syntax)
	pub format: String,
	/// Number of parallel workers
	pub parallelism: usize,
	/// Eval options to pass through
	pub eval_opts: EvalOpts,
	/// Environment name filter (for multi-env directories)
	pub name: Option<String>,
	/// Recursive mode - process all environments found
	pub recursive: bool,
	/// Skip generating manifest.json file that tracks exported files
	pub skip_manifest: bool,
	/// What to do when exporting to an existing directory
	pub merge_strategy: ExportMergeStrategy,
	/// Environments (main.jsonnet files) that have been deleted since the last export
	pub merge_deleted_envs: Vec<String>,
	/// Show detailed timing breakdown
	pub show_timing: bool,
}

impl Default for ExportOpts {
	fn default() -> Self {
		Self {
			output_dir: PathBuf::from("."),
			extension: "yaml".to_string(),
			format: "{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}"
				.to_string(),
			parallelism: 8,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: false,
			skip_manifest: false,
			merge_strategy: ExportMergeStrategy::default(),
			merge_deleted_envs: vec![],
			show_timing: false,
		}
	}
}

/// Timing data for export operations
///
/// This struct captures fine-grained timing information to help identify
/// performance bottlenecks in the export pipeline. The export process has
/// three distinct phases:
///
/// 1. **Evaluation (eval_ms)**: Time spent running jrsonnet to evaluate
///    the environment's main.jsonnet file. This is often the dominant cost
///    for complex environments with many imports.
///
/// 2. **Serialization (serialize_ms)**: Time spent processing manifests
///    in parallel - includes namespace/label injection, filename rendering,
///    and YAML/JSON serialization. Runs on all available CPU cores.
///
/// 3. **Writing (write_ms)**: Time spent creating directories and writing
///    files to disk using BufWriter. Sequential to avoid race conditions.
///
/// # Performance Analysis
///
/// Typical breakdown for a large environment (2800 manifests):
/// - eval_ms: ~400ms (jrsonnet evaluation)
/// - serialize_ms: ~150ms (parallelized on 8 cores)
/// - write_ms: ~50ms (sequential but buffered)
///
/// If eval_ms dominates, consider:
/// - Implementing shared import cache across environments
/// - Pre-warming lib/vendor imports
///
/// If serialize_ms dominates, consider:
/// - Increasing parallelism (--parallel flag)
/// - Profiling serde_saphyr serialization
#[derive(Debug, Clone, Default)]
pub struct ExportTimingData {
	/// Time spent evaluating Jsonnet (single-threaded jrsonnet)
	pub eval_ms: u128,
	/// Time spent serializing manifests (parallelized with Rayon)
	pub serialize_ms: u128,
	/// Time spent writing files to disk (sequential with BufWriter)
	pub write_ms: u128,
	/// Number of manifests processed (useful for per-manifest timing)
	pub manifest_count: usize,
}

/// Result of exporting a single environment
#[derive(Debug)]
pub struct ExportEnvResult {
	/// Path to the environment
	pub env_path: PathBuf,
	/// Files that were written (relative to output_dir)
	#[allow(dead_code)]
	pub files_written: Vec<PathBuf>,
	/// Environment namespace (for manifest.json tracking)
	pub env_namespace: Option<String>,
	/// Any error that occurred
	pub error: Option<String>,
	/// Timing data (if timing is enabled)
	/// Note: Currently populated but not displayed by the CLI. Available for future use
	/// or programmatic access via the library API.
	#[allow(dead_code)]
	pub timing: Option<ExportTimingData>,
}

/// Result of the export operation
#[derive(Debug)]
pub struct ExportResult {
	/// Total environments processed
	#[allow(dead_code)]
	pub total_envs: usize,
	/// Successfully exported environments
	#[allow(dead_code)]
	pub successful: usize,
	/// Failed environments
	pub failed: usize,
	/// Results for each environment
	pub results: Vec<ExportEnvResult>,
}

/// Errors that can occur during export
#[derive(Debug)]
enum ExportError {
	/// Fatal error - stop all processing
	Fatal(String),
	/// Per-environment error - log and continue
	#[allow(dead_code)]
	EnvError(PathBuf, String),
}

/// Export environments from given paths to the output directory
pub fn export(paths: &[String], opts: ExportOpts) -> Result<ExportResult> {
	use std::time::Instant;
	let export_start = Instant::now();

	trace!(
		"Starting export: {} paths, parallelism={}, output_dir={:?}",
		paths.len(),
		opts.parallelism,
		opts.output_dir
	);

	// PHASE 1: Validate template format FIRST (fail fast - Issue #2)
	let validate_start = Instant::now();
	trace!("Validating filename template: {}", opts.format);
	validate_filename_template(&opts.format)
		.context("Invalid filename format template - check Go text/template syntax")?;
	trace!(
		"Template validation completed in {}ms",
		validate_start.elapsed().as_millis()
	);

	// PHASE 2: Discover environments
	let discover_start = Instant::now();
	debug!("Finding Tanka environments in {} paths", paths.len());
	let envs = find_environments(paths)?;
	debug!(
		"Found {} Tanka environments in {}ms",
		envs.len(),
		discover_start.elapsed().as_millis()
	);

	if envs.is_empty() {
		return Ok(ExportResult {
			total_envs: 0,
			successful: 0,
			failed: 0,
			results: vec![],
		});
	}

	// PHASE 3: Check for ambiguous multi-environment case (Issue #4)
	if envs.len() > 1 && opts.name.is_none() && !opts.recursive {
		let env_names: Vec<_> = envs.iter().map(|e| e.path.display().to_string()).collect();
		bail!(
			"Found {} environments. Use --name to select one or --recursive to export all:\n{}",
			envs.len(),
			env_names
				.iter()
				.take(10)
				.map(|n| format!("  - {}", n))
				.collect::<Vec<_>>()
				.join("\n")
		);
	}

	// Filter by name if specified
	let envs: Vec<_> = if let Some(ref name) = opts.name {
		trace!("Filtering environments by name: {}", name);
		envs.into_iter()
			.filter(|e| {
				// For inline environments, check env_name (metadata.name)
				if let Some(ref env_name) = e.env_name {
					if env_name.contains(name) {
						return true;
					}
				}
				// Also allow filtering by path
				e.path.to_string_lossy().contains(name)
			})
			.collect()
	} else {
		envs
	};

	if envs.is_empty() {
		bail!(
			"No environments found matching name filter: {:?}",
			opts.name
		);
	}

	trace!(
		"Will export {} environments: {:?}",
		envs.len(),
		envs.iter()
			.map(|e| e.path.display().to_string())
			.collect::<Vec<_>>()
	);

	// Create output directory
	trace!("Creating output directory: {:?}", opts.output_dir);
	fs::create_dir_all(&opts.output_dir)
		.context(format!("creating output directory {:?}", opts.output_dir))?;

	// Check if directory is empty (if required by merge strategy)
	if opts.merge_strategy == ExportMergeStrategy::None {
		trace!("Checking if output directory is empty (merge_strategy=none)");
		let is_empty = is_dir_empty(&opts.output_dir)?;
		if !is_empty {
			bail!(
				"output dir `{}` not empty. Pass a different --merge-strategy to ignore this",
				opts.output_dir.display()
			);
		}
	}

	// Read the existing manifest to know which files belong to which environments
	// This is used for conflict detection after exports complete
	let existing_manifest: HashMap<String, String> = {
		let manifest_path = opts.output_dir.join(MANIFEST_FILE);
		if manifest_path.exists() {
			let content = fs::read_to_string(&manifest_path)
				.context("reading manifest.json for conflict detection")?;
			serde_json::from_str(&content)
				.context("parsing manifest.json for conflict detection")?
		} else {
			HashMap::new()
		}
	};

	// Collect files previously exported by the targeted environments
	// These can be safely overwritten during re-export
	let mut previously_exported = if opts.merge_strategy == ExportMergeStrategy::ReplaceEnvs {
		collect_previously_exported_files(&opts.output_dir, &envs)?
	} else {
		HashSet::new()
	};

	// Also collect files from environments that have been deleted
	if !opts.merge_deleted_envs.is_empty() {
		let files =
			collect_previously_exported_by_names(&opts.output_dir, &opts.merge_deleted_envs)?;
		previously_exported.extend(files);
	}

	// Abort flag for early termination (Issue #3 & #5)
	let abort_flag = Arc::new(AtomicBool::new(false));

	// Dynamic scheduling: spawn threads up to parallelism limit
	// Fresh threads for each export ensure thread-local GC state is freed
	let (tx, rx) = mpsc::channel();
	let mut env_iter = envs.iter().enumerate();
	let total_envs = envs.len();
	let show_timing = opts.show_timing;

	debug!("Loading {} environments", total_envs);
	let parallel_start = Instant::now();

	// Spawn initial batch of threads (up to parallelism limit)
	for _ in 0..opts.parallelism {
		if let Some((idx, env)) = env_iter.next() {
			let env = env.clone();
			let opts = opts.clone();
			let abort_flag = Arc::clone(&abort_flag);
			let tx = tx.clone();

			thread::spawn(move || {
				if abort_flag.load(Ordering::Relaxed) {
					let result = ExportEnvResult {
						env_path: env.path.clone(),
						files_written: vec![],
						env_namespace: None,
						error: Some("Skipped due to earlier fatal error".to_string()),
						timing: None,
					};
					let _ = tx.send((idx, result));
					return;
				}

				let result = match export_single_env(&env, &opts) {
					Ok((files, namespace, timing)) => ExportEnvResult {
						env_path: env.path.clone(),
						files_written: files,
						env_namespace: Some(namespace),
						error: None,
						timing: if show_timing { Some(timing) } else { None },
					},
					Err(ExportError::Fatal(msg)) => {
						abort_flag.store(true, Ordering::Relaxed);
						ExportEnvResult {
							env_path: env.path.clone(),
							files_written: vec![],
							env_namespace: None,
							error: Some(format!("FATAL: {}", msg)),
							timing: None,
						}
					}
					Err(ExportError::EnvError(_, msg)) => ExportEnvResult {
						env_path: env.path.clone(),
						files_written: vec![],
						env_namespace: None,
						error: Some(msg),
						timing: None,
					},
				};

				let _ = tx.send((idx, result));
			});
		}
	}

	// Keep original sender alive for spawning more threads
	let tx_main = tx.clone();
	drop(tx); // Drop this clone so channel closes when all threads complete

	// Collect results and dynamically spawn new threads as old ones complete
	let mut collected_results = HashMap::with_capacity(total_envs);
	let mut received_count = 0;

	for (idx, result) in rx {
		collected_results.insert(idx, result);
		received_count += 1;

		// Spawn a new thread for the next environment (if any remain)
		if let Some((next_idx, env)) = env_iter.next() {
			let env = env.clone();
			let opts = opts.clone();
			let abort_flag = Arc::clone(&abort_flag);
			let tx = tx_main.clone();

			thread::spawn(move || {
				if abort_flag.load(Ordering::Relaxed) {
					let result = ExportEnvResult {
						env_path: env.path.clone(),
						files_written: vec![],
						env_namespace: None,
						error: Some("Skipped due to earlier fatal error".to_string()),
						timing: None,
					};
					let _ = tx.send((next_idx, result));
					return;
				}

				let result = match export_single_env(&env, &opts) {
					Ok((files, namespace, timing)) => ExportEnvResult {
						env_path: env.path.clone(),
						files_written: files,
						env_namespace: Some(namespace),
						error: None,
						timing: if show_timing { Some(timing) } else { None },
					},
					Err(ExportError::Fatal(msg)) => {
						abort_flag.store(true, Ordering::Relaxed);
						ExportEnvResult {
							env_path: env.path.clone(),
							files_written: vec![],
							env_namespace: None,
							error: Some(format!("FATAL: {}", msg)),
							timing: None,
						}
					}
					Err(ExportError::EnvError(_, msg)) => ExportEnvResult {
						env_path: env.path.clone(),
						files_written: vec![],
						env_namespace: None,
						error: Some(msg),
						timing: None,
					},
				};

				let _ = tx.send((next_idx, result));
			});
		} else if received_count == total_envs {
			// All environments processed, drop the sender to close the channel
			drop(tx_main);
			break;
		}
	}

	// Extract results in order by index
	let mut results: Vec<_> = collected_results.into_iter().collect();
	results.sort_by_key(|(idx, _)| *idx);
	let results: Vec<ExportEnvResult> = results.into_iter().map(|(_, r)| r).collect();

	trace!(
		"Parallel export completed in {}ms",
		parallel_start.elapsed().as_millis()
	);

	// Summarize results
	let successful = results.iter().filter(|r| r.error.is_none()).count();
	let failed = results.iter().filter(|r| r.error.is_some()).count();

	trace!(
		"Export summary: {} successful, {} failed out of {} total",
		successful,
		failed,
		total_envs
	);

	// Check for file conflicts and collect files written
	// Conflicts occur when:
	// 1. Multiple environments in this export write the same file
	// 2. A file was previously exported by a different environment (not being re-exported)
	let mut all_files_written: HashMap<String, PathBuf> = HashMap::new();
	for result in &results {
		if result.error.is_none() {
			for file in &result.files_written {
				// Convert PathBuf to string with forward slashes (cross-platform)
				let file_str = file
					.components()
					.map(|c| c.as_os_str().to_string_lossy())
					.collect::<Vec<_>>()
					.join("/");

				// Check for conflicts between environments in this export
				if let Some(existing_env) = all_files_written.get(&file_str) {
					bail!(
						"file '{}' written by multiple environments: '{}' and '{}'",
						file_str,
						existing_env.display(),
						result.env_path.display()
					);
				}

				// Check for conflicts with files from other environments (not being re-exported)
				// A file in existing_manifest that's NOT in previously_exported belongs to another env
				if existing_manifest.contains_key(&file_str)
					&& !previously_exported.contains(&file_str)
				{
					let other_env = existing_manifest.get(&file_str).unwrap();
					bail!(
						"file '{}' already exists from environment '{}'. Aborting",
						file_str,
						other_env
					);
				}

				all_files_written.insert(file_str.clone(), result.env_path.clone());
				previously_exported.remove(&file_str);
			}
		}
	}

	// Delete files that were previously exported but not re-exported
	// (deferred deletion - these are files whose environments were updated but the file is no longer produced)
	let files_to_delete: Vec<String> = previously_exported.into_iter().collect();

	if !files_to_delete.is_empty() {
		trace!(
			"Deleting {} files that were not re-exported",
			files_to_delete.len()
		);
		for file in &files_to_delete {
			let file_path = opts.output_dir.join(file);
			// Ignore errors if file doesn't exist
			let _ = fs::remove_file(&file_path);

			// Try to clean up empty parent directories
			if let Some(parent) = file_path.parent() {
				if parent != opts.output_dir.as_path() {
					let _ = fs::remove_dir(parent);
				}
			}
		}
	}

	// Generate manifest.json file if not skipped
	// Also removes entries for deleted files
	if !opts.skip_manifest {
		trace!("Writing manifest.json");
		export_manifest_file(&opts.output_dir, &results, &files_to_delete)?;
	}

	trace!(
		"Total export completed in {}ms",
		export_start.elapsed().as_millis()
	);

	Ok(ExportResult {
		total_envs: envs.len(),
		successful,
		failed,
		results,
	})
}

/// Validate that the filename template is valid Go text/template syntax (Issue #2)
fn validate_filename_template(format: &str) -> Result<()> {
	use std::collections::BTreeMap;

	use crate::spec::{Environment, Metadata, Spec};

	// Create a test manifest with all expected fields
	let test_manifest = serde_json::json!({
		"apiVersion": "v1",
		"kind": "ConfigMap",
		"metadata": {
			"name": "test",
			"generateName": "test-",
			"namespace": "default",
			"labels": {
				"app": "test"
			}
		}
	});

	// Create a test environment with typical fields including labels
	let mut labels = BTreeMap::new();
	labels.insert("cluster_name".to_string(), "test-cluster".to_string());
	labels.insert("team".to_string(), "test-team".to_string());
	labels.insert("fluxExport".to_string(), "true".to_string());
	labels.insert("fluxExportDir".to_string(), "test-dir".to_string());

	let test_env = Some(Environment {
		api_version: "tanka.dev/v1alpha1".to_string(),
		kind: "Environment".to_string(),
		metadata: Metadata {
			name: Some("test-env".to_string()),
			namespace: Some("default".to_string()),
			labels: Some(labels),
		},
		spec: Spec {
			api_server: Some("https://kubernetes.default.svc".to_string()),
			context_names: None,
			namespace: "default".to_string(),
			diff_strategy: None,
			apply_strategy: None,
			inject_labels: None,
			resource_defaults: None,
			expect_versions: None,
			export_jsonnet_implementation: None,
		},
		data: None,
	});

	// Try to render with the template
	format_filename_gtmpl(&test_manifest, &test_env, format)
		.context("Template validation failed")?;

	Ok(())
}

/// Count Environment objects in a JSON value (for multi-env detection)
fn count_environment_objects(value: &JsonValue) -> usize {
	let mut count = 0;

	match value {
		JsonValue::Object(obj) => {
			// Check if this is an Environment object
			if obj.get("kind").and_then(|v| v.as_str()) == Some("Environment")
				&& obj.contains_key("apiVersion")
			{
				count += 1;
			}
			// Recurse into object values
			for v in obj.values() {
				count += count_environment_objects(v);
			}
		}
		JsonValue::Array(arr) => {
			for v in arr {
				count += count_environment_objects(v);
			}
		}
		_ => {}
	}

	count
}

/// Export a single environment
/// Returns (files_written, environment_namespace, timing_data)
fn export_single_env(
	env: &DiscoveredEnv,
	opts: &ExportOpts,
) -> Result<(Vec<PathBuf>, String, ExportTimingData), ExportError> {
	use std::time::Instant;

	let env_start = Instant::now();
	let env_display = env.path.display().to_string();
	let env_name_display = env.env_name.as_deref().unwrap_or("");
	debug!(
		"Loading environment name={} path={}",
		env_name_display, env_display
	);

	let mut timing = ExportTimingData::default();

	// Evaluate the environment, passing the env_name if this is a sub-environment
	let mut eval_opts = opts.eval_opts.clone();
	eval_opts.env_name = env.env_name.clone();
	// Pass exportJsonnetImplementation from discovery so eval can use jrsonnet-compatible formatting
	eval_opts.export_jsonnet_implementation = env.export_jsonnet_implementation.clone();

	trace!("[{}] Starting Jsonnet evaluation", env_display);
	let eval_start = Instant::now();
	let result = eval(env.path.to_string_lossy().as_ref(), eval_opts)
		.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;
	timing.eval_ms = eval_start.elapsed().as_millis();
	trace!(
		"[{}] Jsonnet evaluation completed in {}ms",
		env_display,
		timing.eval_ms
	);

	// Check for multiple Environment objects (Issue C - match tk behavior)
	let env_count = count_environment_objects(&result.value);
	if env_count > 1 && opts.name.is_none() && !opts.recursive {
		return Err(ExportError::EnvError(
			env.path.clone(),
			format!(
				"found {} Environments. Use --name to select a single one or --recursive to export all",
				env_count
			),
		));
	}

	// Extract environment identifier for manifest.json tracking
	// This should be the path to main.jsonnet (relative to working directory if possible)
	let main_jsonnet_path = env.path.join("main.jsonnet");
	let env_namespace = if let Ok(cwd) = std::env::current_dir() {
		// Make path relative to current directory if possible
		main_jsonnet_path
			.strip_prefix(&cwd)
			.unwrap_or(&main_jsonnet_path)
			.to_string_lossy()
			.to_string()
	} else {
		main_jsonnet_path.to_string_lossy().to_string()
	};

	// Extract Environment objects (matching Tanka's inline.go/static.go pattern)
	trace!("[{}] Extracting Environment objects", env_display);
	let extract_start = Instant::now();
	let mut environments = extract_environments(&result.value, &result.spec)
		.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;
	trace!(
		"[{}] Extracted {} Environment objects in {}ms",
		env_display,
		environments.len(),
		extract_start.elapsed().as_millis()
	);

	// For inline environments (those without spec.json), set metadata.namespace to the relative
	// path from root to entrypoint. This matches Go Tanka's behavior in pkg/tanka/inline.go:inlineParse
	// which calls spec.Parse with namespace = filepath.Rel(root, file)
	if result.spec.is_none() {
		// This is an inline environment - resolve jpath to get root and entrypoint
		if let Ok(jpath_result) = jpath::resolve(env.path.to_string_lossy().as_ref()) {
			// Compute namespace as relative path from root to entrypoint
			if let Ok(rel_entrypoint) = jpath_result.entrypoint.strip_prefix(&jpath_result.root) {
				let namespace = rel_entrypoint.to_string_lossy().to_string();
				// Set namespace on all extracted inline environments
				for env_data in &mut environments {
					if let Some(ref mut spec) = env_data.spec {
						spec.metadata.namespace = Some(namespace.clone());
					}
				}
			}
		}
	}

	// If a specific env_name is requested, filter to only that environment
	// This prevents processing nested environments that belong to other DiscoveredEnv entries
	if let Some(ref target_name) = env.env_name {
		environments.retain(|env_data| {
			let name = env_data
				.spec
				.as_ref()
				.and_then(|s| s.metadata.name.as_deref())
				.unwrap_or("");
			name == target_name
		});
	}

	if environments.is_empty() {
		trace!("[{}] No environments to process, skipping", env_display);
		return Ok((vec![], env_namespace, timing));
	}

	// Use output directory directly (matching tk behavior)
	// Note: tk writes directly to output_dir without creating env subdirectories
	fs::create_dir_all(&opts.output_dir)
		.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;

	let mut files_written = Vec::new();

	// Process each Environment's manifests separately (matching Tanka's approach)
	// This avoids loading all manifests into memory at once
	for (env_idx, env_data) in environments.iter().enumerate() {
		let env_name = env_data
			.spec
			.as_ref()
			.and_then(|s| s.metadata.name.as_deref())
			.unwrap_or("unnamed");

		trace!(
			"[{}] Processing sub-environment {}/{}: {}",
			env_display,
			env_idx + 1,
			environments.len(),
			env_name
		);

		// Extract manifests from this environment's data field
		trace!("[{}:{}] Collecting manifests", env_display, env_name);
		let collect_start = Instant::now();
		let mut manifests = Vec::new();
		collect_manifests_with_validation(&env_data.data, &mut manifests, "")
			.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;
		trace!(
			"[{}:{}] Collected {} manifests in {}ms",
			env_display,
			env_name,
			manifests.len(),
			collect_start.elapsed().as_millis()
		);

		// Skip if there are no manifests to process
		if manifests.is_empty() {
			trace!(
				"[{}:{}] No manifests to process, skipping",
				env_display,
				env_name
			);
			continue;
		}

		// MAJOR OPTIMIZATION: Pre-substitute env values into template once per environment
		// Instead of evaluating env.metadata.labels.X thousands of times, bake values into template
		trace!("[{}:{}] Specializing template", env_display, env_name);
		let specialized_template = specialize_template_for_env(&opts.format, &env_data.spec)
			.map_err(|e| ExportError::Fatal(format!("Failed to specialize template: {}", e)))?;

		// Parse the specialized template once per environment
		let mut tmpl = gtmpl::Template::default();
		// Register custom Sprig-compatible functions
		tmpl.add_func("default", tmpl_default);
		tmpl.parse(&specialized_template)
			.map_err(|e| ExportError::Fatal(format!("Template parse error: {:?}", e)))?;

		// ============================================================================
		// PERFORMANCE OPTIMIZATION: Parallel Manifest Processing
		// ============================================================================
		//
		// For environments with thousands of manifests (e.g., grafana-o11y generates
		// ~2800 files), sequential processing creates a significant bottleneck.
		// Profiling shows YAML serialization is the dominant cost (~70-80% of time).
		//
		// Strategy: Two-phase processing to maximize CPU utilization while avoiding
		// race conditions in filesystem operations:
		//
		// PHASE 1 - PARALLEL (CPU-bound, uses all available cores):
		//   - Namespace/label injection (in-memory JSON manipulation)
		//   - Filename rendering via Go templates (string operations)
		//   - YAML/JSON serialization (the main bottleneck - serde + formatting)
		//
		// PHASE 2 - SEQUENTIAL (I/O-bound, avoids race conditions):
		//   - Directory creation (must be sequential to avoid mkdir races)
		//   - File writes with BufWriter (batched I/O reduces syscall overhead)
		//
		// Why not parallelize Phase 2?
		//   - create_dir_all() is not thread-safe for overlapping paths
		//   - File system locks and cache contention reduce parallel I/O benefits
		//   - Sequential BufWriter is usually fast enough once data is in memory
		//
		// Expected speedup: 20-40% for environments with 1000+ manifests
		// (Actual improvement depends on CPU cores and serialization complexity)
		// ============================================================================

		trace!(
			"[{}:{}] Starting parallel serialization of {} manifests",
			env_display,
			env_name,
			manifests.len()
		);
		let serialize_start = Instant::now();
		let manifest_count = manifests.len();
		let processed_manifests: Result<Vec<_>, ExportError> = manifests
			.into_par_iter()
			.map(|mut manifest| {
				// Inject namespace if needed (matching Tanka's behavior in pkg/process/namespace.go)
				inject_namespace(&mut manifest, &env_data.spec);

				// Inject tanka.dev/environment label if needed (matching Tanka's behavior in pkg/process/process.go)
				inject_environment_label(&mut manifest, &env_data.spec);

				// Inject resourceDefaults annotations if present (matching Tanka's behavior)
				inject_resource_defaults(&mut manifest, &env_data.spec);

				// Strip null values from metadata.annotations and metadata.labels
				// (standard Kubernetes YAML behavior - null fields should be omitted)
				strip_null_metadata_fields(&mut manifest);

				let rendered_filename = render_filename_simple(&tmpl, &manifest, &env_data.spec)
					.map_err(|e| {
						// Template errors after validation are fatal (something very wrong)
						ExportError::Fatal(format!("Template rendering failed: {}", e))
					})?;

				// Apply Go Tanka's path processing:
				// 1. Replace / with - (prevents accidental subdirs from values like apps/v1)
				// 2. Replace BEL_RUNE back to / (restores intentional subdirs from format)
				let filename = apply_template_path_processing(&rendered_filename);

				// Split by / (now only intentional separators) and sanitize each path component
				// Filter out empty components but keep <no value> (matches tk behavior for cluster-scoped resources)
				let path_parts: Vec<String> = filename
					.split('/')
					.map(|part| part.trim())
					.filter(|part| !part.is_empty())
					.map(|part| sanitize_path_component(part))
					.filter(|part| !part.is_empty())
					.collect();

				if path_parts.is_empty() {
					return Err(ExportError::Fatal(format!(
						"Template produced empty filename for manifest: {}",
						serde_json::to_string(&manifest).unwrap_or_else(|_| "unknown".to_string())
					)));
				}

				// Join path components and add extension to the last component
				let mut relative_path = std::path::PathBuf::new();
				for (i, part) in path_parts.iter().enumerate() {
					if i == path_parts.len() - 1 {
						// Last component - add extension
						relative_path.push(format!("{}.{}", part, opts.extension));
					} else {
						// Directory component
						relative_path.push(part);
					}
				}

				// Serialize manifest (CPU-intensive, good for parallelization)
				let content = if opts.extension == "json" {
					serde_json::to_string_pretty(&manifest)
						.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?
				} else {
					// Sort all object keys to match Go's yaml.v3 output order
					let sorted_manifest = sort_json_keys(manifest);

					// Use serializer options to match Go's yaml.v2 output (used by tk for manifest export)
					let options = serde_saphyr::SerializerOptions {
						indent_step: 2,
						indent_array: Some(0),
						prefer_block_scalars: true,
						empty_map_as_braces: true,
						empty_array_as_brackets: true,
						line_width: Some(80),
						scientific_notation_threshold: Some(1000000), // 1 million
						scientific_notation_small_threshold: Some(0.0001), // Small floats like 0.00001 become 1e-05
						quote_ambiguous_keys: true,                   // Quote y, n, yes, no, etc. to match Go yaml.v3
						quote_numeric_strings: true, // Quote numeric string keys like "12", "12.5" to match Go yaml.v3
						..Default::default()
					};
					let mut output = String::new();
					serde_saphyr::to_fmt_writer_with_options(
						&mut output,
						&sorted_manifest,
						options,
					)
					.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;
					output
				};

				Ok((relative_path, content))
			})
			.collect();

		let processed_manifests = processed_manifests?;
		timing.serialize_ms += serialize_start.elapsed().as_millis();
		timing.manifest_count += manifest_count;
		trace!(
			"[{}:{}] Serialization completed in {}ms ({} manifests, {:.2}ms/manifest)",
			env_display,
			env_name,
			serialize_start.elapsed().as_millis(),
			manifest_count,
			serialize_start.elapsed().as_millis() as f64 / manifest_count as f64
		);

		// Phase 2: Write files (I/O-bound, kept sequential for directory coordination)
		// Note: File writes could also be parallelized with proper directory creation synchronization
		trace!(
			"[{}:{}] Starting sequential file writes for {} files",
			env_display,
			env_name,
			manifest_count
		);
		let write_start = Instant::now();
		let mut files_skipped = 0usize;
		for (relative_path, content) in processed_manifests {
			let filepath = opts.output_dir.join(&relative_path);

			// Create parent directories if needed
			if let Some(parent) = filepath.parent() {
				fs::create_dir_all(parent)
					.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;
			}

			// PERFORMANCE: Skip write if file exists with identical content
			// This is a significant optimization for re-exports where most files don't change.
			// On slow storage (e.g., EBS), reading to compare is much faster than writing.
			if filepath.exists() {
				if let Ok(existing_content) = fs::read_to_string(&filepath) {
					if existing_content == content {
						files_written.push(relative_path);
						files_skipped += 1;
						continue;
					}
				}
				// File exists but content differs or couldn't be read - will be overwritten
			}

			// PERFORMANCE: Use BufWriter to reduce syscall overhead
			//
			// Without BufWriter, each write_all() would result in a direct write() syscall.
			// BufWriter batches small writes into 8KB chunks (default buffer size),
			// significantly reducing kernel transitions for typical manifest files (2-20KB).
			//
			// Benchmark context:
			// - Direct write: ~1 syscall per file
			// - BufWriter: ~1-3 syscalls per file (depending on size)
			// - For 2800 files: saves ~2000+ syscalls
			//
			// The buffer is automatically flushed when the writer is dropped.
			use std::io::Write;
			let file = fs::File::create(&filepath)
				.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;
			let mut writer = std::io::BufWriter::new(file);
			writer
				.write_all(content.as_bytes())
				.map_err(|e| ExportError::EnvError(env.path.clone(), e.to_string()))?;

			// Track relative path for manifest.json
			files_written.push(relative_path);
		}
		if files_skipped > 0 {
			trace!(
				"[{}:{}] Skipped {} unchanged files",
				env_display,
				env_name,
				files_skipped
			);
		}
		timing.write_ms += write_start.elapsed().as_millis();
		trace!(
			"[{}:{}] File writes completed in {}ms ({} files, {:.2}ms/file)",
			env_display,
			env_name,
			write_start.elapsed().as_millis(),
			manifest_count,
			write_start.elapsed().as_millis() as f64 / manifest_count as f64
		);
	}

	debug!(
		"Finished loading environment name={} path={} duration_ms={}",
		env_name_display,
		env_display,
		env_start.elapsed().as_millis()
	);

	Ok((files_written, env_namespace, timing))
}

/// Export manifest file that maps exported files to their environment
/// Merges with existing manifest.json if present
/// Also removes entries for files that were deleted (not re-exported)
fn export_manifest_file(
	output_dir: &PathBuf,
	results: &[ExportEnvResult],
	deleted_files: &[String],
) -> Result<()> {
	let manifest_path = output_dir.join(MANIFEST_FILE);

	// Read existing manifest.json if it exists
	let mut file_to_env: HashMap<String, String> = if manifest_path.exists() {
		let content =
			fs::read_to_string(&manifest_path).context("reading existing manifest.json")?;
		serde_json::from_str(&content).context("parsing existing manifest.json")?
	} else {
		HashMap::new()
	};

	// Remove entries for files that were deleted (not re-exported)
	for file in deleted_files {
		file_to_env.remove(file);
	}

	// Add new entries from successful exports
	for result in results {
		if result.error.is_none() {
			if let Some(ref namespace) = result.env_namespace {
				for file in &result.files_written {
					// Convert PathBuf to string with forward slashes (cross-platform)
					let file_str = file
						.components()
						.map(|c| c.as_os_str().to_string_lossy())
						.collect::<Vec<_>>()
						.join("/");
					file_to_env.insert(file_str, namespace.clone());
				}
			}
		}
	}

	// Write manifest.json with sorted keys
	let content = {
		let mut buf = Vec::new();
		let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
		let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
		// Convert to BTreeMap to ensure sorted keys
		let sorted: BTreeMap<_, _> = file_to_env.into_iter().collect();
		sorted
			.serialize(&mut ser)
			.context("serializing manifest.json")?;
		String::from_utf8(buf).expect("valid utf8")
	};
	fs::write(&manifest_path, content).context("writing manifest.json")?;

	Ok(())
}

/// Extract Kubernetes manifests from evaluation result, keeping track of which Environment each came from
/// Returns pairs of (manifest, env_spec) where env_spec is the Environment that contains this manifest
/// Holds an Environment and its associated data for processing
struct EnvironmentData {
	spec: Option<crate::spec::Environment>,
	data: JsonValue,
}

/// Extract Environment objects from evaluated Jsonnet (matching Tanka's inline.go extractEnvs)
/// For static environments, returns a single EnvironmentData with the spec and all manifests
/// For inline environments, returns multiple EnvironmentData, one per Environment object
fn extract_environments(
	value: &JsonValue,
	default_env_spec: &Option<crate::spec::Environment>,
) -> Result<Vec<EnvironmentData>> {
	let mut environments = Vec::new();

	// Recursively search for Environment objects
	collect_environments_recursive(value, &mut environments);

	// Deduplicate environments by name (same Environment can appear at multiple JSON paths
	// due to Jsonnet object composition with +)
	let mut seen_names = std::collections::HashSet::new();
	environments.retain(|env_data| {
		let name = env_data
			.spec
			.as_ref()
			.and_then(|s| s.metadata.name.as_ref())
			.cloned()
			.unwrap_or_default();
		seen_names.insert(name)
	});

	if !environments.is_empty() {
		return Ok(environments);
	}

	// No Environment objects found - treat all output as belonging to default environment (static case)
	environments.push(EnvironmentData {
		spec: default_env_spec.clone(),
		data: value.clone(),
	});

	Ok(environments)
}

/// Recursively search for Environment objects (matching Tanka's extractEnvs logic)
fn collect_environments_recursive(value: &JsonValue, environments: &mut Vec<EnvironmentData>) {
	match value {
		JsonValue::Object(obj) => {
			// Check if this object itself is an Environment
			if obj.get("kind").and_then(|v| v.as_str()) == Some("Environment")
				&& obj.contains_key("apiVersion")
			{
				// Extract data field
				let data = obj.get("data").cloned().unwrap_or(JsonValue::Null);

				// Parse this Environment object to get its spec
				let env_spec: Result<crate::spec::Environment, _> =
					serde_json::from_value(value.clone());
				let env_spec_opt = match env_spec {
					Ok(spec) => Some(spec),
					Err(e) => {
						eprintln!(
							"Warning: Failed to parse Environment object: {}. Manifests will be processed without environment context.",
							e
						);
						None
					}
				};

				environments.push(EnvironmentData {
					spec: env_spec_opt,
					data,
				});
			} else {
				// Recurse into object values
				for v in obj.values() {
					collect_environments_recursive(v, environments);
				}
			}
		}
		JsonValue::Array(arr) => {
			// Recurse into array elements
			for item in arr {
				collect_environments_recursive(item, environments);
			}
		}
		_ => {}
	}
}

/// Extract Kubernetes manifests from evaluation result (legacy function for tests)
#[allow(dead_code)]
fn extract_manifests(value: &JsonValue) -> Result<Vec<JsonValue>> {
	let environments = extract_environments(value, &None)?;
	let mut all_manifests = Vec::new();
	for env_data in environments {
		collect_manifests_with_validation(&env_data.data, &mut all_manifests, "")?;
	}
	Ok(all_manifests)
}

/// Recursively collect Kubernetes manifests from a JSON value
/// Generate a unique key for a manifest based on apiVersion, kind, namespace, and name
/// Used to deduplicate manifests that appear at multiple paths in the JSON structure
///
/// Validates that objects which look like Kubernetes objects (have `kind` and `metadata`)
/// also have `apiVersion`. This matches Tanka's validation behavior.
fn collect_manifests_with_validation(
	value: &JsonValue,
	manifests: &mut Vec<JsonValue>,
	path: &str,
) -> Result<()> {
	match value {
		JsonValue::Object(obj) => {
			let has_api_version = obj.contains_key("apiVersion");
			let has_kind = obj.contains_key("kind");
			let has_metadata = obj.contains_key("metadata");

			// Check if this looks like a Kubernetes manifest (has apiVersion and kind)
			if has_api_version && has_kind {
				if let Some(JsonValue::String(kind)) = obj.get("kind") {
					// Skip Tanka Environment objects
					if kind == "Environment" {
						// Extract from data field if present
						if let Some(data) = obj.get("data") {
							collect_manifests_with_validation(data, manifests, path)?;
						}
						return Ok(());
					}

					// Expand List kind - extract items as individual manifests
					// This matches Tanka's behavior where List items are exported as separate files
					if kind == "List" {
						if let Some(JsonValue::Array(items)) = obj.get("items") {
							for (i, item) in items.iter().enumerate() {
								let item_path = format!("{}.items[{}]", path, i);
								collect_manifests_with_validation(item, manifests, &item_path)?;
							}
						}
						return Ok(());
					}
				}
				manifests.push(value.clone());
				Ok(())
			} else if has_kind && has_metadata && !has_api_version {
				// Object looks like a Kubernetes manifest (has kind and metadata) but is missing apiVersion
				// This matches Tanka's validation: "found invalid Kubernetes object (at .X): missing attribute "apiVersion""
				bail!(
					"found invalid Kubernetes object (at {}): missing attribute \"apiVersion\"",
					path
				);
			} else {
				// Recurse into object values
				for (key, v) in obj.iter() {
					let child_path = if path.is_empty() {
						format!(".{}", key)
					} else {
						format!("{}.{}", path, key)
					};
					collect_manifests_with_validation(v, manifests, &child_path)?;
				}
				Ok(())
			}
		}
		JsonValue::Array(arr) => {
			// Recurse into array elements
			for (i, v) in arr.iter().enumerate() {
				let item_path = format!("{}[{}]", path, i);
				collect_manifests_with_validation(v, manifests, &item_path)?;
			}
			Ok(())
		}
		_ => Ok(()),
	}
}

/// Check if a Kubernetes kind is cluster-wide (not namespaced)
/// This list is from Tanka's pkg/process/namespace.go
fn is_cluster_wide_kind(kind: &str) -> bool {
	matches!(
		kind,
		"APIService"
			| "CertificateSigningRequest"
			| "ClusterRole"
			| "ClusterRoleBinding"
			| "ComponentStatus"
			| "CSIDriver"
			| "CSINode"
			| "CustomResourceDefinition"
			| "MutatingWebhookConfiguration"
			| "Namespace"
			| "Node" | "NodeMetrics"
			| "PersistentVolume"
			| "PodSecurityPolicy"
			| "PriorityClass"
			| "RuntimeClass"
			| "SelfSubjectAccessReview"
			| "SelfSubjectRulesReview"
			| "StorageClass"
			| "SubjectAccessReview"
			| "TokenReview"
			| "ValidatingWebhookConfiguration"
			| "VolumeAttachment"
	)
}

/// Inject namespace into a manifest if needed (matching Tanka's pkg/process/namespace.go)
fn inject_namespace(manifest: &mut JsonValue, env_spec: &Option<crate::spec::Environment>) {
	if let JsonValue::Object(ref mut obj) = manifest {
		// Get kind and check if it's cluster-wide
		let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
		let is_cluster_wide = is_cluster_wide_kind(kind);

		// Ensure metadata exists
		if !obj.contains_key("metadata") {
			obj.insert(
				"metadata".to_string(),
				JsonValue::Object(serde_json::Map::new()),
			);
		}

		if let Some(JsonValue::Object(ref mut metadata)) = obj.get_mut("metadata") {
			// Check for annotation override (tanka.dev/namespaced)
			let mut namespaced = !is_cluster_wide;
			if let Some(JsonValue::Object(annotations)) = metadata.get("annotations") {
				if let Some(JsonValue::String(ns_str)) = annotations.get("tanka.dev/namespaced") {
					namespaced = ns_str == "true";
				}
			}

			// Inject namespace if needed
			if namespaced {
				let has_namespace = metadata.contains_key("namespace")
					&& metadata
						.get("namespace")
						.and_then(|v| v.as_str())
						.map(|s| !s.is_empty())
						.unwrap_or(false);

				if !has_namespace {
					if let Some(env) = env_spec {
						if !env.spec.namespace.is_empty() {
							metadata.insert(
								"namespace".to_string(),
								JsonValue::String(env.spec.namespace.clone()),
							);
						}
					}
				}
			}
		}
	}
}

/// Inject tanka.dev/environment label into manifest metadata
/// This replicates the behavior from Tanka's pkg/process/process.go
fn inject_environment_label(manifest: &mut JsonValue, env_spec: &Option<crate::spec::Environment>) {
	// Only inject if env_spec exists and injectLabels is true
	let Some(env) = env_spec else { return };
	if !env.spec.inject_labels.unwrap_or(false) {
		return;
	}

	// Generate the label value using SHA256 hash of "name:namespace"
	// This matches Tanka's NameLabel() implementation
	let label_value = generate_environment_label(env);

	// Inject the label
	if let JsonValue::Object(ref mut obj) = manifest {
		// Ensure metadata exists
		if !obj.contains_key("metadata") {
			obj.insert(
				"metadata".to_string(),
				JsonValue::Object(serde_json::Map::new()),
			);
		}

		if let Some(JsonValue::Object(ref mut metadata)) = obj.get_mut("metadata") {
			// Ensure labels exists
			if !metadata.contains_key("labels") {
				metadata.insert(
					"labels".to_string(),
					JsonValue::Object(serde_json::Map::new()),
				);
			}

			// Add the tanka.dev/environment label
			if let Some(JsonValue::Object(ref mut labels)) = metadata.get_mut("labels") {
				labels.insert(
					"tanka.dev/environment".to_string(),
					JsonValue::String(label_value),
				);
			}
		}
	}
}

/// Generate the tanka.dev/environment label value
/// This replicates Tanka's NameLabel() function which creates a SHA256 hash
/// of the environment's metadata.name and metadata.namespace
fn generate_environment_label(env: &crate::spec::Environment) -> String {
	use sha2::{Digest, Sha256};

	// By default, use metadata.name and metadata.namespace
	// Format: "name:namespace"
	let name = env.metadata.name.as_deref().unwrap_or("");
	let namespace = env.metadata.namespace.as_deref().unwrap_or("");
	let label_parts = format!("{}:{}", name, namespace);

	// Compute SHA256 hash
	let mut hasher = Sha256::new();
	hasher.update(label_parts.as_bytes());
	let result = hasher.finalize();

	// Convert to hex and take first 48 characters
	let hex = format!("{:x}", result);
	hex.chars().take(48).collect()
}

/// Sort all JSON object keys recursively to match Go's yaml.v3 output order
/// Go's yaml.v3 uses a "natural sort" algorithm (see sorter.go in gopkg.in/yaml.v3)
fn sort_json_keys(value: JsonValue) -> JsonValue {
	match value {
		JsonValue::Object(map) => {
			// Collect and sort keys using go-yaml v3's natural sort algorithm
			let mut entries: Vec<(String, JsonValue)> = map.into_iter().collect();
			entries.sort_by(|(a, _), (b, _)| yaml_v3_key_compare(a, b));

			// Rebuild with sorted keys, recursively sorting nested values
			let sorted: serde_json::Map<String, JsonValue> = entries
				.into_iter()
				.map(|(k, v)| (k, sort_json_keys(v)))
				.collect();
			JsonValue::Object(sorted)
		}
		JsonValue::Array(arr) => {
			// Recursively sort keys in array elements
			JsonValue::Array(arr.into_iter().map(sort_json_keys).collect())
		}
		// Primitive values remain unchanged
		other => other,
	}
}

/// Implements go-yaml v3's key comparison algorithm (from sorter.go)
/// This is a "natural sort" where:
/// - Numbers are sorted numerically
/// - Letters are sorted before non-letters when transitioning from digits
/// - Non-letters (like '_') are sorted before letters when not in digit context
fn yaml_v3_key_compare(a: &str, b: &str) -> std::cmp::Ordering {
	let ar: Vec<char> = a.chars().collect();
	let br: Vec<char> = b.chars().collect();
	let mut digits = false;

	let min_len = ar.len().min(br.len());
	for i in 0..min_len {
		if ar[i] == br[i] {
			digits = ar[i].is_ascii_digit();
			continue;
		}

		let al = ar[i].is_alphabetic();
		let bl = br[i].is_alphabetic();

		if al && bl {
			return ar[i].cmp(&br[i]);
		}

		if al || bl {
			// One is a letter, one is not
			if digits {
				// After digits: letters come first
				return if al {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Greater
				};
			} else {
				// Not after digits: non-letters come first
				return if bl {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Greater
				};
			}
		}

		// Both are non-letters - check for numeric sequences
		// Handle leading zeros
		let mut an: i64 = 0;
		let mut bn: i64 = 0;

		if ar[i] == '0' || br[i] == '0' {
			// Check if previous chars were non-zero digits
			let mut j = i;
			while j > 0 && ar[j - 1].is_ascii_digit() {
				j -= 1;
				if ar[j] != '0' {
					an = 1;
					bn = 1;
					break;
				}
			}
		}

		// Parse numeric sequences
		let mut ai = i;
		while ai < ar.len() && ar[ai].is_ascii_digit() {
			an = an * 10 + (ar[ai] as i64 - '0' as i64);
			ai += 1;
		}

		let mut bi = i;
		while bi < br.len() && br[bi].is_ascii_digit() {
			bn = bn * 10 + (br[bi] as i64 - '0' as i64);
			bi += 1;
		}

		if an != bn {
			return an.cmp(&bn);
		}
		if ai != bi {
			return ai.cmp(&bi);
		}
		return ar[i].cmp(&br[i]);
	}

	ar.len().cmp(&br.len())
}

/// Inject resourceDefaults (annotations, labels, etc.) into manifest metadata
/// This replicates Tanka's behavior for spec.resourceDefaults
fn inject_resource_defaults(manifest: &mut JsonValue, env_spec: &Option<crate::spec::Environment>) {
	let Some(env) = env_spec else { return };
	let Some(resource_defaults) = &env.spec.resource_defaults else {
		return;
	};

	// resource_defaults is a JSON object that can contain annotations, labels, etc.
	let JsonValue::Object(defaults) = resource_defaults else {
		return;
	};

	if let JsonValue::Object(ref mut obj) = manifest {
		// Ensure metadata exists
		if !obj.contains_key("metadata") {
			obj.insert(
				"metadata".to_string(),
				JsonValue::Object(serde_json::Map::new()),
			);
		}

		if let Some(JsonValue::Object(ref mut metadata)) = obj.get_mut("metadata") {
			// Process annotations from resourceDefaults
			if let Some(JsonValue::Object(default_annotations)) = defaults.get("annotations") {
				// Ensure annotations exists and is an object (not null)
				// Helm templates can produce `annotations:` with no value, which becomes null
				let needs_annotations = match metadata.get("annotations") {
					None => true,
					Some(JsonValue::Null) => true,
					Some(JsonValue::Object(m)) if m.is_empty() => false, // empty object is fine
					Some(JsonValue::Object(_)) => false,                 // existing object is fine
					_ => true,                                           // any other type, replace with object
				};
				if needs_annotations {
					metadata.insert(
						"annotations".to_string(),
						JsonValue::Object(serde_json::Map::new()),
					);
				}

				// Merge default annotations into manifest annotations
				// Don't override existing annotations
				if let Some(JsonValue::Object(ref mut annotations)) =
					metadata.get_mut("annotations")
				{
					for (key, value) in default_annotations {
						if !annotations.contains_key(key) {
							annotations.insert(key.clone(), value.clone());
						}
					}
				}
			}

			// Process labels from resourceDefaults
			if let Some(JsonValue::Object(default_labels)) = defaults.get("labels") {
				// Ensure labels exists and is an object (not null)
				// Helm templates can produce `labels:` with no value, which becomes null
				let needs_labels = match metadata.get("labels") {
					None => true,
					Some(JsonValue::Null) => true,
					Some(JsonValue::Object(m)) if m.is_empty() => false, // empty object is fine
					Some(JsonValue::Object(_)) => false,                 // existing object is fine
					_ => true,                                           // any other type, replace with object
				};
				if needs_labels {
					metadata.insert(
						"labels".to_string(),
						JsonValue::Object(serde_json::Map::new()),
					);
				}

				// Merge default labels into manifest labels
				// Don't override existing labels
				if let Some(JsonValue::Object(ref mut labels)) = metadata.get_mut("labels") {
					for (key, value) in default_labels {
						if !labels.contains_key(key) {
							labels.insert(key.clone(), value.clone());
						}
					}
				}
			}
		}
	}
}

/// Check if a JSON value is null or an empty object
fn is_null_or_empty_object(value: Option<&JsonValue>) -> bool {
	match value {
		Some(JsonValue::Null) => true,
		Some(JsonValue::Object(m)) if m.is_empty() => true,
		_ => false,
	}
}

/// Strip null or empty values from metadata.annotations and metadata.labels
/// This matches Tanka/Kubernetes behavior where null and empty fields are omitted from output
fn strip_null_metadata_fields(manifest: &mut JsonValue) {
	if let JsonValue::Object(ref mut obj) = manifest {
		if let Some(JsonValue::Object(ref mut metadata)) = obj.get_mut("metadata") {
			// Remove annotations if it's null or empty
			if is_null_or_empty_object(metadata.get("annotations")) {
				metadata.remove("annotations");
			}
			// Remove labels if it's null or empty
			if is_null_or_empty_object(metadata.get("labels")) {
				metadata.remove("labels");
			}
		}
	}
}

/// Sanitize a string for use as a path component
fn sanitize_path_component(s: &str) -> String {
	// Preserve <no value> exactly as tk outputs it for cluster-scoped resources
	if s == "<no value>" {
		return s.to_string();
	}
	s.chars()
		.map(|c| {
			if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ':' {
				c
			} else {
				'-'
			}
		})
		.collect()
}

/// Replace text only OUTSIDE of template action blocks ({{ }})
/// This matches Go Tanka's replaceTmplText in pkg/tanka/export.go
fn replace_tmpl_text(s: &str, old: &str, new: &str) -> String {
	let mut result = String::new();
	let mut remaining = s;

	while let Some(start) = remaining.find("{{") {
		// Find matching }}
		if let Some(end_offset) = remaining[start..].find("}}") {
			let end = start + end_offset + 2;

			// Text before {{ - replace in this part
			let text_before = &remaining[..start];
			result.push_str(&text_before.replace(old, new));

			// Template action {{ ... }} - keep as-is
			let action = &remaining[start..end];
			result.push_str(action);

			remaining = &remaining[end..];
		} else {
			// No matching }}, treat rest as text
			break;
		}
	}

	// Remaining text after last }} - replace in this part
	result.push_str(&remaining.replace(old, new));
	result
}

/// Apply template path processing matching Go Tanka's behavior:
/// 1. Replace / in rendered output with - (prevents accidental subdirs from values like apps/v1)
/// 2. Replace BEL_RUNE back to / (restores intentional subdirs from format)
fn apply_template_path_processing(rendered: &str) -> String {
	// First: replace / with - (this handles values like "apps/v1" -> "apps-v1")
	let with_dashes = rendered.replace('/', "-");
	// Second: replace BEL back to / (restores intentional subdirs)
	with_dashes.replace(BEL_RUNE, "/")
}

/// Convert serde_json::Value to gtmpl::Value
fn json_to_gtmpl(value: &JsonValue) -> Value {
	match value {
		JsonValue::Null => Value::Nil,
		JsonValue::Bool(b) => Value::Bool(*b),
		JsonValue::Number(n) => {
			if let Some(i) = n.as_i64() {
				Value::Number(i.into())
			} else if let Some(f) = n.as_f64() {
				Value::Number(f.into())
			} else {
				Value::Nil
			}
		}
		JsonValue::String(s) => Value::String(s.clone()),
		JsonValue::Array(arr) => Value::Array(arr.iter().map(json_to_gtmpl).collect()),
		JsonValue::Object(obj) => {
			let map: HashMap<String, Value> = obj
				.iter()
				.map(|(k, v)| (k.clone(), json_to_gtmpl(v)))
				.collect();
			Value::Map(map)
		}
	}
}

// No longer need env function - all env values are pre-substituted into template

/// Specialize template by substituting environment values (called once per environment)
/// This eliminates the need for env function calls and HashMap lookups during rendering
fn specialize_template_for_env(
	template: &str,
	env_spec: &Option<crate::spec::Environment>,
) -> Result<String> {
	use regex::Regex;

	let mut result = template.to_string();

	// Find all env.metadata.labels.X references in the template
	let label_pattern = Regex::new(r"env\.metadata\.labels\.(\w+)").unwrap();
	let all_label_refs: std::collections::HashSet<String> = label_pattern
		.captures_iter(template)
		.map(|cap| cap[1].to_string())
		.collect();

	if let Some(env) = env_spec {
		// Sort by length descending to avoid partial matches
		let mut sorted_refs: Vec<_> = all_label_refs.iter().collect();
		sorted_refs.sort_by(|a, b| b.len().cmp(&a.len()));

		let labels = env.metadata.labels.as_ref();

		for label_key in sorted_refs {
			let pattern = format!("env.metadata.labels.{}", label_key);
			let replacement = if let Some(label_map) = labels {
				if let Some(value) = label_map.get(label_key.as_str()) {
					// Label exists - use its value as a quoted string
					// All label values must be strings (not booleans) for template comparisons to work
					format!("\"{}\"", value)
				} else {
					// Label doesn't exist - use empty string (falsy in Go templates)
					// This ensures `not env.metadata.labels.X` evaluates to true (empty string is falsy)
					// and `if env.metadata.labels.X` evaluates to false
					"\"\"".to_string()
				}
			} else {
				// No labels at all - use empty string (falsy in Go templates)
				"\"\"".to_string()
			};
			result = result.replace(&pattern, &replacement);
		}

		// Replace env.spec.namespace
		let namespace = &env.spec.namespace;
		result = result.replace("env.spec.namespace", &format!("\"{}\"", namespace));

		// Replace env.metadata.name if present
		if let Some(name) = &env.metadata.name {
			result = result.replace("env.metadata.name", &format!("\"{}\"", name));
		} else {
			result = result.replace("env.metadata.name", "\"\"");
		}
	} else {
		// For None case, replace all env references with empty defaults
		result = result.replace("env.spec.namespace", "\"\"");
		result = result.replace("env.metadata.name", "\"\"");

		// Replace all label references with empty string (falsy in Go templates)
		for label_key in all_label_refs {
			let pattern = format!("env.metadata.labels.{}", label_key);
			result = result.replace(&pattern, "\"\"");
		}
	}

	// Replace / with BEL_RUNE in template TEXT (outside {{ }} blocks)
	// This preserves intentional subdirectory separators while allowing
	// values containing / (like apps/v1) to be replaced with - later
	let result = replace_tmpl_text(&result, "/", &BEL_RUNE.to_string());

	Ok(result)
}

// prepare_env_value removed - no longer needed since env values are pre-substituted

/// Render filename using a specialized template (no env context needed)
/// The template has already been specialized with environment values
fn render_filename_simple(
	tmpl: &gtmpl::Template,
	manifest: &JsonValue,
	env_spec: &Option<crate::spec::Environment>,
) -> Result<String> {
	use gtmpl::Context;

	// Create context with manifest fields
	// Ensure metadata.labels exists as empty object and inject namespace if needed
	let mut manifest_clone = manifest.clone();
	if let JsonValue::Object(ref mut obj) = manifest_clone {
		// Get kind and check if it's cluster-wide
		let kind = obj
			.get("kind")
			.and_then(|v| v.as_str())
			.unwrap_or("")
			.to_string();
		let is_cluster_wide = is_cluster_wide_kind(&kind);

		// Ensure metadata exists
		if !obj.contains_key("metadata") {
			obj.insert(
				"metadata".to_string(),
				JsonValue::Object(serde_json::Map::new()),
			);
		}

		if let Some(JsonValue::Object(ref mut metadata)) = obj.get_mut("metadata") {
			// Ensure labels exists as empty object if not present
			// This prevents template errors when accessing .metadata.labels.field
			if !metadata.contains_key("labels") {
				metadata.insert(
					"labels".to_string(),
					JsonValue::Object(serde_json::Map::new()),
				);
			}

			// Check for annotation override (tanka.dev/namespaced)
			let mut namespaced = !is_cluster_wide;
			if let Some(JsonValue::Object(annotations)) = metadata.get("annotations") {
				if let Some(JsonValue::String(ns_str)) = annotations.get("tanka.dev/namespaced") {
					namespaced = ns_str == "true";
				}
			}

			// Inject namespace if needed (matching Tanka's behavior)
			if namespaced {
				let has_namespace = metadata.contains_key("namespace")
					&& metadata
						.get("namespace")
						.and_then(|v| v.as_str())
						.map(|s| !s.is_empty())
						.unwrap_or(false);

				if !has_namespace {
					if let Some(env) = env_spec {
						if !env.spec.namespace.is_empty() {
							metadata.insert(
								"namespace".to_string(),
								JsonValue::String(env.spec.namespace.clone()),
							);
						}
					}
				}
			}
		}
	}

	// OPTIMIZATION: Only convert fields that templates actually use
	// Templates typically only access .kind, .metadata.name, .metadata.namespace, .metadata.labels
	// Converting the entire manifest is expensive and unnecessary
	let mut context_map = HashMap::new();
	if let JsonValue::Object(obj) = manifest_clone {
		// Only extract and convert the fields templates use
		if let Some(kind) = obj.get("kind") {
			context_map.insert("kind".to_string(), json_to_gtmpl(kind));
		}
		if let Some(metadata) = obj.get("metadata") {
			context_map.insert("metadata".to_string(), json_to_gtmpl(metadata));
		}
		// apiVersion might be used by some templates
		if let Some(api_version) = obj.get("apiVersion") {
			context_map.insert("apiVersion".to_string(), json_to_gtmpl(api_version));
		}
	}

	let context = Context::from(Value::Map(context_map));

	// Render template (env values are already baked into the template)
	let result = tmpl
		.render(&context)
		.map_err(|e| anyhow::anyhow!("Template error: {:?}", e))?;

	// Clean up empty segments (from missing optional fields)
	// Keep <no value> to match tk behavior for cluster-scoped resources
	let cleaned: String = result
		.split('/')
		.filter(|s| !s.is_empty())
		.collect::<Vec<_>>()
		.join("/");

	Ok(cleaned)
}

/// Render filename using a pre-parsed template (non-cached version for compatibility)
/// Used by tests and for single-template operations
fn render_filename_with_template(
	tmpl: &gtmpl::Template,
	manifest: &JsonValue,
	env_spec: &Option<crate::spec::Environment>,
) -> Result<String> {
	render_filename_simple(tmpl, manifest, env_spec)
}

/// Format filename using Go text/template (gtmpl) - legacy version that parses template
fn format_filename_gtmpl(
	manifest: &JsonValue,
	env_spec: &Option<crate::spec::Environment>,
	format: &str,
) -> Result<String> {
	use gtmpl::Template;

	// Specialize template with env values (also replaces / with BEL_RUNE in text)
	let specialized = specialize_template_for_env(format, env_spec)?;

	// Create and parse template (no env function needed - values are baked in)
	let mut tmpl = Template::default();
	// Register custom Sprig-compatible functions
	tmpl.add_func("default", tmpl_default);
	tmpl.parse(&specialized)?;

	// Use the optimized render function
	let rendered = render_filename_with_template(&tmpl, manifest, env_spec)?;

	// Apply Go Tanka's path processing:
	// 1. Replace / with - (prevents accidental subdirs from values like apps/v1)
	// 2. Replace BEL_RUNE back to / (restores intentional subdirs from format)
	Ok(apply_template_path_processing(&rendered))
}

/// Check if a directory is empty
fn is_dir_empty(dir: &PathBuf) -> Result<bool> {
	if !dir.exists() {
		return Ok(true);
	}

	let mut entries = fs::read_dir(dir)?;
	Ok(entries.next().is_none())
}

/// Collect the relative paths of files previously exported by the given environments
/// Returns a set of relative file paths (keys from manifest.json) that belong to these environments
fn collect_previously_exported_files(
	output_dir: &PathBuf,
	envs: &[DiscoveredEnv],
) -> Result<HashSet<String>> {
	// Collect environment identifiers
	let env_ids: Vec<String> = envs
		.iter()
		.map(|env| {
			let main_jsonnet_path = env.path.join("main.jsonnet");
			if let Ok(cwd) = std::env::current_dir() {
				main_jsonnet_path
					.strip_prefix(&cwd)
					.unwrap_or(&main_jsonnet_path)
					.to_string_lossy()
					.to_string()
			} else {
				main_jsonnet_path.to_string_lossy().to_string()
			}
		})
		.collect();

	collect_previously_exported_by_names(output_dir, &env_ids)
}

/// Collect the relative paths of files previously exported by environments with the given names
/// Returns a set of relative file paths (keys from manifest.json) that belong to these environments
fn collect_previously_exported_by_names(
	output_dir: &PathBuf,
	env_names: &[String],
) -> Result<HashSet<String>> {
	if env_names.is_empty() {
		return Ok(HashSet::new());
	}

	let manifest_path = output_dir.join(MANIFEST_FILE);
	if !manifest_path.exists() {
		// No manifest file, nothing to collect
		return Ok(HashSet::new());
	}

	// Read existing manifest
	let manifest_content =
		fs::read_to_string(&manifest_path).context("reading manifest.json for collection")?;
	let file_to_env: HashMap<String, String> =
		serde_json::from_str(&manifest_content).context("parsing manifest.json for collection")?;

	// Normalize environment names - convert to both absolute and relative forms
	let cwd = std::env::current_dir().ok();
	let mut normalized_names = std::collections::HashSet::new();
	for name in env_names {
		normalized_names.insert(name.clone());

		// Try to convert absolute to relative and vice versa
		let path = PathBuf::from(name);
		if path.is_absolute() {
			// Try to make relative to cwd
			if let Some(ref cwd_path) = cwd {
				if let Ok(rel) = path.strip_prefix(cwd_path) {
					normalized_names.insert(rel.to_string_lossy().to_string());
				}
			}
		} else {
			// Try to make absolute
			if let Some(ref cwd_path) = cwd {
				let abs = cwd_path.join(&path);
				normalized_names.insert(abs.to_string_lossy().to_string());
			}
		}
	}

	// Collect files belonging to these environments
	let mut collected_files = HashSet::new();
	for (file, env) in &file_to_env {
		// Check for exact match or prefix match (for inline sub-environments)
		// Also handle the case where user provides directory path but manifest has full main.jsonnet path
		let should_collect = normalized_names.contains(env)
			|| normalized_names.iter().any(|name| {
				// Match inline sub-envs like "path/to/env.jsonnet:subenv"
				env.starts_with(&format!("{}:", name))
					|| env.starts_with(&format!("{}:", name.trim_end_matches(".jsonnet")))
					// Match when user provides directory and manifest has full path
					|| *env == format!("{}/main.jsonnet", name)
					|| *env == format!("{}/main.jsonnet", name.trim_end_matches('/'))
			});

		if should_collect {
			collected_files.insert(file.clone());
		}
	}

	Ok(collected_files)
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	fn setup_test_env(temp: &TempDir, name: &str, content: &str) -> PathBuf {
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		let env_path = root.join(format!("environments/{}", name));
		fs::create_dir_all(&env_path).unwrap();
		fs::write(env_path.join("main.jsonnet"), content).unwrap();
		// Create spec.json to make this a static environment
		fs::write(
			env_path.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"test"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		env_path
	}

	#[test]
	fn test_validate_filename_template_valid() {
		// Default Go template format
		assert!(validate_filename_template(
			"{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}"
		)
		.is_ok());

		// Simple format
		assert!(validate_filename_template("{{.kind}}-{{.metadata.name}}").is_ok());

		// Just kind
		assert!(validate_filename_template("{{.kind}}").is_ok());
	}

	#[test]
	fn test_validate_filename_template_invalid() {
		// Invalid Go template syntax
		assert!(validate_filename_template("{{.invalid syntax").is_err());

		// Unclosed braces
		assert!(validate_filename_template("{{.kind}").is_err());
	}

	#[test]
	fn test_format_filename_gtmpl_basic() {
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "my-config" }
		});

		let result =
			format_filename_gtmpl(&manifest, &None, "{{.kind}}-{{.metadata.name}}").unwrap();
		assert_eq!(result, "ConfigMap-my-config");
	}

	#[test]
	fn test_format_filename_gtmpl_with_or() {
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "my-config" }
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.kind}}-{{or .metadata.name .metadata.generateName}}",
		)
		.unwrap();
		assert_eq!(result, "ConfigMap-my-config");
	}

	#[test]
	fn test_format_filename_gtmpl_with_or_fallback() {
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "Pod",
			"metadata": { "generateName": "job-" }
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.kind}}-{{or .metadata.name .metadata.generateName}}",
		)
		.unwrap();
		assert_eq!(result, "Pod-job-");
	}

	#[test]
	fn test_format_filename_gtmpl_full_default() {
		let manifest = serde_json::json!({
			"apiVersion": "apps/v1",
			"kind": "Deployment",
			"metadata": { "name": "nginx" }
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}",
		)
		.unwrap();
		// apiVersion apps/v1 becomes apps-v1 (/ replaced with -)
		assert_eq!(result, "apps-v1.Deployment-nginx");
	}

	#[test]
	fn test_format_filename_gtmpl_with_default() {
		// Test the default function (Sprig-compatible)
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test-config"
				// Note: no namespace field
			}
		});

		// Template uses default to provide a fallback value
		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.kind}}-{{.metadata.namespace | default \"global\"}}",
		)
		.unwrap();
		// Since namespace is missing, should use "global" as default
		assert_eq!(result, "ConfigMap-global");
	}

	#[test]
	fn test_format_filename_gtmpl_with_default_non_empty() {
		// Test that default returns the value when it's non-empty
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test-config",
				"namespace": "prod"
			}
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.kind}}-{{.metadata.namespace | default \"global\"}}",
		)
		.unwrap();
		// Since namespace exists, should use "prod"
		assert_eq!(result, "ConfigMap-prod");
	}

	#[test]
	fn test_json_to_gtmpl_object() {
		let json = serde_json::json!({
			"name": "test",
			"count": 42,
			"enabled": true
		});

		let gtmpl_val = json_to_gtmpl(&json);
		assert!(matches!(gtmpl_val, Value::Map(_)));
	}

	#[test]
	fn test_json_to_gtmpl_nested() {
		let json = serde_json::json!({
			"metadata": {
				"name": "test"
			}
		});

		let gtmpl_val = json_to_gtmpl(&json);
		if let Value::Map(map) = gtmpl_val {
			assert!(map.contains_key("metadata"));
		} else {
			panic!("Expected Map");
		}
	}

	#[test]
	fn test_extract_manifests_single() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test" }
		});

		let manifests = extract_manifests(&value).unwrap();
		assert_eq!(manifests.len(), 1);
	}

	#[test]
	fn test_extract_manifests_nested() {
		let value = serde_json::json!({
			"configmap": {
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "test" }
			},
			"service": {
				"apiVersion": "v1",
				"kind": "Service",
				"metadata": { "name": "test-svc" }
			}
		});

		let manifests = extract_manifests(&value).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_extract_manifests_array() {
		let value = serde_json::json!([
			{
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "test1" }
			},
			{
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "test2" }
			}
		]);

		let manifests = extract_manifests(&value).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_sanitize_path_component() {
		assert_eq!(sanitize_path_component("hello-world"), "hello-world");
		assert_eq!(sanitize_path_component("hello/world"), "hello-world");
		assert_eq!(sanitize_path_component("hello:world"), "hello:world");
		assert_eq!(sanitize_path_component("my_app"), "my_app");
		// Colons should be preserved (e.g., for RoleBinding names like "system:leader-locking-vpa-recommender")
		assert_eq!(
			sanitize_path_component("RoleBinding-system:leader-locking-vpa-recommender"),
			"RoleBinding-system:leader-locking-vpa-recommender"
		);
	}

	#[test]
	fn test_replace_tmpl_text_basic() {
		// Replace / with BEL only outside {{ }}
		// a/b/ is text before {{.x}}, /c is text after {{.x}}
		// Both get replaced
		let result = replace_tmpl_text("a/b/{{.x}}/c", "/", "\x07");
		assert_eq!(result, "a\x07b\x07{{.x}}\x07c");
	}

	#[test]
	fn test_replace_tmpl_text_multiple_actions() {
		let result = replace_tmpl_text("{{.a}}/{{.b}}/{{.c}}", "/", "\x07");
		// / between actions gets replaced, / inside actions stays
		assert_eq!(result, "{{.a}}\x07{{.b}}\x07{{.c}}");
	}

	#[test]
	fn test_replace_tmpl_text_no_actions() {
		let result = replace_tmpl_text("a/b/c", "/", "-");
		assert_eq!(result, "a-b-c");
	}

	#[test]
	fn test_replace_tmpl_text_preserves_action_content() {
		// The / inside {{ }} should NOT be replaced
		let result = replace_tmpl_text("prefix/{{.apiVersion}}/suffix", "/", "\x07");
		assert_eq!(result, "prefix\x07{{.apiVersion}}\x07suffix");
	}

	#[test]
	fn test_apply_template_path_processing() {
		// Test: / becomes -, BEL becomes /
		let result = apply_template_path_processing("apps/v1.Deployment-nginx");
		assert_eq!(result, "apps-v1.Deployment-nginx");

		// Test: intentional subdir (BEL in input)
		let result = apply_template_path_processing("namespace\x07apps/v1.Deployment-nginx");
		assert_eq!(result, "namespace/apps-v1.Deployment-nginx");
	}

	#[test]
	fn test_intentional_subdirectory_in_template() {
		// When / is in the template FORMAT (not values), it creates subdirectories
		let manifest = serde_json::json!({
			"apiVersion": "apps/v1",
			"kind": "Deployment",
			"metadata": {
				"name": "nginx",
				"namespace": "production"
			}
		});

		// Template with / in format text (between {{ }} blocks) creates subdirs
		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.metadata.namespace}}/{{.apiVersion}}.{{.kind}}-{{.metadata.name}}",
		)
		.unwrap();
		// namespace/apiVersion.Kind-name where / in namespace is intentional (subdir)
		// and / in apps/v1 is replaced with -
		assert_eq!(result, "production/apps-v1.Deployment-nginx");
	}

	#[test]
	fn test_export_simple_env() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "test-config" },
				data: { key: "value" }
			}"#,
		);

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			extension: "yaml".to_string(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			parallelism: 1,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: true, // Allow single env
			skip_manifest: false,
			..Default::default()
		};

		let result = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		assert_eq!(result.total_envs, 1);
		assert_eq!(result.successful, 1);
		assert_eq!(result.failed, 0);
	}

	#[test]
	fn test_export_json_format() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "test-config" },
				data: { key: "value" }
			}"#,
		);

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			extension: "json".to_string(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			parallelism: 1,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: true,
			skip_manifest: false,
			..Default::default()
		};

		let result = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		assert_eq!(result.total_envs, 1);
		assert_eq!(result.successful, 1);

		// Verify file has .json extension
		let files: Vec<_> = result.results[0].files_written.iter().collect();
		assert!(files.iter().any(|f| f.extension().unwrap() == "json"));
	}

	#[test]
	fn test_export_empty_paths() {
		let opts = ExportOpts::default();
		let result = export(&[], opts).unwrap();

		assert_eq!(result.total_envs, 0);
		assert_eq!(result.successful, 0);
		assert_eq!(result.failed, 0);
	}

	#[test]
	fn test_export_multiple_manifests() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"multi",
			r#"{
				configmap: {
					apiVersion: "v1",
					kind: "ConfigMap",
					metadata: { name: "config" },
					data: { key: "value" }
				},
				deployment: {
					apiVersion: "apps/v1",
					kind: "Deployment",
					metadata: { name: "app" },
					spec: {}
				}
			}"#,
		);

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			extension: "yaml".to_string(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			parallelism: 1,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: true,
			skip_manifest: false,
			..Default::default()
		};

		let result = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		assert_eq!(result.total_envs, 1);
		assert_eq!(result.successful, 1);
		// Should have 2 files (one per manifest)
		assert_eq!(result.results[0].files_written.len(), 2);
	}

	#[test]
	fn test_extract_manifests_deeply_nested() {
		let value = serde_json::json!({
			"level1": {
				"level2": {
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "nested" }
				}
			}
		});

		let manifests = extract_manifests(&value).unwrap();
		assert_eq!(manifests.len(), 1);
	}

	#[test]
	fn test_extract_manifests_mixed() {
		// Mix of direct manifests and nested ones
		let value = serde_json::json!({
			"direct": {
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "direct" }
			},
			"nested": {
				"inner": {
					"apiVersion": "v1",
					"kind": "Secret",
					"metadata": { "name": "nested" }
				}
			}
		});

		let manifests = extract_manifests(&value).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_sanitize_path_special_chars() {
		assert_eq!(sanitize_path_component("a/b\\c:d"), "a-b-c:d");
		assert_eq!(sanitize_path_component("test..path"), "test..path");
		assert_eq!(
			sanitize_path_component("normal-name_123"),
			"normal-name_123"
		);
	}

	#[test]
	fn test_export_opts_default() {
		let opts = ExportOpts::default();
		assert_eq!(opts.extension, "yaml");
		assert_eq!(opts.parallelism, 8);
		assert_eq!(
			opts.format,
			"{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}"
		);
	}

	#[test]
	fn test_export_multi_env_without_recursive_fails() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create two environments
		let env1 = root.join("environments/env1");
		let env2 = root.join("environments/env2");
		fs::create_dir_all(&env1).unwrap();
		fs::create_dir_all(&env2).unwrap();
		fs::write(
			env1.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "c1" } }"#,
		)
		.unwrap();
		fs::write(
			env1.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env1"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();
		fs::write(
			env2.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "c2" } }"#,
		)
		.unwrap();
		fs::write(
			env2.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env2"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			recursive: false, // Not recursive
			name: None,       // No name filter
			..Default::default()
		};

		// Should fail with multiple environments
		let result = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		);
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Found 2 environments"));
	}

	#[test]
	fn test_export_multi_env_with_recursive_succeeds() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create two environments
		let env1 = root.join("environments/env1");
		let env2 = root.join("environments/env2");
		fs::create_dir_all(&env1).unwrap();
		fs::create_dir_all(&env2).unwrap();
		fs::write(
			env1.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "c1" } }"#,
		)
		.unwrap();
		fs::write(
			env1.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env1"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();
		fs::write(
			env2.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "c2" } }"#,
		)
		.unwrap();
		fs::write(
			env2.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env2"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true, // Recursive mode
			..Default::default()
		};

		let result = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		);
		assert!(result.is_ok());
		let result = result.unwrap();
		assert_eq!(result.total_envs, 2);
		assert_eq!(result.successful, 2);
	}

	// ==================== ISSUE 1: Go Template Compatibility Tests ====================

	#[test]
	fn test_gtmpl_nested_field_access() {
		// Test deeply nested field access like {{.metadata.labels.app}}
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "Service",
			"metadata": {
				"name": "my-service",
				"labels": {
					"app": "nginx",
					"tier": "frontend"
				}
			}
		});

		let result =
			format_filename_gtmpl(&manifest, &None, "{{.metadata.labels.app}}-{{.kind}}").unwrap();
		assert_eq!(result, "nginx-Service");
	}

	#[test]
	fn test_gtmpl_or_with_missing_first_field() {
		// Test {{or .a .b}} when first field is missing
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "Job",
			"metadata": {
				"generateName": "batch-job-"
			}
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.kind}}-{{or .metadata.name .metadata.generateName}}",
		)
		.unwrap();
		assert_eq!(result, "Job-batch-job-");
	}

	#[test]
	fn test_gtmpl_or_with_both_fields_present() {
		// Test {{or .a .b}} when both fields exist (should use first)
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "Pod",
			"metadata": {
				"name": "my-pod",
				"generateName": "should-not-use-"
			}
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.kind}}-{{or .metadata.name .metadata.generateName}}",
		)
		.unwrap();
		assert_eq!(result, "Pod-my-pod");
	}

	#[test]
	fn test_gtmpl_apiversion_with_slash() {
		// Test apiVersion like "apps/v1" or "networking.k8s.io/v1"
		// The / in values gets replaced with - to prevent accidental subdirectories
		let manifest = serde_json::json!({
			"apiVersion": "networking.k8s.io/v1",
			"kind": "Ingress",
			"metadata": { "name": "my-ingress" }
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.apiVersion}}.{{.kind}}-{{.metadata.name}}",
		)
		.unwrap();
		// / in apiVersion becomes - (matching Go Tanka behavior)
		assert_eq!(result, "networking.k8s.io-v1.Ingress-my-ingress");
	}

	#[test]
	fn test_gtmpl_special_characters_in_name() {
		// Test names with special characters
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "my-config-map.v2" }
		});

		let result =
			format_filename_gtmpl(&manifest, &None, "{{.kind}}-{{.metadata.name}}").unwrap();
		assert_eq!(result, "ConfigMap-my-config-map.v2");
	}

	#[test]
	fn test_gtmpl_default_tanka_format() {
		// Test the exact default format Tanka uses
		let manifest = serde_json::json!({
			"apiVersion": "apps/v1",
			"kind": "Deployment",
			"metadata": { "name": "nginx-deployment" }
		});

		let result = format_filename_gtmpl(
			&manifest,
			&None,
			"{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}",
		)
		.unwrap();
		// / in apiVersion becomes - (matching Go Tanka behavior)
		assert_eq!(result, "apps-v1.Deployment-nginx-deployment");
	}

	// ==================== ISSUE 2: Fail-Fast Validation Tests ====================

	#[test]
	fn test_fail_fast_invalid_template_before_processing() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create a valid environment
		let env = root.join("environments/test");
		fs::create_dir_all(&env).unwrap();
		fs::write(
			env.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "test" } }"#,
		)
		.unwrap();

		// Use invalid template syntax
		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.invalid syntax".to_string(), // Invalid!
			recursive: true,
			..Default::default()
		};

		let result = export(&[env.to_string_lossy().to_string()], opts);

		// Should fail with template error, not evaluation error
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(
			err.contains("template") || err.contains("Template"),
			"Error should mention template: {}",
			err
		);
	}

	#[test]
	fn test_fail_fast_unclosed_braces() {
		// gtmpl should reject obviously broken templates
		assert!(validate_filename_template("{{.kind}").is_err());
		assert!(validate_filename_template("{{.kind").is_err());
		// Note: {.kind}} is technically valid (literal "{" + ".kind}}")
	}

	#[test]
	fn test_fail_fast_valid_complex_templates() {
		// Various valid Go template patterns
		assert!(validate_filename_template("{{.apiVersion}}").is_ok());
		assert!(validate_filename_template("{{.kind}}-{{.metadata.name}}").is_ok());
		assert!(validate_filename_template("{{or .metadata.name .metadata.generateName}}").is_ok());
		assert!(validate_filename_template(
			"{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}"
		)
		.is_ok());
	}

	// ==================== ISSUE 4: Multi-Environment Check Tests ====================

	#[test]
	fn test_multi_env_with_name_filter_succeeds() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create multiple environments
		let env1 = root.join("environments/prod-env");
		let env2 = root.join("environments/staging-env");
		fs::create_dir_all(&env1).unwrap();
		fs::create_dir_all(&env2).unwrap();
		fs::write(
			env1.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "prod" } }"#,
		)
		.unwrap();
		fs::write(
			env1.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"prod-env"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();
		fs::write(
			env2.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "staging" } }"#,
		)
		.unwrap();
		fs::write(
			env2.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"staging-env"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		// Use name filter to select only prod
		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			name: Some("prod".to_string()),
			recursive: false, // Not recursive, but name filter should work
			..Default::default()
		};

		let result = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		);
		assert!(result.is_ok());
		let result = result.unwrap();
		assert_eq!(result.total_envs, 1);
		assert_eq!(result.successful, 1);
	}

	#[test]
	fn test_multi_env_name_filter_no_match() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create environments
		let env1 = root.join("environments/prod");
		let env2 = root.join("environments/staging");
		fs::create_dir_all(&env1).unwrap();
		fs::create_dir_all(&env2).unwrap();
		fs::write(
			env1.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "c1" } }"#,
		)
		.unwrap();
		fs::write(
			env1.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"prod"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();
		fs::write(
			env2.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "c2" } }"#,
		)
		.unwrap();
		fs::write(
			env2.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"staging"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		// Use name filter that matches nothing
		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			name: Some("nonexistent".to_string()),
			recursive: false,
			..Default::default()
		};

		let result = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("No environments found"));
	}

	#[test]
	fn test_single_env_without_recursive_succeeds() {
		// Single environment should work without --recursive
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"single",
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "test" } }"#,
		);

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: false, // Not recursive
			name: None,
			..Default::default()
		};

		let result = export(&[env_path.to_string_lossy().to_string()], opts);
		assert!(result.is_ok());
		assert_eq!(result.unwrap().successful, 1);
	}

	// ==================== ISSUE 3 & 5: Error Handling Tests ====================

	#[test]
	fn test_export_continues_on_per_env_errors() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create one valid and one invalid environment
		let valid_env = root.join("environments/valid");
		let invalid_env = root.join("environments/invalid");
		fs::create_dir_all(&valid_env).unwrap();
		fs::create_dir_all(&invalid_env).unwrap();

		fs::write(
			valid_env.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "valid" } }"#,
		)
		.unwrap();
		fs::write(
			valid_env.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"valid"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();
		// Invalid jsonnet syntax
		fs::write(invalid_env.join("main.jsonnet"), r#"{ invalid jsonnet }"#).unwrap();
		fs::write(
			invalid_env.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"invalid"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true,
			parallelism: 1, // Single thread to ensure predictable order
			..Default::default()
		};

		let result = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		)
		.unwrap();

		// Should have processed both, with one failure
		assert_eq!(result.total_envs, 2);
		assert_eq!(result.successful, 1);
		assert_eq!(result.failed, 1);
	}

	#[test]
	fn test_export_result_contains_error_details() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create invalid environment
		let invalid_env = root.join("environments/broken");
		fs::create_dir_all(&invalid_env).unwrap();
		fs::write(invalid_env.join("main.jsonnet"), r#"syntax error here {"#).unwrap();
		fs::write(
			invalid_env.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"broken"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true,
			..Default::default()
		};

		let result = export(&[invalid_env.to_string_lossy().to_string()], opts).unwrap();

		assert_eq!(result.failed, 1);
		assert!(result.results[0].error.is_some());
		// Error message should contain useful info
		let error_msg = result.results[0].error.as_ref().unwrap();
		assert!(!error_msg.is_empty());
	}

	// ==================== JSON to GTMPL Conversion Tests ====================

	#[test]
	fn test_json_to_gtmpl_all_types() {
		// Test all JSON types convert correctly
		let json = serde_json::json!({
			"string": "hello",
			"number_int": 42,
			"number_float": 3.5,
			"boolean": true,
			"null_value": null,
			"array": [1, 2, 3],
			"nested": {
				"key": "value"
			}
		});

		let gtmpl_val = json_to_gtmpl(&json);
		assert!(matches!(gtmpl_val, Value::Map(_)));

		if let Value::Map(map) = gtmpl_val {
			assert!(matches!(map.get("string"), Some(Value::String(_))));
			assert!(matches!(map.get("number_int"), Some(Value::Number(_))));
			assert!(matches!(map.get("boolean"), Some(Value::Bool(true))));
			assert!(matches!(map.get("null_value"), Some(Value::Nil)));
			assert!(matches!(map.get("array"), Some(Value::Array(_))));
			assert!(matches!(map.get("nested"), Some(Value::Map(_))));
		}
	}

	#[test]
	fn test_json_to_gtmpl_empty_values() {
		let json = serde_json::json!({
			"empty_string": "",
			"empty_array": [],
			"empty_object": {}
		});

		let gtmpl_val = json_to_gtmpl(&json);
		assert!(matches!(gtmpl_val, Value::Map(_)));
	}

	// ==================== Edge Cases ====================

	#[test]
	fn test_export_env_with_no_manifests() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp, "empty", r#"{}"#, // Empty object, no K8s manifests
		);

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true,
			..Default::default()
		};

		let result = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		assert_eq!(result.successful, 1);
		assert_eq!(result.results[0].files_written.len(), 0);
	}

	#[test]
	fn test_extract_manifests_tanka_environment_wrapper() {
		// Test extracting from Tanka Environment wrapper object
		let value = serde_json::json!({
			"apiVersion": "tanka.dev/v1alpha1",
			"kind": "Environment",
			"metadata": { "name": "prod" },
			"data": {
				"configmap": {
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "app-config" }
				}
			}
		});

		let manifests = extract_manifests(&value).unwrap();
		assert_eq!(manifests.len(), 1);
		assert_eq!(manifests[0]["kind"], "ConfigMap");
	}

	#[test]
	fn test_sanitize_path_unicode() {
		// Note: Rust's is_alphanumeric() returns true for Unicode letters like é, ö
		// This is actually correct behavior - these are valid in paths
		assert_eq!(sanitize_path_component("héllo-wörld"), "héllo-wörld");
		// CJK characters are also alphanumeric in Unicode
		assert_eq!(sanitize_path_component("日本語"), "日本語");
		// But special chars like emojis should be replaced
		assert_eq!(sanitize_path_component("test🚀name"), "test-name");
	}

	#[test]
	fn test_sanitize_path_preserves_valid_chars() {
		// Valid chars: alphanumeric, -, _, .
		assert_eq!(
			sanitize_path_component("Valid-Name_123.yaml"),
			"Valid-Name_123.yaml"
		);
	}

	#[test]
	fn test_count_environment_objects_single() {
		let value = serde_json::json!({
			"apiVersion": "tanka.dev/v1alpha1",
			"kind": "Environment",
			"metadata": { "name": "prod" }
		});
		assert_eq!(count_environment_objects(&value), 1);
	}

	#[test]
	fn test_count_environment_objects_multiple() {
		let value = serde_json::json!({
			"env1": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "prod" }
			},
			"env2": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "staging" }
			}
		});
		assert_eq!(count_environment_objects(&value), 2);
	}

	#[test]
	fn test_count_environment_objects_nested() {
		let value = serde_json::json!({
			"level1": {
				"level2": {
					"apiVersion": "tanka.dev/v1alpha1",
					"kind": "Environment",
					"metadata": { "name": "nested" }
				}
			}
		});
		assert_eq!(count_environment_objects(&value), 1);
	}

	#[test]
	fn test_count_environment_objects_none() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test" }
		});
		assert_eq!(count_environment_objects(&value), 0);
	}

	#[test]
	fn test_count_environment_objects_in_array() {
		let value = serde_json::json!([
			{
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "env1" }
			},
			{
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "env2" }
			}
		]);
		assert_eq!(count_environment_objects(&value), 2);
	}

	// ==================== Additional Edge Case Tests ====================

	#[test]
	fn test_export_with_recursive_flag_processes_all() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create 3 environments
		for i in 1..=3 {
			let env = root.join(format!("environments/env{}", i));
			fs::create_dir_all(&env).unwrap();
			fs::write(
				env.join("main.jsonnet"),
				format!(
					r#"{{ apiVersion: "v1", kind: "ConfigMap", metadata: {{ name: "config{}" }} }}"#,
					i
				),
			)
			.unwrap();
			fs::write(
				env.join("spec.json"),
				format!(
					r#"{{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{{"name":"env{}"}},"spec":{{"namespace":"default"}}}}"#,
					i
				),
			)
			.unwrap();
		}

		let opts = ExportOpts {
			output_dir: temp.path().join("output"),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true,
			..Default::default()
		};

		let result = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		)
		.unwrap();

		assert_eq!(result.total_envs, 3);
		assert_eq!(result.successful, 3);
		assert_eq!(result.failed, 0);
	}

	#[test]
	fn test_export_parallel_produces_consistent_results() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create multiple environments
		for i in 1..=5 {
			let env = root.join(format!("environments/env{}", i));
			fs::create_dir_all(&env).unwrap();
			fs::write(
				env.join("main.jsonnet"),
				format!(
					r#"{{ apiVersion: "v1", kind: "ConfigMap", metadata: {{ name: "config{}" }} }}"#,
					i
				),
			)
			.unwrap();
			fs::write(
				env.join("spec.json"),
				format!(
					r#"{{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{{"name":"env{}"}},"spec":{{"namespace":"default"}}}}"#,
					i
				),
			)
			.unwrap();
		}

		// Run with different parallelism levels
		for parallelism in [1, 2, 4] {
			let output_dir = temp.path().join(format!("output-p{}", parallelism));
			let opts = ExportOpts {
				output_dir: output_dir.clone(),
				format: "{{.kind}}-{{.metadata.name}}".to_string(),
				recursive: true,
				parallelism,
				..Default::default()
			};

			let result = export(
				&[root.join("environments").to_string_lossy().to_string()],
				opts,
			)
			.unwrap();

			assert_eq!(
				result.total_envs, 5,
				"parallelism {} should find all envs",
				parallelism
			);
			assert_eq!(
				result.successful, 5,
				"parallelism {} should succeed for all",
				parallelism
			);
		}
	}

	#[test]
	fn test_gtmpl_empty_metadata_name() {
		// Test when metadata.name is empty string
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "" }
		});

		// Should not fail, just produce empty segment
		let result = format_filename_gtmpl(&manifest, &None, "{{.kind}}-{{.metadata.name}}");
		assert!(result.is_ok());
	}

	#[test]
	fn test_gtmpl_missing_metadata() {
		// Test when metadata is completely missing
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap"
		});

		let result = format_filename_gtmpl(&manifest, &None, "{{.kind}}");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "ConfigMap");
	}

	#[test]
	fn test_export_creates_nested_directories() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "test-config", namespace: "my-namespace" },
				data: { key: "value" }
			}"#,
		);

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			format: "{{.metadata.namespace}}/{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true,
			..Default::default()
		};

		let result = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		assert_eq!(result.successful, 1);
		// Verify nested directory was created (namespace becomes part of path after sanitization)
		let files = &result.results[0].files_written;
		assert_eq!(files.len(), 1);
	}

	#[test]
	fn test_export_yaml_vs_json_extension() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "test" },
				data: { key: "value" }
			}"#,
		);

		// Test YAML
		let yaml_output = temp.path().join("yaml-output");
		let yaml_opts = ExportOpts {
			output_dir: yaml_output.clone(),
			extension: "yaml".to_string(),
			format: "{{.kind}}".to_string(),
			recursive: true,
			..Default::default()
		};
		let yaml_result = export(&[env_path.to_string_lossy().to_string()], yaml_opts).unwrap();
		assert!(yaml_result.results[0].files_written[0]
			.to_string_lossy()
			.ends_with(".yaml"));

		// Test JSON
		let json_output = temp.path().join("json-output");
		let json_opts = ExportOpts {
			output_dir: json_output.clone(),
			extension: "json".to_string(),
			format: "{{.kind}}".to_string(),
			recursive: true,
			..Default::default()
		};
		let json_result = export(&[env_path.to_string_lossy().to_string()], json_opts).unwrap();
		assert!(json_result.results[0].files_written[0]
			.to_string_lossy()
			.ends_with(".json"));
	}

	#[test]
	fn test_gtmpl_env_function() {
		// Test that env variable works in templates
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config" }
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "prod-cluster".to_string());
		labels.insert("team".to_string(), "platform".to_string());

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// Test accessing env.metadata.labels
		let result = format_filename_gtmpl(
			&manifest,
			&env,
			"{{env.metadata.labels.cluster_name}}/{{.kind}}-{{.metadata.name}}",
		)
		.unwrap();
		assert_eq!(result, "prod-cluster/ConfigMap-test-config");

		// Test accessing env.metadata.name
		let result2 =
			format_filename_gtmpl(&manifest, &env, "{{env.metadata.name}}/{{.kind}}").unwrap();
		assert_eq!(result2, "test-env/ConfigMap");
	}

	#[test]
	fn test_gtmpl_env_with_conditional() {
		// Test the complex template from tk-compare with conditional logic
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config" }
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "prod-cluster".to_string());
		labels.insert("fluxExport".to_string(), "true".to_string());
		labels.insert("fluxExportDir".to_string(), "test-dir".to_string());

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// Test the actual tk-compare template format (simplified)
		let template = "{{ if not env.metadata.labels.fluxExport }}flux{{ else }}flux-enabled{{ end }}/{{.kind}}";
		let result = format_filename_gtmpl(&manifest, &env, template);

		// This should work - env.metadata.labels.fluxExport exists and is "true" (non-empty string)
		// In Go templates, non-empty strings are truthy, so "not env.metadata.labels.fluxExport" should be false
		assert!(
			result.is_ok(),
			"Template with 'not env.metadata.labels' should work: {:?}",
			result
		);
	}

	#[test]
	fn test_gtmpl_env_inline_environment() {
		// Test that env works even when env_spec is None (inline environments)
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config" }
		});

		// No env_spec (inline environment)
		let env = None;

		// Template tries to access env fields - should not error even with no env
		let template = "{{ if env.metadata.labels.cluster_name }}{{ env.metadata.labels.cluster_name }}{{ else }}default{{ end }}/{{.kind}}";
		let result = format_filename_gtmpl(&manifest, &env, template);

		// Should work with default empty env structure
		assert!(
			result.is_ok(),
			"Template with env should work for inline environments: {:?}",
			result
		);
		// Should fall back to "default" since env.metadata.labels.cluster_name is empty
		assert_eq!(result.unwrap(), "default/ConfigMap");
	}

	#[test]
	fn test_gtmpl_env_missing_label_with_eq_comparison() {
		// Regression test: missing labels should be replaced with empty string ""
		// This ensures comparisons like {{ if eq env.metadata.labels.X "true" }} work correctly
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "Service",
			"metadata": {
				"name": "my-service",
				"namespace": "production"
			}
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "test-cluster".to_string());
		// Intentionally NOT setting namespaceToExportFilenames label

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// This template uses a label that doesn't exist in the environment
		// The bug was that missing labels were replaced with "", causing template parse errors
		let template = r#"{{.kind}}-{{ if eq env.metadata.labels.namespaceToExportFilenames "true" }}{{ .metadata.namespace | default "global" }}-{{ end }}{{.metadata.name}}"#;

		// This should NOT panic or return an error
		let result = format_filename_gtmpl(&manifest, &env, template).unwrap();

		// Since namespaceToExportFilenames is missing (replaced with empty string),
		// the eq comparison should be false, so the conditional block is skipped
		assert_eq!(result, "Service-my-service");
	}

	#[test]
	fn test_flux_export_label_missing() {
		// Test case: fluxExport label is not set (missing)
		// Expected: outputs to "flux-disabled/" directory
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config", "namespace": "default" }
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "test-cluster".to_string());
		// NOT setting fluxExport label

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// Real tk-compare template
		let template = r#"{{ if not env.metadata.labels.fluxExport }}flux-disabled{{ else if eq env.metadata.labels.fluxExport "true" }}flux{{ else }}flux-disabled{{ end }}/{{.kind}}"#;
		let result = format_filename_gtmpl(&manifest, &env, template).unwrap();

		// Missing label → empty string → `not ""` = true → first branch
		assert_eq!(result, "flux-disabled/ConfigMap");
	}

	#[test]
	fn test_flux_export_label_true() {
		// Test case: fluxExport label is set to "true"
		// Expected: outputs to "flux/" directory
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config", "namespace": "default" }
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "test-cluster".to_string());
		labels.insert("fluxExport".to_string(), "true".to_string());

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// Real tk-compare template
		let template = r#"{{ if not env.metadata.labels.fluxExport }}flux-disabled{{ else if eq env.metadata.labels.fluxExport "true" }}flux{{ else }}flux-disabled{{ end }}/{{.kind}}"#;
		let result = format_filename_gtmpl(&manifest, &env, template).unwrap();

		// Label is "true" → `not "true"` = false → check second condition → `eq "true" "true"` = true → second branch
		assert_eq!(result, "flux/ConfigMap");
	}

	#[test]
	fn test_flux_export_label_false() {
		// Test case: fluxExport label is explicitly set to "false"
		// Expected: outputs to "flux-disabled/" directory
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config", "namespace": "default" }
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "test-cluster".to_string());
		labels.insert("fluxExport".to_string(), "false".to_string());

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// Real tk-compare template
		let template = r#"{{ if not env.metadata.labels.fluxExport }}flux-disabled{{ else if eq env.metadata.labels.fluxExport "true" }}flux{{ else }}flux-disabled{{ end }}/{{.kind}}"#;
		let result = format_filename_gtmpl(&manifest, &env, template).unwrap();

		// Label is "false" (string) → `not "false"` = false → check second condition → `eq "false" "true"` = false → else branch
		assert_eq!(result, "flux-disabled/ConfigMap");
	}

	#[test]
	fn test_flux_export_label_other_value() {
		// Test case: fluxExport label is set to some other value (e.g., "disabled")
		// Expected: outputs to "flux-disabled/" directory
		use std::collections::BTreeMap;

		use crate::spec::{Environment, Metadata, Spec};

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config", "namespace": "default" }
		});

		let mut labels = BTreeMap::new();
		labels.insert("cluster_name".to_string(), "test-cluster".to_string());
		labels.insert("fluxExport".to_string(), "disabled".to_string());

		let env = Some(Environment {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("default".to_string()),
				labels: Some(labels),
			},
			spec: Spec {
				api_server: None,
				context_names: None,
				namespace: "default".to_string(),
				diff_strategy: None,
				apply_strategy: None,
				inject_labels: None,
				resource_defaults: None,
				expect_versions: None,
				export_jsonnet_implementation: None,
			},
			data: None,
		});

		// Real tk-compare template
		let template = r#"{{ if not env.metadata.labels.fluxExport }}flux-disabled{{ else if eq env.metadata.labels.fluxExport "true" }}flux{{ else }}flux-disabled{{ end }}/{{.kind}}"#;
		let result = format_filename_gtmpl(&manifest, &env, template).unwrap();

		// Label is "disabled" → `not "disabled"` = false → check second condition → `eq "disabled" "true"` = false → else branch
		assert_eq!(result, "flux-disabled/ConfigMap");
	}

	#[test]
	fn test_flux_export_no_env() {
		// Test case: no environment spec at all (inline environment)
		// Expected: outputs to "flux-disabled/" directory
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test-config", "namespace": "default" }
		});

		let env = None;

		// Real tk-compare template
		let template = r#"{{ if not env.metadata.labels.fluxExport }}flux-disabled{{ else if eq env.metadata.labels.fluxExport "true" }}flux{{ else }}flux-disabled{{ end }}/{{.kind}}"#;
		let result = format_filename_gtmpl(&manifest, &env, template).unwrap();

		// No env → label replaced with "" → `not ""` = true → first branch
		assert_eq!(result, "flux-disabled/ConfigMap");
	}

	#[test]
	fn test_gtmpl_env_complex_tk_compare_template() {
		// Test the full tk-compare template with inline environment (None)
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test-config",
				"namespace": "default"
			}
		});

		let env = None; // Inline environment

		// The actual tk-compare template (simplified version)
		let template = "{{ if env.metadata.labels.fluxExport }}flux/{{ env.metadata.labels.cluster_name }}{{ else }}flux{{ end }}/{{ if .metadata.namespace }}{{.metadata.namespace}}{{ else }}_cluster{{ end }}/{{.kind}}-{{.metadata.name}}";
		let result = format_filename_gtmpl(&manifest, &env, template);

		assert!(
			result.is_ok(),
			"Complex tk-compare template should work: {:?}",
			result
		);
		// Since env has no fluxExport label, should use "flux" prefix and default namespace
		let output = result.unwrap();
		assert!(output.contains("flux/"), "Should contain flux prefix");
		assert!(output.contains("default/"), "Should contain namespace");
		assert!(
			output.contains("ConfigMap-test-config"),
			"Should contain kind and name"
		);
	}

	// ==================== Manifest.json Tests ====================

	#[test]
	fn test_manifest_json_generated() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "test-config" },
				data: { key: "value" }
			}"#,
		);

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			extension: "yaml".to_string(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			parallelism: 1,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: true,
			skip_manifest: false,
			..Default::default()
		};

		let _ = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		// Check manifest.json exists
		let manifest_path = output_dir.join(MANIFEST_FILE);
		assert!(manifest_path.exists(), "manifest.json should exist");

		// Read and verify contents
		let manifest_content = fs::read_to_string(&manifest_path).unwrap();
		let manifest_map: HashMap<String, String> =
			serde_json::from_str(&manifest_content).unwrap();

		// Should have one entry
		assert_eq!(manifest_map.len(), 1);

		// Entry should map the file to the environment path
		let expected_file = "ConfigMap-test-config.yaml";
		assert!(
			manifest_map.contains_key(expected_file),
			"manifest.json should contain the exported file"
		);
	}

	#[test]
	fn test_manifest_json_skipped() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "test-config" },
				data: { key: "value" }
			}"#,
		);

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			extension: "yaml".to_string(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			parallelism: 1,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: true,
			skip_manifest: true,
			..Default::default()
		};

		let _ = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		// Check manifest.json does not exist
		let manifest_path = output_dir.join(MANIFEST_FILE);
		assert!(
			!manifest_path.exists(),
			"manifest.json should not exist when skip_manifest is true"
		);
	}

	#[test]
	fn test_manifest_json_merges_with_existing() {
		let temp = TempDir::new().unwrap();
		let output_dir = temp.path().join("output");
		fs::create_dir_all(&output_dir).unwrap();

		// Create existing manifest.json
		let existing_manifest = serde_json::json!({
			"old-file.yaml": "old-env"
		});
		fs::write(
			output_dir.join(MANIFEST_FILE),
			serde_json::to_string_pretty(&existing_manifest).unwrap(),
		)
		.unwrap();

		// Export a new environment
		let env_path = setup_test_env(
			&temp,
			"test",
			r#"{
				apiVersion: "v1",
				kind: "ConfigMap",
				metadata: { name: "new-config" },
				data: { key: "value" }
			}"#,
		);

		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			extension: "yaml".to_string(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			parallelism: 1,
			eval_opts: EvalOpts::default(),
			name: None,
			recursive: true,
			skip_manifest: false,
			merge_strategy: ExportMergeStrategy::FailOnConflicts, // Allow exporting to non-empty dir
			..Default::default()
		};

		let _ = export(&[env_path.to_string_lossy().to_string()], opts).unwrap();

		// Read manifest.json
		let manifest_content = fs::read_to_string(output_dir.join(MANIFEST_FILE)).unwrap();
		let manifest_map: HashMap<String, String> =
			serde_json::from_str(&manifest_content).unwrap();

		// Should have both entries
		assert_eq!(manifest_map.len(), 2);
		assert!(manifest_map.contains_key("old-file.yaml"));
		assert!(manifest_map.contains_key("ConfigMap-new-config.yaml"));
	}

	#[test]
	fn test_manifest_json_with_multiple_envs() {
		let temp = TempDir::new().unwrap();
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), "{}").unwrap();

		// Create two environments
		let env1 = root.join("environments/env1");
		let env2 = root.join("environments/env2");
		fs::create_dir_all(&env1).unwrap();
		fs::create_dir_all(&env2).unwrap();
		fs::write(
			env1.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "ConfigMap", metadata: { name: "config1" } }"#,
		)
		.unwrap();
		fs::write(
			env1.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env1"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();
		fs::write(
			env2.join("main.jsonnet"),
			r#"{ apiVersion: "v1", kind: "Secret", metadata: { name: "secret1" } }"#,
		)
		.unwrap();
		fs::write(
			env2.join("spec.json"),
			r#"{"apiVersion":"tanka.dev/v1alpha1","kind":"Environment","metadata":{"name":"env2"},"spec":{"namespace":"default"}}"#,
		)
		.unwrap();

		let output_dir = temp.path().join("output");
		let opts = ExportOpts {
			output_dir: output_dir.clone(),
			format: "{{.kind}}-{{.metadata.name}}".to_string(),
			recursive: true,
			skip_manifest: false,
			..Default::default()
		};

		let _ = export(
			&[root.join("environments").to_string_lossy().to_string()],
			opts,
		)
		.unwrap();

		// Read manifest.json
		let manifest_path = output_dir.join(MANIFEST_FILE);
		assert!(manifest_path.exists());

		let manifest_content = fs::read_to_string(&manifest_path).unwrap();
		let manifest_map: HashMap<String, String> =
			serde_json::from_str(&manifest_content).unwrap();

		// Should have entries for both environments
		assert_eq!(manifest_map.len(), 2);
		assert!(manifest_map.contains_key("ConfigMap-config1.yaml"));
		assert!(manifest_map.contains_key("Secret-secret1.yaml"));
	}

	#[test]
	fn test_extract_environments_deduplicates_by_name() {
		// Create a JSON structure with duplicate Environment objects (same name at different paths)
		let value = serde_json::json!({
			"env1": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "my-env" },
				"spec": { "namespace": "default" },
				"data": {
					"cm": {
						"apiVersion": "v1",
						"kind": "ConfigMap",
						"metadata": { "name": "config1" }
					}
				}
			},
			"env2": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "my-env" },  // Same name as env1
				"spec": { "namespace": "default" },
				"data": {
					"cm": {
						"apiVersion": "v1",
						"kind": "ConfigMap",
						"metadata": { "name": "config2" }
					}
				}
			}
		});

		let environments = extract_environments(&value, &None).unwrap();

		// Should deduplicate to just one environment (first one wins)
		assert_eq!(environments.len(), 1);
		assert_eq!(
			environments[0]
				.spec
				.as_ref()
				.unwrap()
				.metadata
				.name
				.as_deref(),
			Some("my-env")
		);
	}

	#[test]
	fn test_extract_environments_keeps_different_names() {
		// Create a JSON structure with two Environment objects with different names
		let value = serde_json::json!({
			"env1": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "env-a" },
				"spec": { "namespace": "default" },
				"data": {}
			},
			"env2": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "env-b" },
				"spec": { "namespace": "default" },
				"data": {}
			}
		});

		let environments = extract_environments(&value, &None).unwrap();

		// Should keep both environments since they have different names
		assert_eq!(environments.len(), 2);
	}
}
