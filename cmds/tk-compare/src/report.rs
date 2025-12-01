use colored::Colorize;
use std::time::Duration;

#[derive(Debug)]
pub struct RuntimeStats {
	pub min: Duration,
	pub max: Duration,
	pub median: Duration,
	pub average: Duration,
}

impl RuntimeStats {
	pub fn from_durations(mut durations: Vec<Duration>) -> Self {
		durations.sort();
		let min = *durations.first().unwrap_or(&Duration::ZERO);
		let max = *durations.last().unwrap_or(&Duration::ZERO);

		let median = if durations.is_empty() {
			Duration::ZERO
		} else if durations.len() % 2 == 0 {
			let mid = durations.len() / 2;
			(durations[mid - 1] + durations[mid]) / 2
		} else {
			durations[durations.len() / 2]
		};

		let total: Duration = durations.iter().sum();
		let average = if durations.is_empty() {
			Duration::ZERO
		} else {
			total / durations.len() as u32
		};

		Self {
			min,
			max,
			median,
			average,
		}
	}
}

#[derive(Debug)]
pub struct CommandReport {
	pub command: String,
	pub runs: usize,
	pub exit_code_matched: bool,
	pub stdout_matched: bool,
	pub result_dir_matched: Option<bool>,
	pub exec1_stats: RuntimeStats,
	pub exec2_stats: RuntimeStats,
	pub exec1_exit_code: i32,
	pub exec2_exit_code: i32,
	pub exec1_stderr: String,
	pub exec2_stderr: String,
}

impl CommandReport {
	pub fn print(&self, index: usize) {
		println!("\n{}", format!("=== Command {} ===", index + 1).bold());
		println!("Command: {}", self.command.cyan());

		// Exit code
		let exit_code_status = if self.exit_code_matched {
			"✓ MATCHED".green()
		} else {
			"✗ MISMATCH".red()
		};
		println!(
			"Exit Code: {} (exec1: {}, exec2: {})",
			exit_code_status, self.exec1_exit_code, self.exec2_exit_code
		);

		// Stdout
		let stdout_status = if self.stdout_matched {
			"✓ MATCHED".green()
		} else {
			"✗ MISMATCH".red()
		};
		println!("Stdout: {}", stdout_status);

		// Result dir
		if let Some(result_dir_matched) = self.result_dir_matched {
			let result_dir_status = if result_dir_matched {
				"✓ MATCHED".green()
			} else {
				"✗ MISMATCH".red()
			};
			println!("Result Dir: {}", result_dir_status);
		} else {
			println!("Result Dir: {}", "N/A".yellow());
		}

		// Runtime comparison
		if self.runs > 1 {
			println!("Runtime (across {} runs):", self.runs);
			println!("  exec1:");
			println!("    min:     {}ms", self.exec1_stats.min.as_millis());
			println!("    max:     {}ms", self.exec1_stats.max.as_millis());
			println!("    median:  {}ms", self.exec1_stats.median.as_millis());
			println!("    average: {}ms", self.exec1_stats.average.as_millis());
			println!("  exec2:");
			println!("    min:     {}ms", self.exec2_stats.min.as_millis());
			println!("    max:     {}ms", self.exec2_stats.max.as_millis());
			println!("    median:  {}ms", self.exec2_stats.median.as_millis());
			println!("    average: {}ms", self.exec2_stats.average.as_millis());

			// Compare based on median
			let exec1_ms = self.exec1_stats.median.as_millis();
			let exec2_ms = self.exec2_stats.median.as_millis();
			let ratio = if exec2_ms > 0 {
				exec1_ms as f64 / exec2_ms as f64
			} else {
				0.0
			};

			print!("  Comparison (median): ");
			if ratio > 1.0 {
				println!("exec1 is {:.2}x slower", ratio);
			} else if ratio < 1.0 && ratio > 0.0 {
				println!("exec1 is {:.2}x faster", 1.0 / ratio);
			} else {
				println!("same");
			}
		} else {
			println!("Runtime:");
			println!("  exec1: {}ms", self.exec1_stats.average.as_millis());
			println!("  exec2: {}ms", self.exec2_stats.average.as_millis());

			let exec1_ms = self.exec1_stats.average.as_millis();
			let exec2_ms = self.exec2_stats.average.as_millis();
			let ratio = if exec2_ms > 0 {
				exec1_ms as f64 / exec2_ms as f64
			} else {
				0.0
			};

			if ratio > 1.0 {
				println!("  exec1 is {:.2}x slower", ratio);
			} else if ratio < 1.0 && ratio > 0.0 {
				println!("  exec1 is {:.2}x faster", 1.0 / ratio);
			}
		}

		// Stderr output
		if !self.exec1_stderr.is_empty() {
			println!("\n{}", "Exec1 stderr:".yellow());
			println!("{}", self.exec1_stderr);
		}

		if !self.exec2_stderr.is_empty() {
			println!("\n{}", "Exec2 stderr:".yellow());
			println!("{}", self.exec2_stderr);
		}
	}
}

pub fn print_summary(reports: &[CommandReport]) {
	println!("\n{}", "=== SUMMARY ===".bold());

	let total = reports.len();
	let exit_code_matches = reports.iter().filter(|r| r.exit_code_matched).count();
	let stdout_matches = reports.iter().filter(|r| r.stdout_matched).count();
	let result_dir_total = reports
		.iter()
		.filter(|r| r.result_dir_matched.is_some())
		.count();
	let result_dir_matches = reports
		.iter()
		.filter(|r| r.result_dir_matched == Some(true))
		.count();

	println!("Total commands: {}", total);
	println!("Exit code matches: {}/{}", exit_code_matches, total);
	println!("Stdout matches: {}/{}", stdout_matches, total);
	if result_dir_total > 0 {
		println!(
			"Result dir matches: {}/{}",
			result_dir_matches, result_dir_total
		);
	}

	let all_passed = exit_code_matches == total
		&& stdout_matches == total
		&& (result_dir_total == 0 || result_dir_matches == result_dir_total);

	if all_passed {
		println!("\n{}", "✓ All tests passed!".green().bold());
	} else {
		println!("\n{}", "✗ Some tests failed!".red().bold());
	}
}
