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
	k8s::client::ClusterConnection,
	spec::{Environment, EnvironmentData, Spec},
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
			.map(|f| regex::Regex::new(f))
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
	fn name(&self) -> Option<&str>;
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
		env_name: args.name().map(|s| s.to_string()),
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
