//! Prune command handler.
//!
//! Removes Kubernetes resources that exist in the cluster but are no longer
//! defined in the Tanka environment manifests.

use std::io::Write;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::diff::ColorMode;
use super::util::UnimplementedArgs;
use crate::{
	eval::EvalOpts,
	k8s::{
		apply::ApplyEngine,
		client::ClusterConnection,
		diff::{DiffEngine, DiffStatus, ResourceDiff},
		output::DiffOutput,
	},
	spec::DiffStrategy,
};

/// Auto-approve settings for the prune command.
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

#[derive(Args)]
pub struct PruneArgs {
	/// Path to prune
	pub path: String,

	/// Skip interactive approval. Only for automation! Allowed values: 'always', 'never', 'if-no-changes'
	#[arg(long, value_enum)]
	pub auto_approve: Option<AutoApprove>,

	/// Controls color in diff output, must be "auto", "always", or "never"
	#[arg(long, default_value = "auto", value_enum)]
	pub color: ColorMode,

	/// Force the diff-strategy to use. Automatically chosen if not set.
	#[arg(long, value_enum)]
	pub diff_strategy: Option<DiffStrategy>,

	/// --dry-run parameter to pass down to kubectl, must be "none", "server", or "client"
	#[arg(long)]
	pub dry_run: Option<String>,

	/// Set code value of extVar (Format: key=<code>)
	#[arg(long)]
	pub ext_code: Vec<String>,

	/// Set string value of extVar (Format: key=value)
	#[arg(short = 'V', long)]
	pub ext_str: Vec<String>,

	/// Force applying (kubectl apply --force)
	#[arg(long)]
	pub force: bool,

	/// Use `go` to use native go-jsonnet implementation and `binary:<path>` to delegate evaluation to a binary (with the same API as the regular `jsonnet` binary)
	#[arg(long, default_value = "go")]
	pub jsonnet_implementation: String,

	/// Jsonnet VM max stack. Increase this if you get: max stack frames exceeded
	#[arg(long, default_value = "500")]
	pub max_stack: i32,

	/// String that only a single inline environment contains in its name
	#[arg(long)]
	pub name: Option<String>,

	/// Regex filter on '<kind>/<name>'. See https://tanka.dev/output-filtering
	#[arg(short = 't', long)]
	pub target: Vec<String>,

	/// Set code value of top level function (Format: key=<code>)
	#[arg(long)]
	pub tla_code: Vec<String>,

	/// Set string value of top level function (Format: key=value)
	#[arg(short = 'A', long)]
	pub tla_str: Vec<String>,
}

/// Run the prune command.
pub fn run<W: Write>(args: PruneArgs, writer: W) -> Result<()> {
	UnimplementedArgs {
		jsonnet_implementation: Some(&args.jsonnet_implementation),
		cache_envs: None,
		cache_path: None,
		mem_ballast_size_bytes: None,
	}
	.warn_if_set();

	// Validate dry-run option
	if let Some(ref dry_run) = args.dry_run {
		match dry_run.as_str() {
			"" | "none" | "client" | "server" => {}
			_ => {
				anyhow::bail!("--dry-run must be either: \"\", \"none\", \"server\" or \"client\"")
			}
		}
	}

	// Create a tokio runtime for async operations
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.context("creating tokio runtime")?;

	runtime.block_on(run_async(args, writer))
}

/// Options for running a prune operation.
#[derive(Default)]
pub struct PruneOpts {
	/// Diff strategy to use.
	pub diff_strategy: Option<DiffStrategy>,
	/// Auto-approval setting.
	pub auto_approve: AutoApprove,
	/// Dry-run mode (none, client, or server).
	pub dry_run: Option<String>,
	/// Force delete.
	pub force: bool,
	/// Color output mode.
	pub color: ColorMode,
	/// Target filters.
	pub target: Vec<String>,
	/// Filter environments by name.
	pub name: Option<String>,
}

/// Prune orphaned resources from the cluster.
///
/// Returns the list of deleted resources.
#[instrument(skip_all, fields(path = %path))]
pub async fn prune_environment<W: Write>(
	path: &str,
	connection: Option<ClusterConnection>,
	eval_opts: EvalOpts,
	opts: PruneOpts,
	mut writer: W,
) -> Result<Vec<ResourceDiff>> {
	// Evaluate the environment
	tracing::debug!(path = %path, "evaluating environment");
	let eval_result = crate::eval::eval(path, eval_opts)
		.context(format!("evaluating environment at {}", path))?;

	// Extract environments
	let mut environments = crate::spec::extract_environments(&eval_result.value, &eval_result.spec);

	// For inline environments, set metadata.namespace to file path
	if eval_result.spec.is_none() {
		crate::spec::set_inline_env_namespace(&mut environments, path);
	}

	// Filter by name if specified
	if let Some(ref target_name) = opts.name {
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

	// For prune, we only support a single environment
	let [env_data] = <[_; 1]>::try_from(environments).map_err(|envs: Vec<_>| {
		anyhow::anyhow!(
			"multiple inline environments found ({}). Use --name to select one: {}",
			envs.len(),
			get_environment_names(&envs)
		)
	})?;
	let env_spec = env_data.spec;

	// Get the spec for cluster connection and strategy selection
	let spec = env_spec.as_ref().map(|e| &e.spec);

	// Prune requires injectLabels to be enabled
	let inject_labels = spec.and_then(|s| s.inject_labels).unwrap_or(false);
	if !inject_labels {
		anyhow::bail!(
			"spec.injectLabels is set to false in your spec.json. Tanka needs to add \
			 a label to your resources to reliably detect which were removed from Jsonnet. \
			 See https://tanka.dev/garbage-collection for more details"
		);
	}

	// Extract manifests from environment data
	let mut manifests = extract_manifests(&env_data.data, &opts.target)?;
	tracing::debug!(manifest_count = manifests.len(), "found manifests");

	// Inject tanka.dev/environment label
	for manifest in &mut manifests {
		crate::spec::inject_environment_label(manifest, &env_spec);
	}

	// Strip empty annotations/labels
	for manifest in &mut manifests {
		crate::spec::strip_null_metadata_fields(manifest);
	}

	// Use provided connection or create one from spec
	let connection = match connection {
		Some(conn) => conn,
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
			conn
		}
	};

	// Determine diff strategy
	let diff_strategy = opts.diff_strategy.unwrap_or_else(|| {
		if let Some(s) = spec {
			DiffStrategy::from_spec(s, connection.server_version())
		} else {
			DiffStrategy::Native
		}
	});
	tracing::debug!(strategy = %diff_strategy, "using diff strategy");

	// Get default namespace from spec or connection
	let default_namespace = spec
		.map(|s| s.namespace.clone())
		.unwrap_or_else(|| connection.default_namespace().to_string());

	// Create diff engine with prune enabled
	let diff_engine = DiffEngine::new(
		connection.clone(),
		diff_strategy,
		default_namespace.clone(),
		&manifests,
		true, // with_prune = true
	)
	.await
	.context("creating diff engine")?;

	// Get environment label for prune detection
	let env_label = env_spec
		.as_ref()
		.map(crate::spec::generate_environment_label);

	// Compute diffs with prune
	tracing::debug!("computing differences with prune detection");
	let diffs = diff_engine
		.diff_all(&manifests, true, env_label.as_deref(), true)
		.await
		.context("computing diffs")?;

	// Filter to only deleted resources
	let to_delete: Vec<_> = diffs
		.iter()
		.filter(|d| d.status == DiffStatus::Deleted)
		.collect();

	if to_delete.is_empty() {
		eprintln!("Nothing to prune.");
		return Ok(Vec::new());
	}

	// Display what will be deleted
	let mut output = DiffOutput::new(&mut writer, opts.color, diff_strategy)?;
	for diff in &to_delete {
		output.write_diff(diff)?;
	}

	eprintln!("\n{} resource(s) will be deleted:", to_delete.len());
	for diff in &to_delete {
		eprintln!(
			"  {} {}/{}",
			diff.gvk.kind,
			diff.namespace.as_deref().unwrap_or(""),
			diff.name
		);
	}

	// Check if we're in dry-run mode
	let is_dry_run = opts
		.dry_run
		.as_deref()
		.is_some_and(|d| d != "none" && !d.is_empty());
	if is_dry_run {
		eprintln!("\nDry-run mode: no resources will be deleted.");
		return Ok(to_delete.into_iter().cloned().collect());
	}

	// Determine if we should proceed
	let should_prune = match opts.auto_approve {
		AutoApprove::Always => true,
		AutoApprove::IfNoChanges => to_delete.is_empty(),
		AutoApprove::Never => {
			// Prompt for confirmation
			if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
				anyhow::bail!(
					"cannot prompt for confirmation in non-interactive mode. \
					 Use --auto-approve=always to skip confirmation."
				);
			}
			prompt_confirmation()?
		}
	};

	if !should_prune {
		eprintln!("Prune cancelled.");
		return Ok(Vec::new());
	}

	// Create apply engine for deletion
	let apply_engine = ApplyEngine::new(
		connection.client().clone(),
		default_namespace,
		false, // server_side doesn't matter for delete
		opts.force,
	);

	// Delete orphaned resources
	eprintln!("\nDeleting resources...");
	let mut deleted = Vec::new();
	for diff in to_delete {
		match apply_engine
			.delete_resource(&diff.gvk, &diff.name, diff.namespace.as_deref())
			.await
		{
			Ok(_) => {
				eprintln!(
					"  {} {}/{} deleted",
					diff.gvk.kind,
					diff.namespace.as_deref().unwrap_or(""),
					diff.name
				);
				deleted.push(diff.clone());
			}
			Err(e) => {
				return Err(anyhow::anyhow!(
					"failed to delete {}/{}: {}",
					diff.gvk.kind,
					diff.name,
					e
				));
			}
		}
	}

	eprintln!("\nPrune complete. {} resource(s) deleted.", deleted.len());
	Ok(deleted)
}

/// Prompt the user for confirmation.
fn prompt_confirmation() -> Result<bool> {
	eprint!("\nDelete these resources? [y/N]: ");
	std::io::stderr().flush()?;

	let mut input = String::new();
	std::io::stdin().read_line(&mut input)?;

	let input = input.trim().to_lowercase();
	Ok(input == "y" || input == "yes")
}

/// Async implementation of the prune command.
#[instrument(skip_all, fields(path = %args.path))]
async fn run_async<W: Write>(args: PruneArgs, writer: W) -> Result<()> {
	let eval_opts = build_eval_opts(&args);
	let opts = PruneOpts {
		diff_strategy: args.diff_strategy,
		auto_approve: args.auto_approve.unwrap_or_default(),
		dry_run: args.dry_run,
		force: args.force,
		color: args.color,
		target: args.target,
		name: args.name,
	};

	prune_environment(&args.path, None, eval_opts, opts, writer).await?;
	Ok(())
}

/// Parse key=value pairs into a HashMap.
fn parse_key_value_pairs(items: &[String]) -> std::collections::HashMap<String, String> {
	items
		.iter()
		.filter_map(|s| {
			s.split_once('=')
				.map(|(k, v)| (k.to_string(), v.to_string()))
		})
		.collect()
}

/// Build evaluation options from command args.
fn build_eval_opts(args: &PruneArgs) -> EvalOpts {
	EvalOpts {
		ext_str: parse_key_value_pairs(&args.ext_str),
		ext_code: parse_key_value_pairs(&args.ext_code),
		tla_str: parse_key_value_pairs(&args.tla_str),
		tla_code: parse_key_value_pairs(&args.tla_code),
		max_stack: Some(args.max_stack as usize),
		eval_expr: None,
		env_name: args.name.clone(),
		export_jsonnet_implementation: None,
	}
}

/// Extract Kubernetes manifests from the evaluation result.
fn extract_manifests(
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
fn collect_manifests(value: &serde_json::Value, manifests: &mut Vec<serde_json::Value>) {
	match value {
		serde_json::Value::Object(map) => {
			if map.contains_key("apiVersion") && map.contains_key("kind") {
				if map.get("kind").and_then(|v| v.as_str()) == Some("List") {
					if let Some(items) = map.get("items").and_then(|v| v.as_array()) {
						for item in items {
							collect_manifests(item, manifests);
						}
					}
				} else {
					manifests.push(value.clone());
				}
			} else {
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
		_ => {}
	}
}

/// Get the name of an environment, if available.
fn env_name(env_data: &crate::spec::EnvironmentData) -> Option<&str> {
	env_data
		.spec
		.as_ref()
		.and_then(|s| s.metadata.name.as_deref())
}

/// Format a list of environment names for error messages.
fn get_environment_names(environments: &[crate::spec::EnvironmentData]) -> String {
	let names: Vec<&str> = environments.iter().filter_map(env_name).collect();
	if names.is_empty() {
		"(unnamed environments)".to_string()
	} else {
		names.join(", ")
	}
}

/// Filter environments by name, trying exact match first, then substring.
fn filter_environments_by_name(
	environments: Vec<crate::spec::EnvironmentData>,
	target_name: &str,
) -> Result<Vec<crate::spec::EnvironmentData>> {
	let exact: Vec<_> = environments
		.iter()
		.filter(|e| env_name(e) == Some(target_name))
		.cloned()
		.collect();

	if let [_single] = exact.as_slice() {
		return Ok(exact);
	}

	let matches: Vec<_> = environments
		.into_iter()
		.filter(|e| env_name(e).is_some_and(|n| n.contains(target_name)))
		.collect();

	if matches.is_empty() {
		anyhow::bail!("no environment found matching name '{}'", target_name);
	}

	Ok(matches)
}
