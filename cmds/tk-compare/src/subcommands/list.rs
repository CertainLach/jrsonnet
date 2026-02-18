use std::io::IsTerminal;

use anyhow::Result;
use colored::Colorize;

use crate::{
	cli::ListCli,
	common::{build_test_globs, command_selected},
	config::Config,
	env,
};

pub fn execute(cli: ListCli) -> Result<()> {
	let env_config = env::EnvConfig::from_env();
	env_config.print_filter_status();

	let config = Config::from_file(&cli.config)?;
	let all_commands = config.all_commands()?;
	let test_globs = build_test_globs(&cli.test)?;
	let commands: Vec<_> = all_commands
		.iter()
		.enumerate()
		.filter(|(_, rc)| {
			command_selected(rc, env_config.filter_regex.as_ref(), test_globs.as_ref())
		})
		.collect();

	for (index, resolved_cmd) in &commands {
		let index_label = format!("{:>3}.", index + 1);
		if std::io::stdout().is_terminal() {
			println!(
				"{} {} {} {}",
				index_label.dimmed(),
				resolved_cmd.test_name.bold().cyan(),
				"/".dimmed(),
				resolved_cmd.basename.bold().green()
			);
			continue;
		}
		println!(
			"{} {} / {}",
			index_label, resolved_cmd.test_name, resolved_cmd.basename
		);
	}

	let noun = if commands.len() == 1 {
		"command"
	} else {
		"commands"
	};
	eprintln!("listed {} {}", commands.len(), noun);
	Ok(())
}
