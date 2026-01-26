//! Tool command handler.

use std::io::Write;

use anyhow::Result;
use clap::{Args, Subcommand};

pub mod charts;
pub mod importers;
pub mod importers_count;
pub mod imports;
pub mod jpath;

#[derive(Args)]
pub struct ToolArgs {
	#[command(subcommand)]
	pub command: ToolCommands,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

#[derive(Subcommand)]
pub enum ToolCommands {
	/// Export JSONNET_PATH for use with other jsonnet tools
	Jpath(jpath::JpathArgs),

	/// List all transitive imports of an environment
	Imports(imports::ImportsArgs),

	/// List all environments that either directly or transitively import the given files
	Importers(importers::ImportersArgs),

	/// For each file in the given directory, list the number of environments that import it
	ImportersCount(importers_count::ImportersCountArgs),

	/// Declarative vendoring of Helm Charts
	Charts(charts::ChartsArgs),
}

/// Run the tool command.
pub fn run<W: Write>(args: ToolArgs, writer: W) -> Result<()> {
	match args.command {
		ToolCommands::Jpath(jpath_args) => jpath::run(jpath_args, writer),
		ToolCommands::Imports(imports_args) => imports::run(imports_args, writer),
		ToolCommands::Importers(importers_args) => importers::run(importers_args, writer),
		ToolCommands::ImportersCount(importers_count_args) => {
			importers_count::run(importers_count_args, writer)
		}
		ToolCommands::Charts(charts_args) => charts::run(charts_args, writer),
	}
}
