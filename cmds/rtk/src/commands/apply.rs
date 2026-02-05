//! Apply command handler.
//!
//! Applies Tanka environment manifests to the Kubernetes cluster after showing
//! a diff and optionally prompting for confirmation.

use std::{fmt, io::Write};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::diff::ColorMode;
use super::util::{
	build_eval_opts, create_tokio_runtime, extract_manifests, get_or_create_connection,
	process_manifests, prompt_confirmation, validate_dry_run, JsonnetArgs, UnimplementedArgs,
};

// Re-export AutoApprove for backwards compatibility
pub use super::util::AutoApprove;
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

/// Apply strategy for resource updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApplyStrategy {
	/// Client-side apply using PATCH with strategic merge.
	#[default]
	Client,

	/// Server-side apply using PATCH with Apply.
	Server,
}

impl fmt::Display for ApplyStrategy {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ApplyStrategy::Client => write!(f, "client"),
			ApplyStrategy::Server => write!(f, "server"),
		}
	}
}

#[derive(Args)]
pub struct ApplyArgs {
	/// Path to the Tanka environment
	pub path: String,

	/// Force the apply strategy to use. Automatically chosen if not set.
	#[arg(long, value_enum)]
	pub apply_strategy: Option<ApplyStrategy>,

	/// Skip interactive approval. Allowed values: 'always', 'never', 'if-no-changes'
	#[arg(long, value_enum)]
	pub auto_approve: Option<AutoApprove>,

	/// Controls color in diff output
	#[arg(long, default_value = "auto", value_enum)]
	pub color: ColorMode,

	/// Force the diff strategy to use. Automatically chosen if not set.
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

	/// Validation of resources (kubectl --validate=false)
	#[arg(long, default_value = "true")]
	pub validate: bool,
}

impl JsonnetArgs for ApplyArgs {
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

/// Run the apply command.
pub fn run<W: Write>(args: ApplyArgs, writer: W) -> Result<()> {
	UnimplementedArgs {
		jsonnet_implementation: Some(&args.jsonnet_implementation),
		cache_envs: None,
		cache_path: None,
		mem_ballast_size_bytes: None,
	}
	.warn_if_set();

	validate_dry_run(args.dry_run.as_deref())?;

	let runtime = create_tokio_runtime()?;
	runtime.block_on(run_async(args, writer))
}

/// Options for running an apply operation.
#[derive(Default)]
pub struct ApplyOpts {
	/// Diff strategy to use.
	pub diff_strategy: Option<DiffStrategy>,
	/// Apply strategy to use.
	pub apply_strategy: Option<ApplyStrategy>,
	/// Auto-approval setting.
	pub auto_approve: AutoApprove,
	/// Dry-run mode (none, client, or server).
	pub dry_run: Option<String>,
	/// Force apply.
	pub force: bool,
	/// Color output mode.
	pub color: ColorMode,
	/// Target filters.
	pub target: Vec<String>,
	/// Filter environments by name.
	pub name: Option<String>,
}

/// Apply manifests to the cluster.
///
/// Returns the list of applied resources.
#[instrument(skip_all, fields(path = %path))]
pub async fn apply_environment<W: Write>(
	path: &str,
	connection: Option<ClusterConnection>,
	eval_opts: EvalOpts,
	opts: ApplyOpts,
	mut writer: W,
) -> Result<Vec<ResourceDiff>> {
	use super::util::evaluate_single_environment;

	let env_data = evaluate_single_environment(path, eval_opts, opts.name.as_deref())?;
	let env_spec = env_data.spec;

	// Get the spec for cluster connection and strategy selection
	let spec = env_spec.as_ref().map(|e| &e.spec);

	// Extract manifests from environment data
	let mut manifests = extract_manifests(&env_data.data, &opts.target)?;
	tracing::debug!(manifest_count = manifests.len(), "found manifests to apply");

	if manifests.is_empty() {
		tracing::warn!("no manifests found in environment");
		eprintln!("No manifests to apply.");
		return Ok(Vec::new());
	}

	process_manifests(&mut manifests, &env_spec);

	let connection = get_or_create_connection(connection, spec).await?;

	// Determine strategies
	let diff_strategy = opts.diff_strategy.unwrap_or_else(|| {
		if let Some(s) = spec {
			DiffStrategy::from_spec(s, connection.server_version())
		} else {
			DiffStrategy::Native
		}
	});
	tracing::debug!(strategy = %diff_strategy, "using diff strategy");

	let apply_strategy = opts.apply_strategy.unwrap_or(ApplyStrategy::Client);
	tracing::debug!(strategy = %apply_strategy, "using apply strategy");

	// Get default namespace from spec or connection
	let default_namespace = spec
		.map(|s| s.namespace.clone())
		.unwrap_or_else(|| connection.default_namespace().to_string());

	// Create diff engine
	let diff_engine = DiffEngine::new(
		connection.clone(),
		diff_strategy,
		default_namespace.clone(),
		&manifests,
		false, // no prune for apply (use prune command)
	)
	.await
	.context("creating diff engine")?;

	// Compute diffs
	tracing::debug!("computing differences");
	let diffs = diff_engine
		.diff_all(&manifests, false, None, false)
		.await
		.context("computing diffs")?;

	// Check if there are changes
	let has_changes = diffs.iter().any(|d| d.has_changes());

	// Display diff
	let mut output = DiffOutput::new(&mut writer, opts.color, diff_strategy)?;
	for diff in &diffs {
		if diff.status != DiffStatus::Unchanged {
			output.write_diff(diff)?;
		}
	}

	if !has_changes {
		eprintln!("No differences. Nothing to apply.");
		return Ok(diffs);
	}

	// Check if we're in dry-run mode
	let is_dry_run = opts
		.dry_run
		.as_deref()
		.is_some_and(|d| d != "none" && !d.is_empty());
	if is_dry_run {
		eprintln!("\nDry-run mode: no changes will be applied.");
		return Ok(diffs);
	}

	// Determine if we should apply
	let should_apply = match opts.auto_approve {
		AutoApprove::Always => true,
		AutoApprove::IfNoChanges => !has_changes,
		AutoApprove::Never => {
			// Prompt for confirmation
			if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
				anyhow::bail!(
					"cannot prompt for confirmation in non-interactive mode. \
					 Use --auto-approve=always to skip confirmation."
				);
			}
			prompt_confirmation("Apply these changes?")?
		}
	};

	if !should_apply {
		eprintln!("Apply cancelled.");
		return Ok(diffs);
	}

	// Create apply engine
	let apply_engine = ApplyEngine::new(
		connection.client().clone(),
		default_namespace,
		apply_strategy == ApplyStrategy::Server,
		opts.force,
	);

	// Apply changes
	eprintln!("\nApplying changes...");
	let changes_to_apply: Vec<_> = diffs
		.iter()
		.filter(|d| d.has_changes() && d.status != DiffStatus::Deleted)
		.collect();

	for diff in &changes_to_apply {
		// Find the corresponding manifest
		let manifest = manifests.iter().find(|m| {
			let name = m
				.pointer("/metadata/name")
				.and_then(|v| v.as_str())
				.unwrap_or("");
			let kind = m.get("kind").and_then(|v| v.as_str()).unwrap_or("");
			name == diff.name && kind == diff.gvk.kind
		});

		if let Some(manifest) = manifest {
			match apply_engine.apply_manifest(manifest).await {
				Ok(_) => {
					eprintln!(
						"  {} {}/{} applied",
						diff.gvk.kind,
						diff.namespace.as_deref().unwrap_or(""),
						diff.name
					);
				}
				Err(e) => {
					return Err(anyhow::anyhow!(
						"failed to apply {}/{}: {}",
						diff.gvk.kind,
						diff.name,
						e
					));
				}
			}
		}
	}

	eprintln!(
		"\nApply complete. {} resource(s) changed.",
		changes_to_apply.len()
	);
	Ok(diffs)
}

/// Async implementation of the apply command.
#[instrument(skip_all, fields(path = %args.path))]
async fn run_async<W: Write>(args: ApplyArgs, writer: W) -> Result<()> {
	let eval_opts = build_eval_opts(&args);
	let opts = ApplyOpts {
		diff_strategy: args.diff_strategy,
		apply_strategy: args.apply_strategy,
		auto_approve: args.auto_approve.unwrap_or_default(),
		dry_run: args.dry_run,
		force: args.force,
		color: args.color,
		target: args.target,
		name: args.name,
	};

	apply_environment(&args.path, None, eval_opts, opts, writer).await?;
	Ok(())
}
