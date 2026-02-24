//! Lint command handler.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use clap::Args;
use jrsonnet_lint::{lint_snippet, Diagnostic, LintConfig, ParseError};
use walkdir::WalkDir;

/// Exit code when lint problems were found (for process::exit)
pub const EXIT_CODE_PROBLEMS: i32 = 2;

#[derive(Args)]
pub struct LintArgs {
	/// Files or directories to lint
	pub paths: Vec<String>,

	/// Globs to exclude
	#[arg(short = 'e', long, default_values_t = vec!["**/.*".to_string(), ".*".to_string(), "**/vendor/**".to_string(), "vendor/**".to_string()])]
	pub exclude: Vec<String>,

	/// Amount of workers
	#[arg(short = 'n', long, default_value = "4")]
	pub parallelism: i32,

	/// Disable specific checks (comma-separated). Valid: unused_locals
	#[arg(long = "disable-checks", value_name = "CHECKS", value_delimiter = ',')]
	pub disable_checks: Vec<String>,
}

/// Run the lint command.
pub fn run<W: Write>(args: LintArgs, _writer: W) -> Result<()> {
	let config = LintConfig::default()
		.with_disabled_checks(&args.disable_checks)
		.map_err(anyhow::Error::msg)?;

	let paths = if args.paths.is_empty() {
		vec![".".to_string()]
	} else {
		args.paths.clone()
	};

	let mut all_files: Vec<String> = Vec::new();
	for path in &paths {
		let p = Path::new(path);
		if p.is_file() {
			if is_jsonnet_file(p) {
				all_files.push(p.display().to_string());
			}
		} else if p.is_dir() {
			for entry in WalkDir::new(p)
				.follow_links(true)
				.into_iter()
				.filter_entry(|e| !is_excluded(e.path(), &args.exclude))
			{
				let entry = entry?;
				let path = entry.path();
				if path.is_file() && is_jsonnet_file(path) {
					all_files.push(path.display().to_string());
				}
			}
		} else {
			anyhow::bail!("no such file or directory: {}", path);
		}
	}

	let mut had_problems = false;
	for file in &all_files {
		let code = fs::read_to_string(file)?;
		let (diagnostics, parse_errors) = lint_snippet(&code, &config);
		for e in parse_errors {
			emit_parse_error(file, &code, &e);
			had_problems = true;
		}
		for d in diagnostics {
			emit_diagnostic(file, &code, &d);
			had_problems = true;
		}
	}

	if had_problems {
		eprintln!("Problems found!");
		std::process::exit(EXIT_CODE_PROBLEMS);
	}
	Ok(())
}

fn is_jsonnet_file(p: &Path) -> bool {
	p.extension()
		.map(|e| e == "jsonnet" || e == "libsonnet")
		.unwrap_or(false)
}

/// Simple exclude: path matches if any exclude pattern matches.
/// Patterns: "**/vendor/**", "vendor/**", "**/.*", ".*" (path contains or path component).
fn is_excluded(path: &Path, exclude: &[String]) -> bool {
	let path_str = path.to_string_lossy();
	for pat in exclude {
		if pat == ".*" {
			if path
				.file_name()
				.map_or(false, |n| n.to_string_lossy().starts_with('.'))
			{
				return true;
			}
		} else if pat == "**/.*" {
			if path
				.components()
				.any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
			{
				return true;
			}
		} else if pat.ends_with("/**") {
			let prefix = pat.trim_end_matches("/**");
			if path_str.contains(prefix) {
				return true;
			}
		} else if pat == "**/vendor/**" || pat == "vendor/**" {
			if path_str.contains("/vendor/") || path_str.contains("vendor/") {
				return true;
			}
		}
	}
	false
}

fn emit_parse_error(filename: &str, code: &str, e: &ParseError) {
	let start: usize = e.range.start().into();
	let (line, col) = offset_to_line_col(code, start);
	eprintln!("{}:{}:{}: parse error: {}", filename, line, col, e.message);
}

fn emit_diagnostic(filename: &str, code: &str, d: &Diagnostic) {
	let start: usize = d.range.start().into();
	let (line, col) = offset_to_line_col(code, start);
	eprintln!("{}:{}:{}: [{}] {}", filename, line, col, d.check, d.message);
}

fn offset_to_line_col(code: &str, offset: usize) -> (u32, u32) {
	let mut line = 1u32;
	let mut col = 1u32;
	for (i, c) in code.char_indices() {
		if i >= offset {
			break;
		}
		if c == '\n' {
			line += 1;
			col = 1;
		} else {
			col += 1;
		}
	}
	(line, col)
}

#[cfg(test)]
mod tests {
	use std::io::sink;

	use super::*;

	#[test]
	fn run_rejects_invalid_disable_checks() {
		let args = LintArgs {
			paths: vec![".".to_string()],
			exclude: vec![],
			parallelism: 4,
			disable_checks: vec!["no_such_check".to_string()],
		};
		let result = run(args, sink());
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("unknown check"));
		assert!(err.contains("no_such_check"));
	}

	#[test]
	fn run_accepts_valid_disable_checks() {
		let args = LintArgs {
			paths: vec![".".to_string()],
			exclude: vec![],
			parallelism: 4,
			disable_checks: vec!["unused_locals".to_string()],
		};
		// Run from cwd; may have no jsonnet files, so Ok(())
		let result = run(args, sink());
		assert!(result.is_ok());
	}

	#[test]
	fn run_on_nonexistent_path_errors() {
		let args = LintArgs {
			paths: vec!["/nonexistent/path/for/rtk/lint/test".to_string()],
			exclude: vec![],
			parallelism: 4,
			disable_checks: vec![],
		};
		let result = run(args, sink());
		assert!(result.is_err());
	}

	#[test]
	fn run_on_dir_with_clean_jsonnet_returns_ok() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("main.jsonnet");
		std::fs::write(&path, "local x = 1; x\n").unwrap();
		let args = LintArgs {
			paths: vec![path.to_string_lossy().to_string()],
			exclude: vec![],
			parallelism: 4,
			disable_checks: vec![],
		};
		let result = run(args, sink());
		assert!(result.is_ok());
	}

	#[test]
	fn is_jsonnet_file_extensions() {
		assert!(is_jsonnet_file(std::path::Path::new("a.jsonnet")));
		assert!(is_jsonnet_file(std::path::Path::new("b.libsonnet")));
		assert!(!is_jsonnet_file(std::path::Path::new("c.json")));
		assert!(!is_jsonnet_file(std::path::Path::new("d.txt")));
	}
}
