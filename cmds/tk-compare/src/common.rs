use std::{
	io::Read,
	path::Path,
	process::{Command as ProcessCommand, Stdio},
	thread,
	time::Duration,
};

use anyhow::{bail, Context, Result};
use globset::{GlobSet, GlobSetBuilder};

use crate::{cli::TestGlob, config};

pub fn build_test_globs(patterns: &[TestGlob]) -> Result<Option<GlobSet>> {
	if patterns.is_empty() {
		return Ok(None);
	}

	let mut builder = GlobSetBuilder::new();
	for pattern in patterns {
		builder.add(pattern.as_glob().clone());
	}

	Ok(Some(
		builder.build().context("failed to build --test glob set")?,
	))
}

pub fn command_selected(
	command: &config::ResolvedCommand,
	filter_regex: Option<&regex::Regex>,
	test_globs: Option<&GlobSet>,
) -> bool {
	let regex_matches = filter_regex.is_none_or(|re| re.is_match(&command.test_name));
	let glob_matches = test_globs.is_none_or(|set| {
		command_test_selectors(command)
			.into_iter()
			.any(|value| set.is_match(value))
	});
	regex_matches && glob_matches
}

pub fn command_test_selectors(command: &config::ResolvedCommand) -> Vec<String> {
	vec![
		command.basename.clone(),
		command.testcase.clone(),
		command.test_name.clone(),
		format!("{}/{}", command.test_name, command.basename),
	]
}

pub fn find_output_dir_in_args(args: &[String]) -> Option<String> {
	let mut found_export = false;
	for arg in args {
		if arg == "export" {
			found_export = true;
			continue;
		}
		if found_export && !arg.starts_with('-') {
			return Some(arg.clone());
		}
	}
	None
}

pub fn run_process_output_with_timeout(
	command: &mut ProcessCommand,
	timeout: Duration,
) -> Result<std::process::Output> {
	command.stdout(Stdio::piped()).stderr(Stdio::piped());
	let mut child = command
		.spawn()
		.context("failed to spawn process for output capture")?;

	let mut stdout = child
		.stdout
		.take()
		.context("failed to capture process stdout")?;
	let mut stderr = child
		.stderr
		.take()
		.context("failed to capture process stderr")?;

	let stdout_reader = thread::spawn(move || {
		let mut buf = Vec::new();
		let _ = stdout.read_to_end(&mut buf);
		buf
	});
	let stderr_reader = thread::spawn(move || {
		let mut buf = Vec::new();
		let _ = stderr.read_to_end(&mut buf);
		buf
	});

	let start = std::time::Instant::now();
	loop {
		if let Some(status) = child.try_wait()? {
			let stdout = stdout_reader
				.join()
				.map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))?;
			let stderr = stderr_reader
				.join()
				.map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))?;
			return Ok(std::process::Output {
				status,
				stdout,
				stderr,
			});
		}

		if start.elapsed() > timeout {
			let _ = child.kill();
			let _ = child.wait();
			let _ = stdout_reader.join();
			let _ = stderr_reader.join();
			bail!("process exceeded timeout of {:?}", timeout);
		}

		std::thread::sleep(Duration::from_millis(25));
	}
}

pub fn cleanup_export_dirs(
	command: &config::Command,
	exec1_destination: &str,
	exec2_destination: &str,
	basename: &str,
	testcase: &str,
	tempdir: &Path,
	working_dir: Option<&str>,
) {
	let tempdir_str = tempdir.to_string_lossy().to_string();
	let args1 = command.args_for_exec(
		exec1_destination,
		basename,
		testcase,
		&tempdir_str,
		working_dir,
	);
	let args2 = command.args_for_exec(
		exec2_destination,
		basename,
		testcase,
		&tempdir_str,
		working_dir,
	);

	for dir in [
		find_output_dir_in_args(&args1),
		find_output_dir_in_args(&args2),
	]
	.into_iter()
	.flatten()
	{
		let path = Path::new(&dir);
		if path.starts_with(tempdir) && path.exists() {
			let _ = std::fs::remove_dir_all(path);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::find_output_dir_in_args;

	#[test]
	fn test_find_output_dir_basic() {
		let args = vec![
			"export".to_string(),
			"/tmp/output".to_string(),
			"path/to/env".to_string(),
		];
		assert_eq!(
			find_output_dir_in_args(&args),
			Some("/tmp/output".to_string())
		);
	}

	#[test]
	fn test_find_output_dir_flag_after_export() {
		let args = vec![
			"export".to_string(),
			"--recursive".to_string(),
			"/tmp/output".to_string(),
		];
		assert_eq!(
			find_output_dir_in_args(&args),
			Some("/tmp/output".to_string())
		);
	}
}
