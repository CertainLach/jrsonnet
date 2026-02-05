//! Diff command handler.
//!
//! Compares local Tanka environment manifests against the live Kubernetes cluster state.

use std::{fmt, io::Write};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::util::{
	build_eval_opts, create_tokio_runtime, extract_manifests, get_or_create_connection,
	parse_key_value_pairs, process_manifests, JsonnetArgs,
};
use crate::{
	discover::find_environments,
	eval::EvalOpts,
	k8s::{
		client::ClusterConnection,
		diff::{DiffEngine, DiffStatus, ResourceDiff},
		output::DiffOutput,
	},
	spec::DiffStrategy,
};

/// Color output mode for diff display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
	/// Color if stdout is a TTY.
	#[default]
	Auto,

	/// Always emit ANSI color codes.
	Always,

	/// No colors (plain text).
	Never,
}

impl ColorMode {
	/// Determine if colors should be used based on mode and terminal detection.
	pub fn should_colorize(&self) -> bool {
		match self {
			ColorMode::Auto => std::io::IsTerminal::is_terminal(&std::io::stdout()),
			ColorMode::Always => true,
			ColorMode::Never => false,
		}
	}
}

impl fmt::Display for ColorMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ColorMode::Auto => write!(f, "auto"),
			ColorMode::Always => write!(f, "always"),
			ColorMode::Never => write!(f, "never"),
		}
	}
}

/// Exit code when differences are found (matches tk behavior).
pub const EXIT_CODE_DIFF_FOUND: i32 = 16;

#[derive(Args)]
pub struct DiffArgs {
	/// Path to the Tanka environment
	pub path: String,

	/// Color output mode
	#[arg(long, default_value = "auto", value_enum)]
	pub color: ColorMode,

	/// Force the diff-strategy to use. Automatically chosen if not set.
	#[arg(long, value_enum)]
	pub diff_strategy: Option<DiffStrategy>,

	/// Exit with 0 even when differences are found
	#[arg(short = 'z', long)]
	pub exit_zero: bool,

	/// Set code value of extVar (Format: key=<code>)
	#[arg(long)]
	pub ext_code: Vec<String>,

	/// Set string value of extVar (Format: key=value)
	#[arg(short = 'V', long)]
	pub ext_str: Vec<String>,

	/// Use `go` to use native go-jsonnet implementation and `binary:<path>` to delegate evaluation to a binary (with the same API as the regular `jsonnet` binary)
	#[arg(long, default_value = "go")]
	pub jsonnet_implementation: String,

	/// Jsonnet VM max stack. Increase this if you get: max stack frames exceeded
	#[arg(long, default_value = "500")]
	pub max_stack: i32,

	/// String that only a single inline environment contains in its name
	#[arg(long)]
	pub name: Option<String>,

	/// Print summary of the differences, not the actual contents
	#[arg(short = 's', long)]
	pub summarize: bool,

	/// Regex filter on '<kind>/<name>'. See https://tanka.dev/output-filtering
	#[arg(short = 't', long)]
	pub target: Vec<String>,

	/// Set code value of top level function (Format: key=<code>)
	#[arg(long)]
	pub tla_code: Vec<String>,

	/// Set string value of top level function (Format: key=value)
	#[arg(short = 'A', long)]
	pub tla_str: Vec<String>,

	/// Include objects deleted from the configuration in the differences
	#[arg(short = 'p', long)]
	pub with_prune: bool,

	/// List environments with changes
	#[arg(long)]
	pub list_modified_envs: bool,
}

impl JsonnetArgs for DiffArgs {
	fn ext_str(&self) -> &[String] {
		&self.ext_str
	}
	fn ext_code(&self) -> &[String] {
		&self.ext_code
	}
	fn tla_str(&self) -> &[String] {
		&self.tla_str
	}
	fn tla_code(&self) -> &[String] {
		&self.tla_code
	}
	fn max_stack(&self) -> i32 {
		self.max_stack
	}
	fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}
}

/// Result of running the diff command.
struct DiffResult {
	/// Whether any differences were found.
	has_changes: bool,
}

/// Run the diff command.
///
/// Returns `Ok(true)` if differences were found and `--exit-zero` was not passed,
/// indicating the caller should exit with `EXIT_CODE_DIFF_FOUND`.
pub fn run<W: Write>(args: DiffArgs, writer: W) -> Result<bool> {
	let exit_zero = args.exit_zero;
	let list_modified_envs = args.list_modified_envs;

	let runtime = create_tokio_runtime()?;
	let result = runtime.block_on(run_async(args, writer))?;

	// --list-modified-envs always exits 0
	if list_modified_envs {
		return Ok(false);
	}

	// Return whether we should exit with non-zero code
	// (has changes AND --exit-zero was not passed)
	Ok(result.has_changes && !exit_zero)
}

/// Options for running a diff operation.
#[derive(Default, bon::Builder)]
pub struct DiffOpts {
	/// Diff strategy to use.
	pub strategy: Option<DiffStrategy>,
	/// Whether to include pruned resources.
	#[builder(default)]
	pub with_prune: bool,
	/// Color output mode.
	#[builder(default)]
	pub color: ColorMode,
	/// Whether to print summary instead of full diff.
	#[builder(default)]
	pub summarize: bool,
	/// Target filters.
	#[builder(default)]
	pub target: Vec<String>,
	/// Filter environments by name (exact match first, then substring).
	pub name: Option<String>,
}

/// Run diff on an environment path against cluster state.
///
/// Evaluates the Jsonnet environment, extracts manifests, and compares them
/// against the current state in the connected cluster. If no connection is
/// provided, one is created from the environment's spec.
#[instrument(skip_all, fields(path = %path))]
pub async fn diff_environment<W: Write>(
	path: &str,
	connection: Option<ClusterConnection>,
	eval_opts: EvalOpts,
	opts: DiffOpts,
	writer: W,
) -> Result<Vec<ResourceDiff>> {
	use super::util::evaluate_single_environment;

	let env_data = evaluate_single_environment(path, eval_opts, opts.name.as_deref())?;
	let env_spec = env_data.spec;

	// Get the spec for cluster connection and strategy selection
	let spec = env_spec.as_ref().map(|e| &e.spec);

	// Extract manifests from environment data
	let mut manifests = extract_manifests(&env_data.data, &opts.target)?;
	tracing::debug!(manifest_count = manifests.len(), "found manifests to diff");

	if manifests.is_empty() {
		tracing::warn!("no manifests found in environment");
		return Ok(Vec::new());
	}

	process_manifests(&mut manifests, &env_spec);

	let connection = get_or_create_connection(connection, spec).await?;

	diff_manifests(manifests, connection, env_spec.as_ref(), opts, writer).await
}

/// Run diff on manifests against cluster state.
///
/// Compares the provided manifests against the current state in the connected
/// cluster, returning the differences for each resource.
#[instrument(skip_all, fields(manifest_count = manifests.len()))]
pub async fn diff_manifests<W: Write>(
	manifests: Vec<serde_json::Value>,
	connection: ClusterConnection,
	env_spec: Option<&crate::spec::Environment>,
	opts: DiffOpts,
	mut writer: W,
) -> Result<Vec<ResourceDiff>> {
	let spec = env_spec.map(|e| &e.spec);

	// Determine diff strategy
	let strategy = opts.strategy.unwrap_or_else(|| {
		if let Some(s) = spec {
			DiffStrategy::from_spec(s, connection.server_version())
		} else {
			DiffStrategy::Native
		}
	});
	tracing::debug!(strategy = %strategy, "using diff strategy");

	// Get default namespace from spec or connection
	let default_namespace = spec
		.map(|s| s.namespace.clone())
		.unwrap_or_else(|| connection.default_namespace().to_string());

	// Create diff engine
	let engine = DiffEngine::new(
		connection,
		strategy,
		default_namespace,
		&manifests,
		opts.with_prune,
	)
	.await
	.context("creating diff engine")?;

	// Get environment label for prune detection (SHA256 hash of name:namespace)
	let env_label_owned = env_spec.map(crate::spec::generate_environment_label);
	let env_label = env_label_owned.as_deref();

	// Check if inject_labels is enabled (required for prune detection)
	let inject_labels = env_spec.and_then(|e| e.spec.inject_labels).unwrap_or(false);

	// Compute diffs
	tracing::debug!("computing differences");
	let diffs = engine
		.diff_all(&manifests, opts.with_prune, env_label, inject_labels)
		.await
		.context("computing diffs")?;

	// Output results if writer is provided
	let has_changes = diffs.iter().any(|d| d.has_changes());
	let mut output = DiffOutput::new(&mut writer, opts.color, strategy)?;

	if opts.summarize {
		output.write_summary(&diffs)?;
	} else {
		for diff in &diffs {
			if diff.status != DiffStatus::Unchanged {
				output.write_diff(diff)?;
			}
		}

		if !has_changes {
			eprintln!("No differences.");
		}
	}

	Ok(diffs)
}

/// Async implementation of the diff command.
#[instrument(skip_all, fields(path = %args.path))]
async fn run_async<W: Write>(args: DiffArgs, writer: W) -> Result<DiffResult> {
	// Handle --list-modified-envs mode: find all environments and check each for changes
	if args.list_modified_envs {
		return list_modified_environments(&args, &mut std::io::sink()).await;
	}

	let opts = DiffOpts {
		strategy: args.diff_strategy,
		with_prune: args.with_prune,
		color: args.color,
		summarize: args.summarize,
		target: args.target.clone(),
		name: args.name.clone(),
	};

	let diffs = diff_environment(&args.path, None, build_eval_opts(&args), opts, writer).await?;
	let has_changes = diffs.iter().any(|d| d.has_changes());

	Ok(DiffResult { has_changes })
}

/// List environments that have changes.
///
/// Discovers all environments in the path, checks each for changes in parallel,
/// and prints the names of environments with differences.
#[instrument(skip_all, fields(path = %args.path))]
async fn list_modified_environments<W: Write>(
	args: &DiffArgs,
	writer: &mut W,
) -> Result<DiffResult> {
	// Discover all environments in the path
	tracing::debug!(path = %args.path, "discovering environments");
	let envs =
		find_environments(std::slice::from_ref(&args.path)).context("discovering environments")?;

	// Filter environments by --name if specified
	let envs: Vec<_> = if let Some(ref target_name) = args.name {
		// First try exact match on env_name
		let exact: Vec<_> = envs
			.iter()
			.filter(|e| e.env_name.as_deref() == Some(target_name.as_str()))
			.cloned()
			.collect();

		if !exact.is_empty() {
			exact
		} else {
			// Fall back to substring match
			envs.into_iter()
				.filter(|e| {
					e.env_name
						.as_ref()
						.map(|n| n.contains(target_name))
						.unwrap_or(false)
				})
				.collect()
		}
	} else {
		envs
	};

	if envs.is_empty() {
		eprintln!("No environments with changes.");
		return Ok(DiffResult { has_changes: false });
	}

	tracing::debug!(env_count = envs.len(), "found environments");

	// Build shared eval options from args
	let ext_str = parse_key_value_pairs(&args.ext_str);
	let ext_code = parse_key_value_pairs(&args.ext_code);
	let tla_str = parse_key_value_pairs(&args.tla_str);
	let tla_code = parse_key_value_pairs(&args.tla_code);

	// Check all environments in parallel using JoinSet with concurrency limit
	const MAX_PARALLEL: usize = 8;
	let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_PARALLEL));
	let mut join_set = tokio::task::JoinSet::new();

	for env in &envs {
		let env_path = env.path.to_string_lossy().to_string();
		let display_name = env
			.env_name
			.as_ref()
			.map(|n| n.to_string())
			.unwrap_or_else(|| env_path.clone());

		let eval_opts = EvalOpts {
			ext_str: ext_str.clone(),
			ext_code: ext_code.clone(),
			tla_str: tla_str.clone(),
			tla_code: tla_code.clone(),
			max_stack: Some(args.max_stack as usize),
			eval_expr: None,
			env_name: env.env_name.clone(),
			export_jsonnet_implementation: None,
		};

		let diff_strategy = args.diff_strategy;
		let with_prune = args.with_prune;
		let target = args.target.clone();
		let sem = semaphore.clone();

		join_set.spawn(async move {
			let _permit = sem.acquire().await.expect("semaphore closed");
			tracing::debug!(env_path = %env_path, "checking environment");
			match check_environment_for_changes(
				env_path.clone(),
				eval_opts,
				diff_strategy,
				with_prune,
				target,
			)
			.await
			{
				Ok(true) => Some(display_name),
				Ok(false) => {
					tracing::debug!(env_path = %env_path, "no changes");
					None
				}
				Err(e) => {
					tracing::warn!(env_path = %env_path, error = %e, "failed to check environment");
					None
				}
			}
		});
	}

	let mut changed_envs = Vec::new();
	while let Some(result) = join_set.join_next().await {
		if let Ok(Some(name)) = result {
			changed_envs.push(name);
		}
	}

	// Print results
	if changed_envs.is_empty() {
		eprintln!("No environments with changes.");
		Ok(DiffResult { has_changes: false })
	} else {
		changed_envs.sort();
		for name in &changed_envs {
			writeln!(writer, "{}", name)?;
		}
		Ok(DiffResult { has_changes: true })
	}
}

/// Check if a single environment has changes.
#[instrument(skip_all, fields(path = %path))]
async fn check_environment_for_changes(
	path: String,
	eval_opts: EvalOpts,
	diff_strategy: Option<DiffStrategy>,
	with_prune: bool,
	target: Vec<String>,
) -> Result<bool> {
	// Evaluate the environment
	let eval_result = crate::eval::eval(&path, eval_opts).context("evaluating environment")?;

	let spec = eval_result.spec.as_ref().map(|e| &e.spec);

	// Extract manifests
	let mut manifests = extract_manifests(&eval_result.value, &target)?;
	if manifests.is_empty() {
		return Ok(false);
	}

	// Inject tanka.dev/environment label if injectLabels is enabled
	for manifest in &mut manifests {
		crate::spec::inject_environment_label(manifest, &eval_result.spec);
	}

	// Connect to the cluster
	let spec_for_connection = spec.cloned().unwrap_or_default();
	let connection = ClusterConnection::from_spec(&spec_for_connection).await?;

	// Determine diff strategy
	let strategy = diff_strategy.unwrap_or_else(|| {
		if let Some(s) = spec {
			DiffStrategy::from_spec(s, connection.server_version())
		} else {
			DiffStrategy::Native
		}
	});

	// Get default namespace
	let default_namespace = spec
		.map(|s| s.namespace.clone())
		.unwrap_or_else(|| connection.default_namespace().to_string());

	// Create diff engine
	let engine = DiffEngine::new(
		connection,
		strategy,
		default_namespace,
		&manifests,
		with_prune,
	)
	.await?;

	// Get environment label for prune detection
	let env_label_owned = eval_result
		.spec
		.as_ref()
		.map(crate::spec::generate_environment_label);
	let env_label = env_label_owned.as_deref();

	let inject_labels = eval_result
		.spec
		.as_ref()
		.and_then(|e| e.spec.inject_labels)
		.unwrap_or(false);

	// Compute diffs
	let diffs = engine
		.diff_all(&manifests, with_prune, env_label, inject_labels)
		.await?;

	// Check if any resource has changes
	Ok(diffs.iter().any(|d| d.has_changes()))
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Helper to extract (kind, name) pairs from manifests for structural comparison.
	fn manifest_ids(manifests: &[serde_json::Value]) -> Vec<(&str, &str)> {
		let mut ids: Vec<_> = manifests
			.iter()
			.map(|m| {
				let kind = m.get("kind").and_then(|v| v.as_str()).unwrap_or("");
				let name = m
					.pointer("/metadata/name")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				(kind, name)
			})
			.collect();
		ids.sort();
		ids
	}

	#[test]
	fn test_extract_manifests_single() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test"
			}
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifest_ids(&manifests), vec![("ConfigMap", "test")]);
	}

	#[test]
	fn test_extract_manifests_array() {
		let value = serde_json::json!([
			{
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "cm1" }
			},
			{
				"apiVersion": "v1",
				"kind": "Secret",
				"metadata": { "name": "secret1" }
			}
		]);

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(
			manifest_ids(&manifests),
			vec![("ConfigMap", "cm1"), ("Secret", "secret1")]
		);
	}

	#[test]
	fn test_extract_manifests_nested() {
		let value = serde_json::json!({
			"configs": {
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "nested" }
			},
			"deployments": {
				"apiVersion": "apps/v1",
				"kind": "Deployment",
				"metadata": { "name": "deploy" }
			}
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(
			manifest_ids(&manifests),
			vec![("ConfigMap", "nested"), ("Deployment", "deploy")]
		);
	}

	#[test]
	fn test_extract_manifests_list() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "List",
			"items": [
				{
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "cm1" }
				},
				{
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "cm2" }
				}
			]
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(
			manifest_ids(&manifests),
			vec![("ConfigMap", "cm1"), ("ConfigMap", "cm2")]
		);
	}

	#[test]
	fn test_extract_manifests_with_filter() {
		let value = serde_json::json!([
			{
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "cm1" }
			},
			{
				"apiVersion": "v1",
				"kind": "Secret",
				"metadata": { "name": "secret1" }
			},
			{
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "cm2" }
			}
		]);

		let manifests = extract_manifests(&value, &["ConfigMap/.*".to_string()]).unwrap();
		assert_eq!(
			manifest_ids(&manifests),
			vec![("ConfigMap", "cm1"), ("ConfigMap", "cm2")]
		);
	}

	#[test]
	fn test_build_eval_opts() {
		let args = DiffArgs {
			path: "test".to_string(),
			color: ColorMode::Auto,
			diff_strategy: None,
			exit_zero: false,
			ext_code: vec!["code1={}".to_string()],
			ext_str: vec!["str1=value1".to_string()],
			jsonnet_implementation: "go".to_string(),
			max_stack: 500,
			name: Some("my-env".to_string()),
			summarize: false,
			target: vec![],
			tla_code: vec!["tla1=true".to_string()],
			tla_str: vec!["tla2=hello".to_string()],
			with_prune: false,
			list_modified_envs: false,
		};

		let opts = build_eval_opts(&args);
		assert_eq!(
			opts,
			crate::eval::EvalOpts {
				ext_str: [("str1".to_string(), "value1".to_string())]
					.into_iter()
					.collect(),
				ext_code: [("code1".to_string(), "{}".to_string())]
					.into_iter()
					.collect(),
				tla_str: [("tla2".to_string(), "hello".to_string())]
					.into_iter()
					.collect(),
				tla_code: [("tla1".to_string(), "true".to_string())]
					.into_iter()
					.collect(),
				max_stack: Some(500),
				eval_expr: None,
				env_name: Some("my-env".to_string()),
				export_jsonnet_implementation: None,
			}
		);
	}
}
