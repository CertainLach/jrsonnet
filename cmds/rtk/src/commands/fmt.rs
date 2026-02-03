//! Fmt command handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct FmtArgs {
	/// Files or directories to format
	pub paths: Vec<String>,

	/// Globs to exclude
	#[arg(short = 'e', long, default_values_t = vec!["**/.*".to_string(), ".*".to_string(), "**/vendor/**".to_string(), "vendor/**".to_string()])]
	pub exclude: Vec<String>,

	/// Print formatted contents to stdout instead of writing to disk
	#[arg(long)]
	pub stdout: bool,

	/// Exit with non-zero when changes would be made
	#[arg(short = 't', long)]
	pub test: bool,

	/// Print each checked file
	#[arg(short = 'v', long)]
	pub verbose: bool,
}

/// Run the fmt command.
pub fn run<W: Write>(_args: FmtArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
