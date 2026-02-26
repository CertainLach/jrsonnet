//! Utilities for command handlers.

use std::{
	collections::HashMap,
	fmt,
	io::{self, ErrorKind, Write},
};

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
	eval::EvalOpts,
	k8s::{client::ClusterConnection, diff::DiffEngine},
	spec::{DiffStrategy, Environment, EnvironmentData, Spec},
};

/// Warn about unimplemented CLI arguments that are accepted for Tanka compatibility
/// but don't do anything in Rustanka.
pub struct UnimplementedArgs<'a> {
	pub jsonnet_implementation: Option<&'a str>,
	pub cache_envs: Option<&'a [String]>,
	pub cache_path: Option<&'a Option<String>>,
	pub mem_ballast_size_bytes: Option<&'a Option<i64>>,
}

impl<'a> UnimplementedArgs<'a> {
	/// Log warnings for any unimplemented arguments that were provided.
	pub fn warn_if_set(&self) {
		if let Some(impl_str) = self.jsonnet_implementation {
			if impl_str != "go" {
				warn!(
					"--jsonnet-implementation is unimplemented in rtk and has no effect; \
					 rtk always uses the built-in jrsonnet evaluator"
				);
			}
		}

		if let Some(envs) = self.cache_envs {
			if !envs.is_empty() {
				warn!("--cache-envs is unimplemented in rtk and has no effect");
			}
		}

		if let Some(Some(_)) = self.cache_path {
			warn!("--cache-path is unimplemented in rtk and has no effect");
		}

		if let Some(Some(_)) = self.mem_ballast_size_bytes {
			warn!("--mem-ballast-size-bytes is unimplemented in rtk and has no effect");
		}
	}

	/// Convenience method to warn only about jsonnet_implementation.
	///
	/// Most commands only have the jsonnet_implementation flag as an unimplemented
	/// option. This helper avoids the boilerplate of constructing the full struct.
	pub fn warn_jsonnet_impl(jsonnet_implementation: &str) {
		UnimplementedArgs {
			jsonnet_implementation: Some(jsonnet_implementation),
			cache_envs: None,
			cache_path: None,
			mem_ballast_size_bytes: None,
		}
		.warn_if_set();
	}
}

/// A writer wrapper that silently handles broken pipe errors.
///
/// When the underlying writer returns a broken pipe error (EPIPE), this wrapper
/// converts it to a successful write. This allows commands to exit cleanly when
/// output is piped to a process that closes early (e.g., `rtk eval . | head -1`).
pub struct BrokenPipeGuard<W> {
	inner: W,
}

impl<W> BrokenPipeGuard<W> {
	pub fn new(inner: W) -> Self {
		Self { inner }
	}
}

impl<W: Write> Write for BrokenPipeGuard<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match self.inner.write(buf) {
			Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(buf.len()),
			other => other,
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		match self.inner.flush() {
			Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
			other => other,
		}
	}
}

/// Parse key=value pairs into a HashMap.
pub fn parse_key_value_pairs(items: &[String]) -> HashMap<String, String> {
	items
		.iter()
		.filter_map(|s| {
			s.split_once('=')
				.map(|(k, v)| (k.to_string(), v.to_string()))
		})
		.collect()
}

/// Extract Kubernetes manifests from the evaluation result.
///
/// The evaluation result can be:
/// - A single manifest object
/// - An array of manifests
/// - A nested object containing manifests (Tanka environment format)
pub fn extract_manifests(
	value: &serde_json::Value,
	target_filters: &[String],
) -> Result<Vec<serde_json::Value>> {
	let mut manifests = Vec::new();
	collect_manifests(value, &mut manifests);

	// Apply target filters if specified
	if !target_filters.is_empty() {
		let filters: Vec<regex::Regex> = target_filters
			.iter()
			.map(|f| regex::RegexBuilder::new(f).case_insensitive(true).build())
			.collect::<Result<Vec<_>, _>>()
			.context("invalid target filter regex")?;

		manifests.retain(|m| {
			let kind = m.get("kind").and_then(|v| v.as_str()).unwrap_or("");
			let name = m
				.pointer("/metadata/name")
				.and_then(|v| v.as_str())
				.unwrap_or("");
			let target = format!("{}/{}", kind, name);

			filters.iter().any(|f| f.is_match(&target))
		});
	}

	Ok(manifests)
}

/// Recursively collect Kubernetes manifests from a JSON value.
pub fn collect_manifests(value: &serde_json::Value, manifests: &mut Vec<serde_json::Value>) {
	match value {
		serde_json::Value::Object(map) => {
			// Check if this looks like a Kubernetes manifest
			if map.contains_key("apiVersion") && map.contains_key("kind") {
				// Check if it's a List
				if map.get("kind").and_then(|v| v.as_str()) == Some("List") {
					if let Some(items) = map.get("items").and_then(|v| v.as_array()) {
						for item in items {
							collect_manifests(item, manifests);
						}
					}
				} else {
					// Regular manifest
					manifests.push(value.clone());
				}
			} else {
				// Nested object - recurse into values
				for (_, v) in map {
					collect_manifests(v, manifests);
				}
			}
		}
		serde_json::Value::Array(arr) => {
			for item in arr {
				collect_manifests(item, manifests);
			}
		}
		_ => {
			// Ignore primitives
		}
	}
}

/// Get the name of an environment, if available.
pub fn env_name(env_data: &EnvironmentData) -> Option<&str> {
	env_data
		.spec
		.as_ref()
		.and_then(|s| s.metadata.name.as_deref())
}

/// Format a list of environment names for error messages.
pub fn get_environment_names(environments: &[EnvironmentData]) -> String {
	let names: Vec<&str> = environments.iter().filter_map(env_name).collect();
	if names.is_empty() {
		"(unnamed environments)".to_string()
	} else {
		names.join(", ")
	}
}

/// Filter environments by name, trying exact match first, then substring.
pub fn filter_environments_by_name(
	environments: Vec<EnvironmentData>,
	target_name: &str,
) -> Result<Vec<EnvironmentData>> {
	// Try exact match first
	let exact: Vec<_> = environments
		.iter()
		.filter(|e| env_name(e) == Some(target_name))
		.cloned()
		.collect();

	if let [_single] = exact.as_slice() {
		return Ok(exact);
	}

	// Fall back to substring matching
	let matches: Vec<_> = environments
		.into_iter()
		.filter(|e| env_name(e).is_some_and(|n| n.contains(target_name)))
		.collect();

	if matches.is_empty() {
		anyhow::bail!("no environment found matching name '{}'", target_name);
	}

	Ok(matches)
}

/// Prompt the user for confirmation with a custom prompt.
pub fn prompt_confirmation(prompt: &str) -> Result<bool> {
	eprint!("\n{} [y/N]: ", prompt);
	std::io::stderr().flush()?;

	let mut input = String::new();
	std::io::stdin().read_line(&mut input)?;

	let input = input.trim().to_lowercase();
	Ok(input == "y" || input == "yes")
}

/// Auto-approve settings for apply and prune commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoApprove {
	/// Always require manual approval.
	#[default]
	Never,

	/// Always auto-approve without prompting.
	Always,

	/// Auto-approve only if there are no changes (no-op).
	IfNoChanges,
}

impl fmt::Display for AutoApprove {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AutoApprove::Never => write!(f, "never"),
			AutoApprove::Always => write!(f, "always"),
			AutoApprove::IfNoChanges => write!(f, "if-no-changes"),
		}
	}
}

/// Trait for command args that have Jsonnet evaluation options.
pub trait JsonnetArgs {
	fn ext_str(&self) -> &[String];
	fn ext_code(&self) -> &[String];
	fn tla_str(&self) -> &[String];
	fn tla_code(&self) -> &[String];
	fn max_stack(&self) -> i32;
}

/// Macro to implement JsonnetArgs for a struct with standard fields.
///
/// The struct must have these fields:
/// - `ext_str: Vec<String>`
/// - `ext_code: Vec<String>`
/// - `tla_str: Vec<String>`
/// - `tla_code: Vec<String>`
/// - `max_stack: i32`
#[macro_export]
macro_rules! impl_jsonnet_args {
	($ty:ty) => {
		impl $crate::commands::util::JsonnetArgs for $ty {
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
		}
	};
}

/// Build evaluation options from command args that implement JsonnetArgs.
pub fn build_eval_opts(args: &impl JsonnetArgs) -> EvalOpts {
	EvalOpts {
		ext_str: parse_key_value_pairs(args.ext_str()),
		ext_code: parse_key_value_pairs(args.ext_code()),
		tla_str: parse_key_value_pairs(args.tla_str()),
		tla_code: parse_key_value_pairs(args.tla_code()),
		max_stack: Some(args.max_stack() as usize),
		eval_expr: None,
		env_name: None,
		export_jsonnet_implementation: None,
	}
}

/// Evaluate an environment path and extract a single environment.
///
/// This performs the common pattern of:
/// 1. Evaluating the Jsonnet
/// 2. Extracting environments
/// 3. Setting inline env namespace if needed
/// 4. Filtering by name if specified
/// 5. Ensuring exactly one environment is returned
pub fn evaluate_single_environment(
	path: &str,
	eval_opts: EvalOpts,
	name_filter: Option<&str>,
) -> Result<EnvironmentData> {
	tracing::debug!(path = %path, "evaluating environment");
	let eval_result = crate::eval::eval(path, eval_opts)
		.context(format!("evaluating environment at {}", path))?;

	// Extract environments (handles both inline and static environments)
	let mut environments = crate::spec::extract_environments(&eval_result.value, &eval_result.spec);

	// For inline environments, set metadata.namespace to file path
	if eval_result.spec.is_none() {
		crate::spec::set_inline_env_namespace(&mut environments, path);
	}

	// Filter by name if specified
	if let Some(target_name) = name_filter {
		environments = filter_environments_by_name(environments, target_name).map_err(|_| {
			anyhow::anyhow!(
				"no environment found matching name '{}'. Available environments: {}",
				target_name,
				get_environment_names(&crate::spec::extract_environments(
					&eval_result.value,
					&eval_result.spec
				))
			)
		})?;
	}

	// Ensure exactly one environment
	let [env_data] = <[_; 1]>::try_from(environments).map_err(|envs: Vec<_>| {
		anyhow::anyhow!(
			"multiple inline environments found ({}). Use --name to select one: {}",
			envs.len(),
			get_environment_names(&envs)
		)
	})?;

	Ok(env_data)
}

/// Process manifests by injecting labels and stripping null fields.
///
/// This performs the common pattern of:
/// 1. Injecting tanka.dev/environment label if injectLabels is enabled
/// 2. Stripping empty annotations/labels
pub fn process_manifests(manifests: &mut [serde_json::Value], env_spec: &Option<Environment>) {
	for manifest in manifests.iter_mut() {
		crate::spec::inject_environment_label(manifest, env_spec);
	}

	for manifest in manifests.iter_mut() {
		crate::spec::strip_null_metadata_fields(manifest);
	}
}

/// Get or create a cluster connection from the spec.
///
/// If a connection is already provided, returns it. Otherwise, creates
/// a new connection from the spec.
pub async fn get_or_create_connection(
	connection: Option<ClusterConnection>,
	spec: Option<&Spec>,
) -> Result<ClusterConnection> {
	match connection {
		Some(conn) => Ok(conn),
		None => {
			let spec_for_connection = spec.cloned().unwrap_or_default();
			tracing::debug!("connecting to Kubernetes cluster");
			let conn = ClusterConnection::from_spec(&spec_for_connection)
				.await
				.context("connecting to Kubernetes cluster")?;
			tracing::debug!(
				cluster = %conn.cluster_identifier(),
				server_version = %format!("{}.{}", conn.server_version().major, conn.server_version().minor),
				"connected to cluster"
			);
			Ok(conn)
		}
	}
}

/// Validate the dry-run option value.
///
/// Returns an error if the value is not one of: "", "none", "client", "server".
pub fn validate_dry_run(dry_run: Option<&str>) -> Result<()> {
	if let Some(value) = dry_run {
		match value {
			"" | "none" | "client" | "server" => {}
			_ => {
				anyhow::bail!("--dry-run must be either: \"\", \"none\", \"server\" or \"client\"")
			}
		}
	}
	Ok(())
}

/// Create a multi-threaded tokio runtime.
pub fn create_tokio_runtime() -> Result<tokio::runtime::Runtime> {
	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.context("creating tokio runtime")
}

/// Configuration for setting up a diff engine.
pub struct DiffEngineConfig<'a> {
	/// Connection to the Kubernetes cluster.
	pub connection: &'a ClusterConnection,
	/// Optional spec for strategy selection.
	pub spec: Option<&'a Spec>,
	/// Manifests to diff against.
	pub manifests: &'a [serde_json::Value],
	/// Whether to enable prune detection.
	pub with_prune: bool,
	/// Optional override for diff strategy.
	pub diff_strategy_override: Option<DiffStrategy>,
}

/// Result of setting up a diff engine.
pub struct DiffEngineSetup {
	/// The configured diff engine.
	pub engine: DiffEngine,
	/// The diff strategy being used.
	pub strategy: DiffStrategy,
	/// The default namespace for resources.
	pub default_namespace: String,
}

/// Set up a diff engine with strategy and namespace resolution.
///
/// This consolidates the common pattern of:
/// 1. Determining diff strategy from override, spec, or default
/// 2. Resolving default namespace from spec or connection
/// 3. Creating the diff engine
pub async fn setup_diff_engine(config: DiffEngineConfig<'_>) -> Result<DiffEngineSetup> {
	// Determine diff strategy
	let strategy = config.diff_strategy_override.unwrap_or_else(|| {
		if let Some(s) = config.spec {
			DiffStrategy::from_spec(s, config.connection.server_version())
		} else {
			DiffStrategy::Native
		}
	});
	tracing::debug!(strategy = %strategy, "using diff strategy");

	// Get default namespace from spec or connection
	let default_namespace = config
		.spec
		.map(|s| s.namespace.clone())
		.unwrap_or_else(|| config.connection.default_namespace().to_string());

	// Create diff engine
	let engine = DiffEngine::new(
		config.connection.clone(),
		strategy,
		default_namespace.clone(),
		config.manifests,
		config.with_prune,
	)
	.await
	.context("creating diff engine")?;

	Ok(DiffEngineSetup {
		engine,
		strategy,
		default_namespace,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	// -----------------------------------------------------------------------
	// filter_environments_by_name tests (mirrors Tanka's pkg/tanka/load_test.go)
	// -----------------------------------------------------------------------

	fn make_env(name: &str) -> EnvironmentData {
		EnvironmentData {
			spec: Some(Environment {
				metadata: crate::spec::Metadata {
					name: Some(name.to_string()),
					namespace: None,
					labels: None,
				},
				..Default::default()
			}),
			data: serde_json::json!({}),
		}
	}

	#[test]
	fn test_filter_environments_by_name_exact_match() {
		// Exact match should return single environment
		let envs = vec![
			make_env("project1-env1"),
			make_env("project1-env2"),
			make_env("project2-env1"),
		];

		let result = filter_environments_by_name(envs, "project1-env1").unwrap();
		assert_eq!(result.len(), 1);
		assert_eq!(env_name(&result[0]), Some("project1-env1"));
	}

	#[test]
	fn test_filter_environments_by_name_partial_match_single() {
		// Partial match that matches one environment
		let envs = vec![
			make_env("project1-env1"),
			make_env("project1-env2"),
			make_env("project2-env1"),
		];

		let result = filter_environments_by_name(envs, "project2").unwrap();
		assert_eq!(result.len(), 1);
		assert_eq!(env_name(&result[0]), Some("project2-env1"));
	}

	#[test]
	fn test_filter_environments_by_name_partial_match_multiple() {
		// Partial match that matches multiple environments - should return all matches
		let envs = vec![
			make_env("project1-env1"),
			make_env("project1-env2"),
			make_env("project2-env1"),
		];

		let result = filter_environments_by_name(envs, "project1").unwrap();
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn test_filter_environments_by_name_no_match() {
		// No match should return error
		let envs = vec![make_env("project1-env1"), make_env("project1-env2")];

		let result = filter_environments_by_name(envs, "no match");
		assert!(result.is_err());
	}

	#[test]
	fn test_filter_environments_by_name_full_match_has_priority() {
		// If there's an exact match, return just that one, even if there are
		// partial matches too. Mirrors Tanka's TestLoadSelectEnvironmentFullMatchHasPriority
		let envs = vec![make_env("base"), make_env("base-extended")];

		let result = filter_environments_by_name(envs, "base").unwrap();
		assert_eq!(result.len(), 1);
		assert_eq!(env_name(&result[0]), Some("base"));
	}

	// -----------------------------------------------------------------------
	// extract_manifests tests (mirrors Tanka's pkg/process/extract_test.go)
	// -----------------------------------------------------------------------

	#[test]
	fn test_extract_manifests_regular() {
		let value = serde_json::json!({
			"deployment": {
				"apiVersion": "apps/v1",
				"kind": "Deployment",
				"metadata": { "name": "grafana" }
			},
			"service": {
				"apiVersion": "v1",
				"kind": "Service",
				"metadata": { "name": "grafana" }
			}
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_extract_manifests_flat() {
		let value = serde_json::json!({
			"apiVersion": "apps/v1",
			"kind": "Deployment",
			"metadata": { "name": "grafana" }
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 1);
	}

	#[test]
	fn test_extract_manifests_deep_nesting() {
		let value = serde_json::json!({
			"app": {
				"web": {
					"backend": {
						"server": {
							"grafana": {
								"deployment": {
									"apiVersion": "apps/v1",
									"kind": "Deployment",
									"metadata": { "name": "grafana" }
								}
							}
						}
					},
					"frontend": {
						"nodejs": {
							"express": {
								"service": {
									"apiVersion": "v1",
									"kind": "Service",
									"metadata": { "name": "frontend" }
								}
							}
						}
					}
				}
			}
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 2);
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
				"kind": "ConfigMap",
				"metadata": { "name": "cm2" }
			}
		]);

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_extract_manifests_nil_values_ignored() {
		// null values should be silently skipped
		let value = serde_json::json!({
			"enabled": {
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "config" }
			},
			"disabledObject": null
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 1);
	}

	#[test]
	fn test_extract_manifests_unwrap_list() {
		// List kind should be expanded into individual manifests
		let value = serde_json::json!({
			"foo": {
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
			}
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_extract_manifests_primitives_ignored() {
		// Primitive values in the object should be silently skipped
		let value = serde_json::json!({
			"string_val": "hello",
			"number_val": 42,
			"bool_val": true,
			"cm": {
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "test" }
			}
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 1);
	}

	// -----------------------------------------------------------------------
	// Target filter tests (mirrors Tanka's pkg/process/process_test.go target tests)
	// -----------------------------------------------------------------------

	#[test]
	fn test_extract_manifests_target_filter_regex() {
		// Regex filter on kind/name
		let value = serde_json::json!({
			"deployment": {
				"apiVersion": "apps/v1",
				"kind": "Deployment",
				"metadata": { "name": "grafana" }
			},
			"service": {
				"apiVersion": "v1",
				"kind": "Service",
				"metadata": { "name": "frontend" }
			}
		});

		let manifests = extract_manifests(&value, &["Deployment/.*".to_string()]).unwrap();
		assert_eq!(manifests.len(), 1);
		assert_eq!(manifests[0]["kind"], "Deployment");
	}

	#[test]
	fn test_extract_manifests_target_filter_multiple() {
		// Multiple targets: match any
		let value = serde_json::json!({
			"dep": {
				"apiVersion": "apps/v1",
				"kind": "Deployment",
				"metadata": { "name": "grafana" }
			},
			"svc": {
				"apiVersion": "v1",
				"kind": "Service",
				"metadata": { "name": "frontend" }
			},
			"ns": {
				"apiVersion": "v1",
				"kind": "Namespace",
				"metadata": { "name": "monitoring" }
			}
		});

		let manifests = extract_manifests(
			&value,
			&[
				"Deployment/grafana".to_string(),
				"Service/frontend".to_string(),
			],
		)
		.unwrap();
		assert_eq!(manifests.len(), 2);
	}

	// -----------------------------------------------------------------------
	// collect_manifests tests (additional edge cases)
	// -----------------------------------------------------------------------

	#[test]
	fn test_collect_manifests_nested_objects() {
		let value = serde_json::json!({
			"app": {
				"nested": {
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "nested" }
				}
			}
		});

		let mut manifests = Vec::new();
		collect_manifests(&value, &mut manifests);
		assert_eq!(manifests.len(), 1);
	}

	#[test]
	fn test_collect_manifests_environment_wrapper_extracts_data() {
		// Environment objects should not be collected themselves;
		// List kind should be expanded
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "List",
			"items": [
				{
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "cm1" }
				}
			]
		});

		let mut manifests = Vec::new();
		collect_manifests(&value, &mut manifests);
		assert_eq!(manifests.len(), 1);
		assert_eq!(manifests[0]["metadata"]["name"], "cm1");
	}

	// -----------------------------------------------------------------------
	// process_manifests tests (mirrors Tanka's Process() integration)
	// -----------------------------------------------------------------------

	#[test]
	fn test_process_manifests_inject_labels() {
		let env = Some(Environment {
			spec: crate::spec::Spec {
				inject_labels: Some(true),
				..Default::default()
			},
			metadata: crate::spec::Metadata {
				name: Some("test-env".to_string()),
				namespace: Some("main.jsonnet".to_string()),
				labels: None,
			},
			..Default::default()
		});

		let mut manifests = vec![serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test" }
		})];

		process_manifests(&mut manifests, &env);

		let labels = manifests[0]["metadata"]["labels"].as_object().unwrap();
		assert!(labels.contains_key("tanka.dev/environment"));
	}

	#[test]
	fn test_process_manifests_strips_null_labels() {
		let mut manifests = vec![serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test",
				"labels": null,
				"annotations": null
			}
		})];

		process_manifests(&mut manifests, &None);

		assert!(manifests[0]["metadata"].get("labels").is_none());
		assert!(manifests[0]["metadata"].get("annotations").is_none());
	}

	#[test]
	fn test_process_manifests_order_consistent() {
		// Processing the same data multiple times should produce the same result
		// Mirrors Tanka's TestProcessOrder
		let manifests_data = vec![
			serde_json::json!({
				"apiVersion": "apps/v1",
				"kind": "Deployment",
				"metadata": { "name": "deploy1" }
			}),
			serde_json::json!({
				"apiVersion": "v1",
				"kind": "Service",
				"metadata": { "name": "svc1" }
			}),
			serde_json::json!({
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "cm1" }
			}),
		];

		let env = Some(Environment {
			spec: crate::spec::Spec {
				inject_labels: Some(true),
				..Default::default()
			},
			..Default::default()
		});

		let mut results = Vec::new();
		for _ in 0..10 {
			let mut manifests = manifests_data.clone();
			process_manifests(&mut manifests, &env);
			results.push(manifests);
		}

		// All results should be identical
		for i in 1..10 {
			assert_eq!(results[0], results[i], "run {} differs from run 0", i);
		}
	}
}
