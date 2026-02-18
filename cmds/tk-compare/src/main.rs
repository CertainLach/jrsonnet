use anyhow::{Context, Result};
use clap::Parser;

mod cli;
mod common;
mod comparison;
mod config;
mod constants;
mod env;
mod execution;
mod mock_k8s;
mod output;
mod report;
mod subcommands;
mod telemetry;
mod types;
mod workspace;

use cli::{Cli, Commands, RunCli};

#[tokio::main]
async fn main() -> Result<()> {
	telemetry::init();
	let cli = Cli::parse();
	let global = cli.global.clone();
	match cli.command {
		Some(Commands::Run(run)) => subcommands::run::execute(run, &global).await,
		Some(Commands::List(list)) => subcommands::list::execute(list),
		Some(Commands::Compare(compare)) => {
			std::process::exit(subcommands::compare::execute(compare));
		}
		Some(Commands::GoldenFixtures(golden_fixtures)) => {
			subcommands::golden_fixtures::execute(golden_fixtures, &global)
		}
		None => {
			let config = cli.config.context("missing required argument <CONFIG>")?;
			subcommands::run::execute(
				RunCli {
					config,
					run: cli.run,
				},
				&global,
			)
			.await
		}
	}
}
