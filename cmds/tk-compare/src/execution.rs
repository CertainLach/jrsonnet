use std::{
	collections::HashMap,
	io::Read,
	process::{Command as ProcessCommand, Stdio},
	thread,
	time::{Duration, Instant},
};

use anyhow::{Context, Result};

#[derive(Debug)]
pub struct RunResult {
	pub exit_code: i32,
	pub stdout: String,
	pub stderr: String,
	pub duration: Duration,
}

/// A single command execution with its result and metadata
#[derive(Debug)]
pub struct CommandExecution<'a> {
	pub result: &'a RunResult,
}

pub fn run_command_with_env(
	executable: &str,
	args: &[String],
	workspace_dir: Option<&str>,
	working_dir: Option<&str>,
	env_vars: Option<&HashMap<String, String>>,
	timeout: Duration,
) -> Result<RunResult> {
	let start = Instant::now();

	let mut cmd = ProcessCommand::new(executable);
	cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

	// Add environment variables if provided
	if let Some(vars) = env_vars {
		for (key, value) in vars {
			cmd.env(key, value);
		}
	}

	// Determine the actual working directory
	let actual_working_dir = match (workspace_dir, working_dir) {
		(Some(ws), Some(wd)) => {
			// If both are specified, combine them: workspace_dir/working_dir
			let combined = format!("{}/{}", ws, wd);
			std::fs::create_dir_all(&combined)?;
			Some(combined)
		}
		(Some(ws), None) => {
			// Only workspace directory
			std::fs::create_dir_all(ws)?;
			Some(ws.to_string())
		}
		(None, Some(wd)) => {
			// Only working directory (no workspace isolation)
			Some(wd.to_string())
		}
		(None, None) => None,
	};

	if let Some(dir) = &actual_working_dir {
		cmd.current_dir(dir);
	}

	let child = cmd
		.spawn()
		.with_context(|| format!("Failed to execute command: {} {:?}", executable, args))?;

	let output = wait_with_timeout_and_capture(child, timeout).with_context(|| {
		format!(
			"command timed out after {:?}: {} {:?}",
			timeout, executable, args
		)
	})?;

	let duration = start.elapsed();

	let exit_code = output.status.code().unwrap_or(-1);
	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();

	Ok(RunResult {
		exit_code,
		stdout,
		stderr,
		duration,
	})
}

fn wait_with_timeout_and_capture(
	mut child: std::process::Child,
	timeout: Duration,
) -> Result<std::process::Output> {
	let mut stdout = child
		.stdout
		.take()
		.context("failed to capture child stdout")?;
	let mut stderr = child
		.stderr
		.take()
		.context("failed to capture child stderr")?;

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

	let start = Instant::now();
	let status = loop {
		if let Some(status) = child.try_wait()? {
			break status;
		}

		if start.elapsed() > timeout {
			let _ = child.kill();
			let _ = child.wait();
			let _ = stdout_reader.join();
			let _ = stderr_reader.join();
			anyhow::bail!("process exceeded timeout");
		}

		std::thread::sleep(Duration::from_millis(25));
	};

	let stdout = stdout_reader
		.join()
		.map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))?;
	let stderr = stderr_reader
		.join()
		.map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))?;

	Ok(std::process::Output {
		status,
		stdout,
		stderr,
	})
}
