//! Env add subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct AddArgs {
	/// Path for the new environment
	pub path: String,

	/// Valid context name for environment, can pass multiple, regex supported
	#[arg(long)]
	pub context_name: Vec<String>,

	/// Specify diff-strategy. Automatically detected otherwise.
	#[arg(long)]
	pub diff_strategy: Option<String>,

	/// Add tanka environment label to each created resource. Required for 'tk prune'.
	#[arg(long)]
	pub inject_labels: bool,

	/// Create an inline environment
	#[arg(short = 'i', long)]
	pub inline: bool,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Namespace to create objects in
	#[arg(long)]
	pub namespace: Option<String>,

	/// Endpoint of the Kubernetes API
	#[arg(long)]
	pub server: Option<String>,

	/// Set the server to a known one from $KUBECONFIG
	#[arg(long)]
	pub server_from_context: Option<String>,
}

/// Run the env add subcommand.
pub fn run<W: Write>(_args: AddArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
