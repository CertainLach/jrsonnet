use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::util::BrokenPipeGuard;
use jrsonnet_evaluator::FileImportResolver;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod commands;
mod config;
mod discover;
mod env;
mod eval;
mod export;
mod importers;
mod jpath;
mod spec;
mod tanka;
#[cfg(test)]
pub mod test_utils;

#[cfg(all(
	target_os = "linux",
	feature = "mimalloc",
	not(feature = "system-alloc")
))]
#[global_allocator]
static GLOBAL: mimallocator::Mimalloc = mimallocator::Mimalloc;

#[derive(Parser)]
#[command(name = "rtk")]
#[command(about = "Tanka dummy CLI", long_about = None)]
#[command(version = env!("RTK_VERSION"))]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Apply the configuration to the cluster
	Apply(commands::apply::ApplyArgs),

	/// Jsonnet as yaml
	Show(commands::show::ShowArgs),

	/// Differences between the configuration and the cluster
	Diff(commands::diff::DiffArgs),

	/// Delete resources removed from Jsonnet
	Prune(commands::prune::PruneArgs),

	/// Delete the environment from cluster
	Delete(commands::delete::DeleteArgs),

	/// Manipulate environments
	Env(commands::env::EnvArgs),

	/// Display an overview of the environment, including contents and metadata
	Status(commands::status::StatusArgs),

	/// Export environments found in path(s)
	Export(commands::export::ExportArgs),

	/// Format Jsonnet code
	Fmt(commands::fmt::FmtArgs),

	/// Lint Jsonnet code
	Lint(commands::lint::LintArgs),

	/// Evaluate the jsonnet to json
	Eval(commands::eval::EvalArgs),

	/// Create the directory structure
	Init(commands::init::InitArgs),

	/// Handy utilities for working with jsonnet
	Tool(commands::tool::ToolArgs),

	/// Install CLI completions
	Complete(commands::complete::CompleteArgs),
}

/// Initialize tracing with logfmt output format
fn init_logger(level: &str) {
	let level = match level.to_lowercase().as_str() {
		"trace" => "trace",
		"debug" => "debug",
		"info" => "info",
		"warn" | "warning" => "warn",
		"error" => "error",
		_ => "info",
	};

	let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

	tracing_subscriber::registry()
		.with(filter)
		.with(tracing_logfmt::layer())
		.init();
}

/// Extract log level from command
fn get_log_level(cmd: &Commands) -> &str {
	match cmd {
		Commands::Apply(args) => &args.log_level,
		Commands::Show(args) => &args.log_level,
		Commands::Diff(args) => &args.log_level,
		Commands::Prune(args) => &args.log_level,
		Commands::Delete(args) => &args.log_level,
		Commands::Env(args) => &args.log_level,
		Commands::Status(args) => &args.log_level,
		Commands::Export(args) => &args.log_level,
		Commands::Fmt(args) => &args.log_level,
		Commands::Lint(args) => &args.log_level,
		Commands::Eval(args) => &args.log_level,
		Commands::Init(args) => &args.log_level,
		Commands::Tool(args) => {
			// Prefer log_level from subcommand if available, otherwise use ToolArgs log_level
			match &args.command {
				crate::commands::tool::ToolCommands::Importers(importers_args) => {
					&importers_args.log_level
				}
				crate::commands::tool::ToolCommands::Imports(imports_args) => {
					&imports_args.log_level
				}
				crate::commands::tool::ToolCommands::Jpath(jpath_args) => &jpath_args.log_level,
				crate::commands::tool::ToolCommands::ImportersCount(importers_count_args) => {
					&importers_count_args.log_level
				}
				crate::commands::tool::ToolCommands::Charts(_) => &args.log_level,
			}
		}
		Commands::Complete(_) => "info",
	}
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	// Initialize logger based on log level
	init_logger(get_log_level(&cli.command));

	let stdout = BrokenPipeGuard::new(std::io::stdout());

	match cli.command {
		Commands::Apply(args) => commands::apply::run(args, stdout),
		Commands::Show(args) => commands::show::run(args, stdout),
		Commands::Diff(args) => commands::diff::run(args, stdout),
		Commands::Prune(args) => commands::prune::run(args, stdout),
		Commands::Delete(args) => commands::delete::run(args, stdout),
		Commands::Env(args) => commands::env::run(args, stdout),
		Commands::Status(args) => commands::status::run(args, stdout),
		Commands::Export(args) => commands::export::run(args, stdout),
		Commands::Fmt(args) => commands::fmt::run(args, stdout),
		Commands::Lint(args) => commands::lint::run(args, stdout),
		Commands::Eval(args) => {
			let jpath_result = jpath::resolve(&args.path)?;
			let import_resolver = FileImportResolver::new(jpath_result.import_paths.clone());
			let spec = eval::load_spec(&jpath_result)?;
			let opts = commands::eval::build_eval_opts(&args);
			commands::eval::run(
				import_resolver,
				&jpath_result.entrypoint,
				Some(&jpath_result.base),
				spec,
				opts,
				stdout,
			)
		}
		Commands::Init(args) => commands::init::run(args, stdout),
		Commands::Tool(args) => commands::tool::run(args, stdout),
		Commands::Complete(args) => commands::complete::run(args, stdout),
	}
}
