//! Diff command handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct DiffArgs {
	/// Path to diff
	pub path: String,

	/// Controls color in diff output, must be "auto", "always", or "never"
	#[arg(long, default_value = "auto")]
	pub color: String,

	/// Force the diff-strategy to use. Automatically chosen if not set.
	#[arg(long)]
	pub diff_strategy: Option<String>,

	/// Exit with 0 even when differences are found
	#[arg(short = 'z', long)]
	pub exit_zero: bool,

	/// Set code value of extVar (Format: key=<code>)
	#[arg(long)]
	pub ext_code: Vec<String>,

	/// Set string value of extVar (Format: key=value)
	#[arg(short = 'V', long)]
	pub ext_str: Vec<String>,

	/// Use `go` to use native go-jsonnet implementation and `binary:<path>` to delegate evaluation to a binary (with the same API as the regular `jsonnet` binary)
	#[arg(long, default_value = "go")]
	pub jsonnet_implementation: String,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Jsonnet VM max stack. Increase this if you get: max stack frames exceeded
	#[arg(long, default_value = "500")]
	pub max_stack: i32,

	/// String that only a single inline environment contains in its name
	#[arg(long)]
	pub name: Option<String>,

	/// Print summary of the differences, not the actual contents
	#[arg(short = 's', long)]
	pub summarize: bool,

	/// Regex filter on '<kind>/<name>'. See https://tanka.dev/output-filtering
	#[arg(short = 't', long)]
	pub target: Vec<String>,

	/// Set code value of top level function (Format: key=<code>)
	#[arg(long)]
	pub tla_code: Vec<String>,

	/// Set string value of top level function (Format: key=value)
	#[arg(short = 'A', long)]
	pub tla_str: Vec<String>,

	/// Include objects deleted from the configuration in the differences
	#[arg(short = 'p', long)]
	pub with_prune: bool,

	/// List environments with changes
	#[arg(long)]
	pub list_modified_envs: bool,
}

/// Run the diff command.
pub fn run<W: Write>(_args: DiffArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
