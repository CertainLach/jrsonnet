//! Complete command handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct CompleteArgs {
	#[arg(long)]
	pub remove: bool,
}

/// Run the complete command.
pub fn run<W: Write>(_args: CompleteArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
