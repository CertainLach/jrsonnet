//! Env remove subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

use crate::env::env_remove;

#[derive(Args)]
pub struct RemoveArgs {
	/// Path(s) to the environment(s) to remove
	pub paths: Vec<String>,
}

/// Run the env remove subcommand.
pub fn run<W: Write>(args: RemoveArgs, _writer: W) -> Result<()> {
	env_remove(&args.paths)
}
