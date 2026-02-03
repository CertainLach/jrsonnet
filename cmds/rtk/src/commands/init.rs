//! Init command handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct InitArgs {
	/// Ignore the working directory not being empty
	#[arg(short = 'f', long)]
	pub force: bool,

	/// Create an inline environment
	#[arg(short = 'i', long)]
	pub inline: bool,

	/// Choose the version of k8s-libsonnet, set to false to skip
	#[arg(long, default_value = "1.29")]
	pub k8s: String,
}

/// Run the init command.
pub fn run<W: Write>(_args: InitArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
