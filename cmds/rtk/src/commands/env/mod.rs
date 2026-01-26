//! Env command handler.

use std::io::Write;

use anyhow::Result;
use clap::{Args, Subcommand};

pub mod add;
pub mod list;
pub mod remove;
pub mod set;

#[derive(Args)]
pub struct EnvArgs {
	#[command(subcommand)]
	pub command: EnvCommands,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

#[derive(Subcommand)]
pub enum EnvCommands {
	/// Create a new environment
	Add(add::AddArgs),

	/// List environments relative to current dir or <path>
	#[command(alias = "ls")]
	List(list::ListArgs),

	/// Delete an environment
	#[command(alias = "rm")]
	Remove(remove::RemoveArgs),

	/// Update properties of an environment
	Set(set::SetArgs),
}

/// Run the env command.
pub fn run<W: Write>(args: EnvArgs, writer: W) -> Result<()> {
	match args.command {
		EnvCommands::Add(add_args) => add::run(add_args, writer),
		EnvCommands::List(list_args) => list::run(list_args, writer),
		EnvCommands::Remove(remove_args) => remove::run(remove_args, writer),
		EnvCommands::Set(set_args) => set::run(set_args, writer),
	}
}
