//! Imports subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct ImportsArgs {
	/// Path to check imports
	pub path: String,

	/// Git commit hash to check against
	#[arg(short = 'c', long)]
	pub check: Option<String>,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

/// Run the imports subcommand.
pub fn run<W: Write>(_args: ImportsArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
