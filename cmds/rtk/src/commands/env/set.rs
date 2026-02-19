//! Env set subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

use crate::env::{env_set, EnvSpecOptions};

#[derive(Args)]
pub struct SetArgs {
	/// Path to the environment
	pub path: String,

	/// Valid context name for environment, can pass multiple, regex supported
	#[arg(long)]
	pub context_name: Vec<String>,

	/// Specify diff-strategy. Automatically detected otherwise.
	#[arg(long)]
	pub diff_strategy: Option<String>,

	/// Add tanka environment label to each created resource. Required for 'tk prune'.
	#[arg(long, value_parser = clap::builder::BoolishValueParser::new())]
	pub inject_labels: Option<bool>,

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

/// Run the env set subcommand.
pub fn run<W: Write>(args: SetArgs, _writer: W) -> Result<()> {
	let opts = EnvSpecOptions {
		namespace: args.namespace,
		server: args.server,
		server_from_context: args.server_from_context,
		context_name: args.context_name,
		diff_strategy: args.diff_strategy,
		inject_labels: args.inject_labels,
	};
	env_set(&args.path, &opts)
}
