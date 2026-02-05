//! Prune command handler.
//!
//! Removes Kubernetes resources that exist in the cluster but are no longer
//! defined in the Tanka environment manifests.

use std::io::Write;

use anyhow::{Context, Result};
use clap::Args;
use tracing::instrument;

use super::diff::ColorMode;
use super::util::{
	build_eval_opts, create_tokio_runtime, extract_manifests, get_or_create_connection,
	process_manifests, prompt_confirmation, setup_diff_engine, validate_dry_run, DiffEngineConfig,
	JsonnetArgs, UnimplementedArgs,
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

crate::impl_jsonnet_args!(PruneArgs);

/// Run the prune command.
pub fn run<W: Write>(args: PruneArgs, writer: W) -> Result<()> {
	UnimplementedArgs::warn_jsonnet_impl(&args.jsonnet_implementation);

	validate_dry_run(args.dry_run.as_deref())?;

	let runtime = create_tokio_runtime()?;
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
	use super::util::evaluate_single_environment;

	let env_data = evaluate_single_environment(path, eval_opts, opts.name.as_deref())?;
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

	process_manifests(&mut manifests, &env_spec);

	let connection = get_or_create_connection(connection, spec).await?;

	// Set up diff engine with prune enabled
	let setup = setup_diff_engine(DiffEngineConfig {
		connection: &connection,
		spec,
		manifests: &manifests,
		with_prune: true,
		diff_strategy_override: opts.diff_strategy,
	})
	.await?;
	let diff_engine = setup.engine;
	let diff_strategy = setup.strategy;
	let default_namespace = setup.default_namespace;

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
			prompt_confirmation("Delete these resources?")?
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
