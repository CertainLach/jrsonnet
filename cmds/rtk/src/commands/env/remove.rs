//! Env remove subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct RemoveArgs {
	/// Path(s) to the environment(s) to remove
	pub paths: Vec<String>,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

/// Run the env remove subcommand.
pub fn run<W: Write>(_args: RemoveArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
