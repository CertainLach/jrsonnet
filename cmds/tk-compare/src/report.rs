use std::time::Duration;

use k8s_mock::HttpExchange;
use rtk_diff::SimilarityScore;
use serde::Serialize;

use crate::types::Pair;

/// Threshold for considering outputs as "near matches" (percentage).
const NEAR_MATCH_THRESHOLD: f64 = 99.5;

/// Format a duration in a human-readable way
pub fn format_duration(duration: Duration) -> String {
	let millis = duration.as_millis();

	if millis < 1000 {
		format!("{}ms", millis)
	} else if millis < 60_000 {
		let secs = millis as f64 / 1000.0;
		format!("{:.2}s", secs)
	} else if millis < 3_600_000 {
		let mins = millis / 60_000;
		let secs = (millis % 60_000) as f64 / 1000.0;
		format!("{}m {:.1}s", mins, secs)
	} else {
		let hours = millis / 3_600_000;
		let mins = (millis % 3_600_000) / 60_000;
		let secs = (millis % 60_000) as f64 / 1000.0;
		format!("{}h {}m {:.1}s", hours, mins, secs)
	}
}

/// Per-executor result information.
#[derive(Debug)]
pub struct ExecResult {
	pub name: String,
	pub duration: Duration,
	pub exit_code: i32,
	pub stdout: String,
	pub stderr: String,
}

#[derive(Debug)]
pub struct CommandReport {
	pub command: String,
	pub exit_code_matched: bool,
	pub exit_codes_consistent: bool,
	pub stdout_matched: bool,
	pub stdout_similarity: Option<SimilarityScore>,
	pub both_failed_unexpectedly: bool,
	pub execs: Pair<ExecResult>,
}

impl CommandReport {
	pub fn passed(&self) -> bool {
		self.exit_code_matched
			&& self.exit_codes_consistent
			&& self.stdout_matched
			&& !self.both_failed_unexpectedly
	}

	pub fn summary(&self) -> String {
		if self.both_failed_unexpectedly {
			return format!(
				"expected success, both failed ({}: {}, {}: {})",
				self.execs.first.name,
				self.execs.first.exit_code,
				self.execs.second.name,
				self.execs.second.exit_code
			);
		}
		if !self.exit_codes_consistent {
			return "exit codes inconsistent across runs".to_string();
		}
		if !self.exit_code_matched {
			return format!(
				"exit mismatch ({}: {}, {}: {})",
				self.execs.first.name,
				self.execs.first.exit_code,
				self.execs.second.name,
				self.execs.second.exit_code
			);
		}
		if !self.stdout_matched {
			return self
				.stdout_similarity
				.as_ref()
				.map(|sim| {
					format!(
						"output mismatch ({:.1}% similar: {}/{})",
						sim.percentage, sim.matched, sim.total
					)
				})
				.unwrap_or_else(|| "output mismatch".to_string());
		}
		"matched".to_string()
	}
}

/// Aggregated statistics from a set of command reports.
struct ReportStats {
	total: usize,
	exit_code_matches: usize,
	stdout_matches: usize,
	stdout_near_matches: usize,
	all_passed: bool,
}

impl ReportStats {
	/// Calculate statistics from a slice of reports.
	fn from_reports(reports: &[CommandReport]) -> Self {
		let total = reports.len();

		let exit_code_matches = reports
			.iter()
			.filter(|r| {
				r.exit_code_matched && r.exit_codes_consistent && !r.both_failed_unexpectedly
			})
			.count();

		let stdout_matches = reports.iter().filter(|r| r.stdout_matched).count();

		let stdout_near_matches = reports
			.iter()
			.filter(|r| {
				r.stdout_similarity
					.as_ref()
					.map(|s| s.is_near_match(NEAR_MATCH_THRESHOLD))
					.unwrap_or(false)
			})
			.count();

		let all_passed = exit_code_matches == total
			&& stdout_matches == total
			&& reports.iter().all(|r| r.exit_codes_consistent)
			&& !reports.iter().any(|r| r.both_failed_unexpectedly);

		Self {
			total,
			exit_code_matches,
			stdout_matches,
			stdout_near_matches,
			all_passed,
		}
	}
}

pub struct SummaryStats {
	pub total: usize,
	pub exit_code_matches: usize,
	pub stdout_matches: usize,
	pub stdout_near_matches: usize,
	pub all_passed: bool,
}

pub fn summarize(reports: &[CommandReport]) -> SummaryStats {
	let stats = ReportStats::from_reports(reports);
	SummaryStats {
		total: stats.total,
		exit_code_matches: stats.exit_code_matches,
		stdout_matches: stats.stdout_matches,
		stdout_near_matches: stats.stdout_near_matches,
		all_passed: stats.all_passed,
	}
}

#[derive(Debug, Serialize)]
/// Detailed mismatch payload emitted when stdout comparison fails.
///
/// This includes:
/// - comparer subprocess output (`stdout`/`stderr`)
/// - raw command output from both executors (`tk_*` and `rtk_*`)
pub struct CompareOutput {
	/// Stdout emitted by the comparer subprocess (`tk-compare compare ...`).
	pub stdout: String,
	/// Stderr emitted by the comparer subprocess (`tk-compare compare ...`).
	pub stderr: String,
	/// Raw stdout from the `tk` command invocation for this test.
	pub tk_stdout: String,
	/// Raw stdout from the `rtk` command invocation for this test.
	pub rtk_stdout: String,
	/// Raw stderr from the `tk` command invocation for this test.
	pub tk_stderr: String,
	/// Raw stderr from the `rtk` command invocation for this test.
	pub rtk_stderr: String,
}

#[derive(Debug, Serialize)]
pub struct EventExec {
	pub exit_code: i32,
	pub duration_ms: u128,
	pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct TestEvent {
	pub event: &'static str,
	pub index: usize,
	pub total: usize,
	pub suite: String,
	pub name: String,
	pub command: String,
	pub summary: String,
	pub passed: bool,
	pub elapsed_ms: u128,
	pub failure_flags: Vec<&'static str>,
	pub compare: Option<CompareOutput>,
	pub mock_http: Vec<HttpExchange>,
	pub tk: EventExec,
	pub rtk: EventExec,
	pub delta_ms: i128,
}

#[derive(Debug, Serialize)]
pub struct SummaryEvent {
	pub event: &'static str,
	pub total: usize,
	pub exit_code_matches: usize,
	pub stdout_matches: usize,
	pub stdout_near_matches: usize,
	pub all_passed: bool,
	pub elapsed_ms: u128,
}

impl CommandReport {
	pub fn failure_flags(&self) -> Vec<&'static str> {
		let mut flags = Vec::new();
		if !self.exit_code_matched {
			flags.push("exit");
		}
		if !self.stdout_matched {
			flags.push("output");
		}
		if self.both_failed_unexpectedly {
			flags.push("unexpected");
		}
		flags
	}
}

pub fn build_test_event(
	run_index: usize,
	total_runs: usize,
	suite: &str,
	case_name: &str,
	elapsed_ms: u128,
	report: &CommandReport,
	compare_stdout: &str,
	compare_stderr: &str,
	mock_http: Vec<HttpExchange>,
) -> TestEvent {
	let compare = (!report.stdout_matched).then(|| CompareOutput {
		stdout: compare_stdout.to_string(),
		stderr: compare_stderr.to_string(),
		tk_stdout: report.execs.first.stdout.clone(),
		rtk_stdout: report.execs.second.stdout.clone(),
		tk_stderr: report.execs.first.stderr.clone(),
		rtk_stderr: report.execs.second.stderr.clone(),
	});

	TestEvent {
		event: "test",
		index: run_index,
		total: total_runs,
		suite: suite.to_string(),
		name: case_name.to_string(),
		command: report.command.clone(),
		summary: report.summary(),
		passed: report.passed(),
		elapsed_ms,
		failure_flags: report.failure_flags(),
		compare,
		mock_http,
		tk: EventExec {
			exit_code: report.execs.first.exit_code,
			duration_ms: report.execs.first.duration.as_millis(),
			stderr: report.execs.first.stderr.clone(),
		},
		rtk: EventExec {
			exit_code: report.execs.second.exit_code,
			duration_ms: report.execs.second.duration.as_millis(),
			stderr: report.execs.second.stderr.clone(),
		},
		delta_ms: report.execs.second.duration.as_millis() as i128
			- report.execs.first.duration.as_millis() as i128,
	}
}

pub fn build_summary_event(stats: &SummaryStats, elapsed_ms: u128) -> SummaryEvent {
	SummaryEvent {
		event: "summary",
		total: stats.total,
		exit_code_matches: stats.exit_code_matches,
		stdout_matches: stats.stdout_matches,
		stdout_near_matches: stats.stdout_near_matches,
		all_passed: stats.all_passed,
		elapsed_ms,
	}
}
