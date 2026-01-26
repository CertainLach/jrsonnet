//! Lint command handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct LintArgs {
	/// Files or directories to lint
	pub paths: Vec<String>,

	/// Globs to exclude
	#[arg(short = 'e', long, default_values_t = vec!["**/.*".to_string(), ".*".to_string(), "**/vendor/**".to_string(), "vendor/**".to_string()])]
	pub exclude: Vec<String>,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Amount of workers
	#[arg(short = 'n', long, default_value = "4")]
	pub parallelism: i32,
}

/// Run the lint command.
pub fn run<W: Write>(_args: LintArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
