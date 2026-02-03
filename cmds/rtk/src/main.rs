use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::util::BrokenPipeGuard;
use jrsonnet_evaluator::FileImportResolver;

mod commands;
mod config;
mod discover;
mod env;
mod eval;
mod export;
mod importers;
mod imports;
mod jpath;
mod k8s;
mod spec;
mod tanka;
mod telemetry;
#[cfg(test)]
pub mod test_utils;
mod yaml;

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
	/// Log level (error, warn, info, debug, trace). Falls back to RUST_LOG env var.
	#[arg(long, global = true)]
	log_level: Option<tracing::Level>,

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

fn main() -> Result<()> {
	let cli = Cli::parse();

	// Initialize telemetry (tracing + optional OpenTelemetry)
	// Guard ensures traces are flushed on exit
	let _telemetry_guard = telemetry::init(cli.log_level)?;

	let stdout = BrokenPipeGuard::new(std::io::stdout());

	match cli.command {
		Commands::Apply(args) => commands::apply::run(args, stdout),
		Commands::Show(args) => commands::show::run(args, stdout),
		Commands::Diff(args) => {
			if commands::diff::run(args, stdout)? {
				std::process::exit(commands::diff::EXIT_CODE_DIFF_FOUND);
			}
			Ok(())
		}
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
