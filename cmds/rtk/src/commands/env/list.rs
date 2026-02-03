//! Env list subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

use crate::commands::util::UnimplementedArgs;
use crate::env as env_impl;

#[derive(Args)]
pub struct ListArgs {
	/// Path to search for environments
	pub path: Option<String>,

	/// Set code value of extVar (Format: key=<code>)
	#[arg(long)]
	pub ext_code: Vec<String>,

	/// Set string value of extVar (Format: key=value)
	#[arg(short = 'V', long)]
	pub ext_str: Vec<String>,

	/// JSON output
	#[arg(long)]
	pub json: bool,

	/// Use `go` to use native go-jsonnet implementation and `binary:<path>` to delegate evaluation to a binary (with the same API as the regular `jsonnet` binary)
	#[arg(long, default_value = "go")]
	pub jsonnet_implementation: String,

	/// Jsonnet VM max stack. Increase this if you get: max stack frames exceeded
	#[arg(long, default_value = "500")]
	pub max_stack: i32,

	/// Plain names output
	#[arg(long)]
	pub names: bool,

	/// Label selector. Uses the same syntax as kubectl does
	#[arg(short = 'l', long)]
	pub selector: Option<String>,

	/// Set code value of top level function (Format: key=<code>)
	#[arg(long)]
	pub tla_code: Vec<String>,

	/// Set string value of top level function (Format: key=value)
	#[arg(short = 'A', long)]
	pub tla_str: Vec<String>,
}

/// Run the env list subcommand.
pub fn run<W: Write>(args: ListArgs, writer: W) -> Result<()> {
	UnimplementedArgs {
		jsonnet_implementation: Some(&args.jsonnet_implementation),
		cache_envs: None,
		cache_path: None,
		mem_ballast_size_bytes: None,
	}
	.warn_if_set();

	env_impl::list_envs_to_writer(args.path, args.json, writer)
}

#[cfg(test)]
mod tests {
	use assert_matches::assert_matches;

	use super::*;
	use crate::{commands::util::BrokenPipeGuard, test_utils::BrokenPipeWriter};

	fn make_args() -> ListArgs {
		ListArgs {
			path: None,
			ext_code: vec![],
			ext_str: vec![],
			json: false,
			jsonnet_implementation: "go".to_string(),
			max_stack: 500,
			names: false,
			selector: None,
			tla_code: vec![],
			tla_str: vec![],
		}
	}

	#[test]
	fn test_list_exits_cleanly_on_broken_pipe() {
		let args = make_args();
		// Wrap BrokenPipeWriter with BrokenPipeGuard to test the guard handles broken pipes
		let writer = BrokenPipeGuard::new(BrokenPipeWriter);
		let result = run(args, writer);

		// The command should exit cleanly on broken pipe, not panic or error
		assert_matches!(result, Ok(()));
	}
}
