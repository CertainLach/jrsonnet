//! Importers-count subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct ImportersCountArgs {
	/// Directory to scan for files
	pub dir: String,

	/// Only count files that match the given regex. Matches only jsonnet files by default
	#[arg(long)]
	pub filename_regex: Option<String>,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Find files recursively
	#[arg(long)]
	pub recursive: bool,

	/// Root directory to search for environments
	#[arg(long, default_value = ".")]
	pub root: String,
}

/// Run the importers-count subcommand.
pub fn run<W: Write>(_args: ImportersCountArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
