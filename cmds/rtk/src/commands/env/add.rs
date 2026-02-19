//! Env add subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

use crate::env::{env_add, EnvSpecOptions};

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
pub fn run<W: Write>(args: AddArgs, _writer: W) -> Result<()> {
	let opts = EnvSpecOptions {
		namespace: args.namespace,
		server: args.server,
		server_from_context: args.server_from_context,
		context_name: args.context_name,
		diff_strategy: args.diff_strategy,
		inject_labels: Some(args.inject_labels),
	};
	env_add(&args.path, args.inline, &opts)
}
