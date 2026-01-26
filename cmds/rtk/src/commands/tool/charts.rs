//! Charts subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ChartsArgs {
	#[command(subcommand)]
	pub command: ChartsCommands,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

#[derive(Subcommand)]
pub enum ChartsCommands {
	/// Create a new Chartfile
	Init(ChartsInitArgs),

	/// Adds Charts to the chartfile
	Add(ChartsAddArgs),

	/// Adds a repository to the chartfile
	AddRepo(ChartsAddRepoArgs),

	/// Download Charts to a local folder
	Vendor(ChartsVendorArgs),

	/// Displays the current manifest
	Config(ChartsConfigArgs),

	/// Check required charts for updated versions
	VersionCheck(ChartsVersionCheckArgs),
}

#[derive(Args)]
pub struct ChartsInitArgs {
	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

#[derive(Args)]
pub struct ChartsAddArgs {
	/// Charts to add (format: chart@version)
	pub charts: Vec<String>,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Specify a local helm repository config file to use instead of the repositories in the chartfile.yaml. For use with private repositories
	#[arg(long)]
	pub repository_config: Option<String>,
}

#[derive(Args)]
pub struct ChartsAddRepoArgs {
	/// Repository name
	pub name: String,

	/// Repository URL
	pub url: String,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

#[derive(Args)]
pub struct ChartsVendorArgs {
	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Also remove non-vendored files from the destination directory
	#[arg(long)]
	pub prune: bool,

	/// Specify a local helm repository config file to use instead of the repositories in the chartfile.yaml. For use with private repositories
	#[arg(long)]
	pub repository_config: Option<String>,
}

#[derive(Args)]
pub struct ChartsConfigArgs {
	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,
}

#[derive(Args)]
pub struct ChartsVersionCheckArgs {
	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Pretty print json output with indents
	#[arg(long)]
	pub pretty_print: bool,

	/// Specify a local helm repository config file to use instead of the repositories in the chartfile.yaml. For use with private repositories
	#[arg(long)]
	pub repository_config: Option<String>,
}

/// Run the charts subcommand.
pub fn run<W: Write>(args: ChartsArgs, writer: W) -> Result<()> {
	match args.command {
		ChartsCommands::Init(init_args) => run_init(init_args, writer),
		ChartsCommands::Add(add_args) => run_add(add_args, writer),
		ChartsCommands::AddRepo(add_repo_args) => run_add_repo(add_repo_args, writer),
		ChartsCommands::Vendor(vendor_args) => run_vendor(vendor_args, writer),
		ChartsCommands::Config(config_args) => run_config(config_args, writer),
		ChartsCommands::VersionCheck(version_check_args) => {
			run_version_check(version_check_args, writer)
		}
	}
}

fn run_init<W: Write>(_args: ChartsInitArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}

fn run_add<W: Write>(_args: ChartsAddArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}

fn run_add_repo<W: Write>(_args: ChartsAddRepoArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}

fn run_vendor<W: Write>(_args: ChartsVendorArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}

fn run_config<W: Write>(_args: ChartsConfigArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}

fn run_version_check<W: Write>(_args: ChartsVersionCheckArgs, _writer: W) -> Result<()> {
	anyhow::bail!("not implemented")
}
