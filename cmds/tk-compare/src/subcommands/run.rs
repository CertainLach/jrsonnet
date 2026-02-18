use std::{
	collections::HashMap,
	io::IsTerminal,
	path::{Path, PathBuf},
	process::Command as ProcessCommand,
	time::Instant,
};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

use crate::{
	cli::{DiscoveryModeArg, GlobalOptions, RunCli},
	common::{
		build_test_globs, cleanup_export_dirs, command_selected, find_output_dir_in_args,
		run_process_output_with_timeout,
	},
	config::{self, Config},
	constants::{COMMAND_TIMEOUT, COMPARE_TIMEOUT, RTK_EXEC_NAME, TK_EXEC_NAME},
	env, execution,
	mock_k8s::MockCluster,
	report::{self, CommandReport, ExecResult, SummaryEvent, TestEvent},
	types::Pair,
	workspace,
};

struct RunTempDir {
	path: PathBuf,
	keep: bool,
}

impl RunTempDir {
	fn create() -> Result<Self> {
		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap_or_default()
			.as_millis();
		let path =
			std::env::temp_dir().join(format!("tk-compare-run-{}-{}", std::process::id(), now));
		std::fs::create_dir_all(&path)?;
		Ok(Self { path, keep: false })
	}

	fn as_path(&self) -> &Path {
		&self.path
	}

	fn as_str(&self) -> &str {
		self.path.to_str().unwrap_or("/tmp")
	}

	fn preserve(&mut self) {
		self.keep = true;
	}
}

impl Drop for RunTempDir {
	fn drop(&mut self) {
		if self.keep {
			return;
		}
		let _ = std::fs::remove_dir_all(&self.path);
	}
}

pub async fn execute(cli: RunCli, global: &GlobalOptions) -> Result<()> {
	let human_output = std::io::stdout().is_terminal();
	let run_started = Instant::now();
	let env_config = env::EnvConfig::from_env();
	env_config.print_debug_status();
	env_config.print_filter_status();

	let config = Config::from_file(&cli.config)?;
	let mut run_tempdir = RunTempDir::create()?;

	let tk_exec = global
		.tk
		.clone()
		.unwrap_or_else(|| TK_EXEC_NAME.to_string());
	let rtk_exec = global
		.rtk
		.clone()
		.unwrap_or_else(|| RTK_EXEC_NAME.to_string());
	let jrsonnet_exec = global.jrsonnet_path.clone();

	let tk_absolute = resolve_executable_path(&tk_exec)
		.with_context(|| format!("Executable '{}' not found in PATH", tk_exec))?;
	let rtk_absolute = resolve_executable_path(&rtk_exec)
		.with_context(|| format!("Executable '{}' not found in PATH", rtk_exec))?;
	let jrsonnet_absolute = jrsonnet_exec
		.as_deref()
		.map(resolve_executable_path)
		.transpose()
		.with_context(|| {
			format!(
				"Executable '{}' not found in PATH",
				jrsonnet_exec.clone().unwrap_or_default()
			)
		})?;

	let tk_exec_str = tk_absolute.to_string_lossy().to_string();
	let rtk_exec_str = rtk_absolute.to_string_lossy().to_string();
	let jrsonnet_exec_str = jrsonnet_absolute
		.as_ref()
		.map(|path| path.to_string_lossy().to_string());
	let tk_compare_bin = std::env::current_exe()?.to_string_lossy().to_string();

	if human_output {
		eprintln!("Comparing executables:");
		eprintln!("  {}: {}", TK_EXEC_NAME, tk_exec_str);
		eprintln!("  {}: {}", RTK_EXEC_NAME, rtk_exec_str);
		if let Some(ref jrsonnet) = jrsonnet_exec_str {
			eprintln!("  jrsonnet: {}", jrsonnet);
		}
		if let Some(ref wd) = config.working_dir {
			eprintln!("  global working_dir: {}", wd);
		}
	}

	let all_commands = config.all_commands()?;
	let test_globs = build_test_globs(&cli.run.test)?;
	let commands_to_run: Vec<_> = all_commands
		.iter()
		.enumerate()
		.filter(|(_, rc)| {
			command_selected(rc, env_config.filter_regex.as_ref(), test_globs.as_ref())
		})
		.collect();
	if human_output {
		eprintln!("  total commands: {}", all_commands.len());
		eprintln!("  filtered commands: {}\n", commands_to_run.len());
	}

	let discovery_modes = cli.run.discovery_mode.clone();
	let total_runs: usize = commands_to_run
		.iter()
		.map(|(_, rc)| {
			if rc.command.cluster_dir.is_some() {
				discovery_modes.len()
			} else {
				1
			}
		})
		.sum();
	let counter_width = total_runs.max(1).to_string().len();
	let name_width = commands_to_run
		.iter()
		.map(|(_, rc)| {
			if rc.command.cluster_dir.is_some() {
				discovery_modes
					.iter()
					.map(|mode| format!("{}/{} ({})", rc.test_name, rc.basename, mode).len())
					.max()
					.unwrap_or_else(|| format!("{}/{}", rc.test_name, rc.basename).len())
			} else {
				format!("{}/{}", rc.test_name, rc.basename).len()
			}
		})
		.max()
		.unwrap_or(0);
	let progress = human_output.then(|| {
		let pb = ProgressBar::new(total_runs as u64);
		let style = ProgressStyle::with_template("{spinner:.cyan} [{pos}/{len}] [{msg}]")
			.expect("valid progress template")
			.tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]);
		pb.set_style(style);
		pb.enable_steady_tick(std::time::Duration::from_millis(100));
		pb
	});

	let mut reports = Vec::new();
	workspace::cleanup_all()?;
	let mut run_index = 0usize;

	for (index, resolved_cmd) in commands_to_run.iter() {
		let command = &resolved_cmd.command;
		let cmd_working_dir = resolved_cmd.working_dir.as_deref();

		let workspace = if resolved_cmd.workspace {
			Some(workspace::Workspace::new(TK_EXEC_NAME, RTK_EXEC_NAME))
		} else {
			None
		};
		if let Some(ref ws) = workspace {
			let _ = ws.clean();
			if let Some(working_dir) = cmd_working_dir {
				workspace::stage_working_dir(
					ws.first(),
					working_dir,
					jrsonnet_exec_str.as_deref(),
				)?;
				workspace::stage_working_dir(
					ws.second(),
					working_dir,
					jrsonnet_exec_str.as_deref(),
				)?;
			}
		}

		let modes_for_run: Vec<Option<DiscoveryModeArg>> = if command.cluster_dir.is_some() {
			discovery_modes.iter().copied().map(Some).collect()
		} else {
			vec![None]
		};
		for discovery_mode in modes_for_run {
			run_index += 1;
			let case_name = discovery_mode
				.map(|m| format!("{} ({})", resolved_cmd.basename, m))
				.unwrap_or_else(|| resolved_cmd.basename.clone());
			if let Some(pb) = progress.as_ref() {
				pb.set_position(run_index as u64);
				pb.set_message(format!("{}/{}: running", resolved_cmd.test_name, case_name));
			}

			let (result1, result2, stdout_matched, compare_stdout, compare_stderr, mock_http) =
				{
					let tk_destination = run_tempdir
						.as_path()
						.join("outputs")
						.join(TK_EXEC_NAME)
						.join(&resolved_cmd.basename)
						.to_string_lossy()
						.to_string();
					let rtk_destination = run_tempdir
						.as_path()
						.join("outputs")
						.join(RTK_EXEC_NAME)
						.join(&resolved_cmd.basename)
						.to_string_lossy()
						.to_string();

					cleanup_export_dirs(
						command,
						&tk_destination,
						&rtk_destination,
						&resolved_cmd.basename,
						&resolved_cmd.testcase,
						run_tempdir.as_path(),
						cmd_working_dir,
					);

					let args1 = substitute_runtime_tokens(
						command.args_for_exec(
							&tk_destination,
							&resolved_cmd.basename,
							&resolved_cmd.testcase,
							run_tempdir.as_str(),
							cmd_working_dir,
						),
						jrsonnet_exec_str.as_deref(),
					);
					let args2 = substitute_runtime_tokens(
						command.args_for_exec(
							&rtk_destination,
							&resolved_cmd.basename,
							&resolved_cmd.testcase,
							run_tempdir.as_str(),
							cmd_working_dir,
						),
						jrsonnet_exec_str.as_deref(),
					);

					let mock_cluster =
						if let Some(ref cluster_dir) = command.cluster_dir {
							let mode = discovery_mode.map(Into::into).unwrap_or_default();
							Some(MockCluster::start(cluster_dir, mode).await.with_context(
								|| format!("failed to start mock K8s server for {}", cluster_dir),
							)?)
						} else {
							None
						};
					let env_vars: Option<HashMap<String, String>> =
						mock_cluster.as_ref().map(|mc| mc.env_vars());

					let result1 = execution::run_command_with_env(
						&tk_exec_str,
						&args1,
						workspace.as_ref().map(|w| w.first()),
						cmd_working_dir,
						env_vars.as_ref(),
						COMMAND_TIMEOUT,
					)?;

					let rtk_config_written = command.write_rtk_config(cmd_working_dir);
					let result2 = execution::run_command_with_env(
						&rtk_exec_str,
						&args2,
						workspace.as_ref().map(|w| w.second()),
						cmd_working_dir,
						env_vars.as_ref(),
						COMMAND_TIMEOUT,
					)?;
					if rtk_config_written.is_some() {
						config::Command::cleanup_rtk_config(cmd_working_dir);
					}

					let run_artifacts = run_tempdir
						.as_path()
						.join("artifacts")
						.join(format!("cmd-{}", index + 1));
					std::fs::create_dir_all(&run_artifacts)?;
					let tk_stdout = run_artifacts.join("tk.stdout");
					let rtk_stdout = run_artifacts.join("rtk.stdout");
					std::fs::write(&tk_stdout, &result1.stdout)?;
					std::fs::write(&rtk_stdout, &result2.stdout)?;

					let left = find_output_dir_in_args(&args1)
						.filter(|d| Path::new(d).is_dir())
						.unwrap_or_else(|| tk_stdout.to_string_lossy().to_string());
					let right = find_output_dir_in_args(&args2)
						.filter(|d| Path::new(d).is_dir())
						.unwrap_or_else(|| rtk_stdout.to_string_lossy().to_string());

					let mut compare_argv = command.compare_argv(
						&tk_compare_bin,
						&resolved_cmd.basename,
						&resolved_cmd.testcase,
						run_tempdir.as_str(),
					);
					if compare_argv.is_empty() {
						bail!("compare argv resolved empty for {}", command.display_name());
					}
					let compare_exec = compare_argv.remove(0);
					compare_argv.push(left.clone());
					compare_argv.push(right.clone());

					let mut compare_cmd = ProcessCommand::new(&compare_exec);
					compare_cmd.args(&compare_argv);

					let compare_out =
						run_process_output_with_timeout(&mut compare_cmd, COMPARE_TIMEOUT)
							.with_context(|| {
								format!(
									"Failed to run comparer: {} {:?}",
									compare_exec, compare_argv
								)
							})?;

					let compare_stdout = String::from_utf8_lossy(&compare_out.stdout).to_string();
					let compare_stderr = String::from_utf8_lossy(&compare_out.stderr).to_string();
					let run_matched = compare_out.status.success();
					let mock_http = mock_cluster
						.as_ref()
						.map(|cluster| cluster.http_exchanges())
						.unwrap_or_default();
					(
						result1,
						result2,
						run_matched,
						compare_stdout,
						compare_stderr,
						mock_http,
					)
				};

			let exit_code_matched = result1.exit_code == result2.exit_code;
			let both_failed_unexpectedly = !command.expect_error && !exit_code_matched;
			let report = CommandReport {
				command: format!("{}|{} / {}", TK_EXEC_NAME, RTK_EXEC_NAME, case_name),
				exit_code_matched,
				exit_codes_consistent: true,
				stdout_matched,
				stdout_similarity: None,
				both_failed_unexpectedly,
				execs: Pair::new(
					ExecResult {
						name: TK_EXEC_NAME.to_string(),
						duration: result1.duration,
						exit_code: result1.exit_code,
						stdout: result1.stdout,
						stderr: result1.stderr,
					},
					ExecResult {
						name: RTK_EXEC_NAME.to_string(),
						duration: result2.duration,
						exit_code: result2.exit_code,
						stdout: result2.stdout,
						stderr: result2.stderr,
					},
				),
			};
			let event = report::build_test_event(
				run_index,
				total_runs,
				&resolved_cmd.test_name,
				&case_name,
				run_started.elapsed().as_millis(),
				&report,
				&compare_stdout,
				&compare_stderr,
				mock_http,
			);

			if !human_output {
				emit_json_report(&event)?;
				reports.push(report);
				continue;
			}

			emit_human_report(progress.as_ref(), counter_width, name_width, &event);
			reports.push(report);
		}
	}

	let stats = report::summarize(&reports);
	let summary_event = report::build_summary_event(&stats, run_started.elapsed().as_millis());
	if !human_output {
		emit_json_summary(&summary_event)?;
	}
	if human_output {
		emit_human_summary(progress.as_ref(), &summary_event);
	}

	if workspace::should_keep(cli.run.keep_workspace) {
		run_tempdir.preserve();
		workspace::print_preserved_message();
		eprintln!("Temp output preserved at: {}", run_tempdir.as_str());
		return Ok(());
	}

	workspace::cleanup_all()?;
	for (_, resolved_cmd) in &commands_to_run {
		let tk_destination = run_tempdir
			.as_path()
			.join("outputs")
			.join(TK_EXEC_NAME)
			.join(&resolved_cmd.basename)
			.to_string_lossy()
			.to_string();
		let rtk_destination = run_tempdir
			.as_path()
			.join("outputs")
			.join(RTK_EXEC_NAME)
			.join(&resolved_cmd.basename)
			.to_string_lossy()
			.to_string();

		cleanup_export_dirs(
			&resolved_cmd.command,
			&tk_destination,
			&rtk_destination,
			&resolved_cmd.basename,
			&resolved_cmd.testcase,
			run_tempdir.as_path(),
			resolved_cmd.working_dir.as_deref(),
		);
	}

	Ok(())
}

fn resolve_executable_path(executable: &str) -> Result<std::path::PathBuf> {
	let candidate = Path::new(executable);
	if candidate.components().count() > 1 || candidate.is_absolute() {
		if candidate.exists() {
			return Ok(std::fs::canonicalize(candidate)?);
		}
	}
	which::which(executable).map_err(Into::into)
}

fn substitute_runtime_tokens(args: Vec<String>, jrsonnet_path: Option<&str>) -> Vec<String> {
	let replacement = jrsonnet_path.unwrap_or("");
	args.into_iter()
		.map(|arg| arg.replace("{{jrsonnet}}", replacement))
		.collect()
}

fn indent_block(text: &str, prefix: &str) -> String {
	text.lines()
		.map(|line| format!("{prefix}{line}"))
		.collect::<Vec<_>>()
		.join("\n")
}

fn emit_human_report(
	progress: Option<&ProgressBar>,
	counter_width: usize,
	name_width: usize,
	event: &TestEvent,
) {
	use colored::Colorize;

	let icon = if event.passed {
		"✓".green().bold().to_string()
	} else {
		"✗".red().bold().to_string()
	};
	let full_name = format!("{}/{}", event.suite, event.name);
	let name_padded = format!("{:name_width$}", full_name);
	let tk_ms = event.tk.duration_ms;
	let rtk_ms = event.rtk.duration_ms;
	let delta_ms = rtk_ms as i128 - tk_ms as i128;
	let delta_colored = if delta_ms < 0 {
		format!("{:+}ms", delta_ms).green().to_string()
	} else if delta_ms > 0 {
		format!("{:+}ms", delta_ms).red().to_string()
	} else {
		format!("{:+}ms", delta_ms).normal().to_string()
	};
	let flag_suffix = failure_flag_suffix(event);
	let elapsed =
		report::format_duration(std::time::Duration::from_millis(event.elapsed_ms as u64));
	let line = format!(
		"{} [{:>counter_width$}/{}] [{}]: tk={}ms rtk={}ms Δ={} [{}]{}",
		icon,
		event.index,
		event.total,
		name_padded.bold().cyan(),
		tk_ms,
		rtk_ms,
		delta_colored,
		elapsed,
		flag_suffix
	);
	if let Some(pb) = progress {
		pb.println(line);
	}
	if let Some(compare) = event.compare.as_ref() {
		print_colored_compare_output(progress, &compare.stdout);
		print_colored_compare_output(progress, &compare.stderr);
	}
	if event.passed {
		return;
	}
	print_labeled_if_present(progress, "tk stderr:", &event.tk.stderr);
	print_labeled_if_present(progress, "rtk stderr:", &event.rtk.stderr);
	print_mock_http_if_present(progress, &event.mock_http);
}

fn emit_json_report(event: &TestEvent) -> Result<()> {
	println!("{}", serde_json::to_string(event)?);
	Ok(())
}

fn emit_human_summary(progress: Option<&ProgressBar>, event: &SummaryEvent) {
	use colored::Colorize;

	if let Some(pb) = progress {
		pb.finish_and_clear();
	}
	let status = if event.all_passed {
		"✓".green().bold().to_string()
	} else {
		"✗".red().bold().to_string()
	};
	println!(
		"{} [{}/{}] [run / total]: exit {}/{}, output {}/{}. [{}]",
		status,
		event.total,
		event.total,
		event.exit_code_matches,
		event.total,
		event.stdout_matches,
		event.total,
		report::format_duration(std::time::Duration::from_millis(event.elapsed_ms as u64))
	);
}

fn emit_json_summary(event: &SummaryEvent) -> Result<()> {
	println!("{}", serde_json::to_string(&event)?);
	Ok(())
}

fn print_indented_if_present(progress: Option<&ProgressBar>, text: &str) {
	let Some(pb) = progress else {
		return;
	};
	if text.trim().is_empty() {
		return;
	}
	pb.println(indent_block(text, "    "));
}

fn print_colored_compare_output(progress: Option<&ProgressBar>, text: &str) {
	let Some(pb) = progress else {
		return;
	};
	if text.trim().is_empty() {
		return;
	}

	for line in text.lines() {
		let styled = crate::output::stylize_compare_line(line, true);
		pb.println(format!("    {styled}"));
	}
}

fn print_labeled_if_present(progress: Option<&ProgressBar>, label: &str, text: &str) {
	if text.trim().is_empty() {
		return;
	}
	print_indented_if_present(progress, label);
	print_indented_if_present(progress, text);
}

fn failure_flag_suffix(event: &TestEvent) -> String {
	let flags = &event.failure_flags;
	if flags.is_empty() {
		return String::new();
	}
	format!(" ({})", flags.join(","))
}

fn print_mock_http_if_present(
	progress: Option<&ProgressBar>,
	exchanges: &[k8s_mock::HttpExchange],
) {
	let Some(pb) = progress else {
		return;
	};
	if exchanges.is_empty() {
		return;
	}
	pb.println("    mock HTTP exchanges:");
	for (index, exchange) in exchanges.iter().enumerate() {
		let query = exchange
			.query
			.as_ref()
			.map(|value| format!("?{}", value))
			.unwrap_or_default();
		pb.println(format!(
			"      [{}] {} {}{} -> {}",
			index + 1,
			exchange.method,
			exchange.path,
			query,
			exchange.response_status
		));
		if let Some(accept) = exchange.accept.as_ref() {
			pb.println(format!("        accept: {}", accept));
		}
		if let Some(content_type) = exchange.content_type.as_ref() {
			pb.println(format!("        content-type: {}", content_type));
		}
		if !exchange.request_body.trim().is_empty() {
			pb.println("        request body:");
			pb.println(indent_block(&exchange.request_body, "          "));
		}
		if !exchange.response_body.trim().is_empty() {
			pb.println("        response body:");
			pb.println(indent_block(&exchange.response_body, "          "));
		}
	}
}
