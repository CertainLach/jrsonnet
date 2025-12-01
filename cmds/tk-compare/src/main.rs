use anyhow::Result;
use clap::Parser;

mod config;
mod report;
mod runner;

use config::Config;
use report::CommandReport;

#[derive(Parser)]
#[command(name = "tk-compare")]
#[command(about = "Integration testing and benchmarking tool for comparing two executables", long_about = None)]
#[command(version)]
struct Cli {
	/// Path to the config file
	config: String,

	/// Keep workspace directory after tests complete
	#[arg(long)]
	keep_workspace: bool,
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	// Load config
	let config = Config::from_file(&cli.config)?;

	// Verify executables exist and convert to absolute paths
	use std::path::Path;
	let exec1_path = Path::new(&config.tk_exec_1);
	if !exec1_path.exists() {
		anyhow::bail!("Executable not found: {}", config.tk_exec_1);
	}
	let exec1_absolute = std::fs::canonicalize(exec1_path)?;

	let exec2_path = Path::new(&config.tk_exec_2);
	if !exec2_path.exists() {
		anyhow::bail!("Executable not found: {}", config.tk_exec_2);
	}
	let exec2_absolute = std::fs::canonicalize(exec2_path)?;

	let exec1_str = exec1_absolute.to_string_lossy().to_string();
	let exec2_str = exec2_absolute.to_string_lossy().to_string();

	println!("Comparing executables:");
	println!("  exec1: {}", exec1_str);
	println!("  exec2: {}", exec2_str);
	if let Some(ref wd) = config.working_dir {
		println!("  working_dir: {}", wd);
	}
	println!("  commands: {}\n", config.commands.len());

	let mut reports = Vec::new();

	// Create workspace directories for each executable (only if no working_dir specified)
	// When working_dir is specified, both executables run in the same directory
	let (workspace1, workspace2) = if config.working_dir.is_none() {
		// Clean up old workspace if it exists
		if std::path::Path::new(".tk-compare-workspace").exists() {
			std::fs::remove_dir_all(".tk-compare-workspace")?;
		}
		(
			Some(".tk-compare-workspace/exec1"),
			Some(".tk-compare-workspace/exec2"),
		)
	} else {
		(None, None)
	};

	// Run each command
	for (index, command) in config.commands.iter().enumerate() {
		let runs = if command.runs == 0 { 1 } else { command.runs };

		if runs > 1 {
			println!(
				"Running command {}/{}: {} ({} runs)",
				index + 1,
				config.commands.len(),
				command.as_string(),
				runs
			);
		} else {
			println!(
				"Running command {}/{}: {}",
				index + 1,
				config.commands.len(),
				command.as_string()
			);
		}

		let mut exec1_durations = Vec::new();
		let mut exec2_durations = Vec::new();
		let mut exit_code_matched = true;
		let mut stdout_matched = true;
		let mut result_dir_matched = None;
		let mut exec1_exit_code = 0;
		let mut exec2_exit_code = 0;
		let mut exec1_stderr = String::new();
		let mut exec2_stderr = String::new();

		// Run the command multiple times
		for run in 0..runs {
			if runs > 1 {
				print!("  Run {}/{}...\r", run + 1, runs);
				use std::io::Write;
				std::io::stdout().flush().ok();
			}

			// Run with exec1 in its workspace
			let result1 = runner::run_command(
				&exec1_str,
				&command.args,
				workspace1,
				config.working_dir.as_deref(),
			)?;

			// Run with exec2 in its workspace
			let result2 = runner::run_command(
				&exec2_str,
				&command.args,
				workspace2,
				config.working_dir.as_deref(),
			)?;

			exec1_durations.push(result1.duration);
			exec2_durations.push(result2.duration);

			// Check consistency across runs (use first run as baseline)
			if run == 0 {
				exit_code_matched = result1.exit_code == result2.exit_code;
				stdout_matched = result1.stdout == result2.stdout;
				exec1_exit_code = result1.exit_code;
				exec2_exit_code = result2.exit_code;
				exec1_stderr = result1.stderr;
				exec2_stderr = result2.stderr;

				// Compare result directories if specified (only on first run)
				result_dir_matched = if let Some(ref result_dir) = command.result_dir {
					if let (Some(ws1), Some(ws2)) = (workspace1, workspace2) {
						// Construct result directory paths within each workspace
						let dir1 = format!("{}/{}", ws1, result_dir);
						let dir2 = format!("{}/{}", ws2, result_dir);

						Some(runner::compare_directories(&dir1, &dir2)?)
					} else {
						// Can't compare result directories when using shared working_dir
						None
					}
				} else {
					None
				};
			} else {
				// Verify consistency
				if result1.exit_code != exec1_exit_code || result2.exit_code != exec2_exit_code {
					println!("\nWarning: Exit codes changed across runs!");
				}
				if result1.stdout != exec1_stderr.replace(&exec1_stderr, &result1.stdout)
					|| result2.stdout != exec2_stderr.replace(&exec2_stderr, &result2.stdout)
				{
					// Just a sanity check, we don't fail on this
				}
			}
		}

		if runs > 1 {
			println!("  Completed {} runs    ", runs);
		}

		let exec1_stats = report::RuntimeStats::from_durations(exec1_durations);
		let exec2_stats = report::RuntimeStats::from_durations(exec2_durations);

		let report = CommandReport {
			command: command.as_string(),
			runs,
			exit_code_matched,
			stdout_matched,
			result_dir_matched,
			exec1_stats,
			exec2_stats,
			exec1_exit_code,
			exec2_exit_code,
			exec1_stderr,
			exec2_stderr,
		};

		reports.push(report);
	}

	// Print individual reports
	for (index, report) in reports.iter().enumerate() {
		report.print(index);
	}

	// Print summary
	report::print_summary(&reports);

	// Clean up workspace unless --keep-workspace is specified
	if !cli.keep_workspace {
		if std::path::Path::new(".tk-compare-workspace").exists() {
			std::fs::remove_dir_all(".tk-compare-workspace")?;
		}
	} else {
		println!("\nWorkspace preserved at: .tk-compare-workspace/");
	}

	Ok(())
}
