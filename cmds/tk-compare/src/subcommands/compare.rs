use std::{
	io::{IsTerminal, Write},
	time::Duration,
};

use anyhow::{Context, Result};
use rtk_diff::{
	directory::compare_directories_detailed,
	unified::{render_unified_diff, ParsedUnifiedDiff},
	SimilarityScore,
};

use crate::{
	cli::{CompareCli, CompareKind},
	comparison::{
		auto::AutoComparer,
		json::JsonComparer,
		string::StringComparer,
		traits::{Comparer, ComparisonResult},
	},
	execution,
};

#[derive(Debug, Clone, Copy)]
struct SimilarityMetric {
	unit_plural: &'static str,
	description: &'static str,
}

pub fn execute(cli: CompareCli) -> i32 {
	match run_inner(&cli) {
		Ok(true) => 0,
		Ok(false) => 1,
		Err(err) => {
			eprintln!("{}", err);
			2
		}
	}
}

fn run_inner(cli: &CompareCli) -> Result<bool> {
	match cli.kind {
		CompareKind::Directory => {
			let cmp = compare_directories_detailed(&cli.left, &cli.right)?;
			if !cmp.matched {
				println!("mismatch: {} differing file paths", cmp.differences.len());
				for diff in cmp.differences {
					println!("{}", diff);
				}
			}
			Ok(cmp.matched)
		}
		_ => {
			let left = std::fs::read_to_string(&cli.left)
				.with_context(|| format!("Failed to read {}", cli.left))?;
			let right = std::fs::read_to_string(&cli.right)
				.with_context(|| format!("Failed to read {}", cli.right))?;

			let (result, metric) = match cli.kind {
				CompareKind::Json => (
					compare_text_with(JsonComparer, &left, &right)?,
					SimilarityMetric {
						unit_plural: "JSON paths",
						description: "counted over object keys and array indices",
					},
				),
				CompareKind::String => (
					compare_text_with(StringComparer, &left, &right)?,
					SimilarityMetric {
						unit_plural: "lines",
						description: "line-by-line positional comparison",
					},
				),
				CompareKind::Auto => {
					let is_unified = ParsedUnifiedDiff::parse(&left).is_some()
						&& ParsedUnifiedDiff::parse(&right).is_some();
					let metric = if is_unified {
						SimilarityMetric {
							unit_plural: "diff changes",
							description:
								"added/removed unified-diff lines, normalized by resource path",
						}
					} else {
						SimilarityMetric {
							unit_plural: "lines",
							description: "auto fallback to line-by-line positional comparison",
						}
					};
					(compare_text_with(AutoComparer, &left, &right)?, metric)
				}
				CompareKind::UnifiedDiff => {
					if left.trim().is_empty() && right.trim().is_empty() {
						(
							ComparisonResult::new(true, SimilarityScore::perfect()),
							SimilarityMetric {
								unit_plural: "diff changes",
								description:
									"added/removed unified-diff lines, normalized by resource path",
							},
						)
					} else {
						match (
							ParsedUnifiedDiff::parse(&left),
							ParsedUnifiedDiff::parse(&right),
						) {
							(Some(left_parsed), Some(right_parsed)) => (
								ComparisonResult::new(
									left_parsed == right_parsed,
									left_parsed.similarity_score(&right_parsed),
								),
								SimilarityMetric {
									unit_plural: "diff changes",
									description:
										"added/removed unified-diff lines, normalized by resource path",
								},
							),
							_ => (
								compare_text_with(StringComparer, &left, &right)?,
								SimilarityMetric {
									unit_plural: "lines",
									description:
										"unified-diff parse failed; fell back to line-by-line comparison",
								},
							),
						}
					}
				}
				CompareKind::Directory => unreachable!(),
			};

			if !result.matched {
				let differing = result
					.similarity
					.total
					.saturating_sub(result.similarity.matched);
				println!(
					"mismatch: matched {}/{} {} ({:.1}%, {} differing)",
					result.similarity.matched,
					result.similarity.total,
					metric.unit_plural,
					result.similarity.percentage,
					differing
				);
				println!("metric: {}", metric.description);
				print_mismatch_diff(cli.kind, &left, &right);
			}
			Ok(result.matched)
		}
	}
}

fn print_mismatch_diff(kind: CompareKind, left: &str, right: &str) {
	let (left_text, right_text) = match kind {
		CompareKind::Json => {
			let pretty_left = serde_json::from_str::<serde_json::Value>(left)
				.ok()
				.and_then(|v| serde_json::to_string_pretty(&v).ok());
			let pretty_right = serde_json::from_str::<serde_json::Value>(right)
				.ok()
				.and_then(|v| serde_json::to_string_pretty(&v).ok());
			match (pretty_left, pretty_right) {
				(Some(l), Some(r)) => (l, r),
				_ => (left.to_string(), right.to_string()),
			}
		}
		_ => (left.to_string(), right.to_string()),
	};

	let diff = render_unified_diff(&left_text, &right_text, "first", "second");
	if diff.trim().is_empty() {
		return;
	}
	print_colored_diff(diff.trim_end());
}

fn print_colored_diff(diff: &str) {
	if !std::io::stdout().is_terminal() {
		println!("{diff}");
		return;
	}

	let mut stdout = std::io::stdout().lock();
	for line in diff.lines() {
		let styled = crate::output::stylize_compare_line(line, true);
		let _ = writeln!(stdout, "{styled}");
	}
}

fn compare_text_with<C: Comparer>(
	comparer: C,
	left: &str,
	right: &str,
) -> Result<ComparisonResult> {
	let result1 = execution::RunResult {
		exit_code: 0,
		stdout: left.to_string(),
		stderr: String::new(),
		duration: Duration::default(),
	};
	let result2 = execution::RunResult {
		exit_code: 0,
		stdout: right.to_string(),
		stderr: String::new(),
		duration: Duration::default(),
	};
	let exec1 = execution::CommandExecution { result: &result1 };
	let exec2 = execution::CommandExecution { result: &result2 };
	comparer.compare(&exec1, &exec2)
}
