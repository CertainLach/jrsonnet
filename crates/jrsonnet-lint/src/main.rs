//! Jsonnet linter binary. Args compatible with jsonnet-lint (go-jsonnet).
//! Exit: 0 = no problems, 1 = error (e.g. missing file), 2 = problems found.

use std::io::{self, Read};

use clap::Parser;
use jrsonnet_lint::{apply_fixes, lint_snippet, Diagnostic, LintConfig, ParseError};

#[derive(Parser)]
#[command(name = "jrsonnet-lint")]
#[command(about = "Lint Jsonnet code")]
#[command(version)]
struct Opts {
	/// Add directory to the library search path (right-most wins)
	#[arg(short = 'J', long = "jpath", value_name = "DIR")]
	jpath: Vec<String>,

	/// Disable specific checks (comma-separated). Valid: unused_locals
	#[arg(long = "disable-checks", value_name = "CHECKS", value_delimiter = ',')]
	disable_checks: Vec<String>,

	/// Automatically fix lint issues where possible (not supported for stdin)
	#[arg(long = "fix")]
	fix: bool,

	/// Input files (use - for stdin)
	#[arg(value_name = "FILE")]
	files: Vec<String>,
}

fn main() {
	let opts = Opts::parse();

	if opts.files.is_empty() {
		eprintln!("ERROR: file not provided");
		eprintln!();
		print_usage();
		std::process::exit(1);
	}

	let config = match LintConfig::default().with_disabled_checks(&opts.disable_checks) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("ERROR: {}", e);
			std::process::exit(1);
		}
	};
	let mut had_error = false;
	let mut had_problems = false;

	for file in &opts.files {
		let (code, filename, is_stdin) = if file == "-" {
			let mut s = String::new();
			if io::stdin().read_to_string(&mut s).is_err() {
				eprintln!("ERROR: failed to read stdin");
				had_error = true;
				continue;
			}
			(s, "<stdin>".to_string(), true)
		} else {
			match std::fs::read_to_string(file) {
				Ok(s) => (s, file.clone(), false),
				Err(e) => {
					eprintln!("ERROR: {}: {}", file, e);
					had_error = true;
					continue;
				}
			}
		};

		let (diagnostics, parse_errors) = lint_snippet(&code, &config);

		for e in parse_errors {
			emit_parse_error(&filename, &code, &e);
			had_problems = true;
		}

		if opts.fix && !is_stdin {
			let fixed = apply_fixes(&code, &diagnostics);
			if fixed != code {
				if let Err(e) = std::fs::write(file, &fixed) {
					eprintln!("ERROR: failed to write {}: {}", file, e);
					had_error = true;
					continue;
				}
			}
			// Report any diagnostics that could not be auto-fixed
			for d in diagnostics.iter().filter(|d| d.fix.is_none()) {
				emit_diagnostic(&filename, &code, d);
				had_problems = true;
			}
		} else {
			if opts.fix && is_stdin {
				eprintln!("WARNING: --fix is not supported for stdin; reporting diagnostics only");
			}
			for d in diagnostics {
				emit_diagnostic(&filename, &code, &d);
				had_problems = true;
			}
		}
	}

	if had_error {
		std::process::exit(1);
	}
	if had_problems {
		eprintln!("Problems found!");
		std::process::exit(2);
	}
	std::process::exit(0);
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

fn print_usage() {
	eprintln!("jrsonnet-lint {{<option>}} {{ <filenames ...> }}");
	eprintln!();
	eprintln!("Available options:");
	eprintln!("  -h / --help                This message");
	eprintln!(
		"  -J / --jpath <dir>         Specify an additional library search dir (right-most wins)"
	);
	eprintln!(
		"  --disable-checks <CHECKS>   Disable checks (comma-separated). Valid: unused_locals"
	);
	eprintln!("  --fix                      Automatically fix lint issues where possible");
	eprintln!("  --version                  Print version");
	eprintln!();
	eprintln!("Environment variables:");
	eprintln!("  JSONNET_PATH  Colon (semicolon on Windows) separated list of directories");
	eprintln!("                added in reverse order before the paths specified by --jpath.");
	eprintln!();
	eprintln!("  <filename> can be - (stdin). Use -- to separate options from filenames.");
	eprintln!();
	eprintln!("Exit code:");
	eprintln!("  0  No problems found.");
	eprintln!("  1  Error (e.g. file missing).");
	eprintln!("  2  Problems found.");
}
