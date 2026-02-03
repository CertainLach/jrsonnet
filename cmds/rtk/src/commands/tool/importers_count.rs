//! Importers-count subcommand handler.
//!
//! For each file in a directory, counts how many environments import it.

use std::{fs, io::Write, path::Path};

use anyhow::{Context, Result};
use clap::Args;
use regex::Regex;
use walkdir::WalkDir;

use crate::importers as importers_impl;
use crate::jpath::DEFAULT_ENTRYPOINT;

#[derive(Args)]
pub struct ImportersCountArgs {
	/// Directory to scan for files
	pub dir: String,

	/// Only count files that match the given regex. Matches only jsonnet files by default
	#[arg(long)]
	pub filename_regex: Option<String>,

	/// Find files recursively
	#[arg(long)]
	pub recursive: bool,

	/// Root directory to search for environments
	#[arg(long, default_value = ".")]
	pub root: String,
}

/// Result of counting importers for a file
struct ImporterCount {
	file: String,
	count: usize,
}

/// Run the importers-count subcommand.
///
/// Lists all files in the given directory and counts how many environments import each one.
pub fn run<W: Write>(args: ImportersCountArgs, mut writer: W) -> Result<()> {
	let root = fs::canonicalize(&args.root).context("resolving root")?;
	let root_str = root.to_string_lossy().to_string();

	// Compile filename regex (default matches jsonnet/libsonnet files)
	let filename_regex_str = args
		.filename_regex
		.as_deref()
		.unwrap_or(r"^.*\.(jsonnet|libsonnet)$");
	let filename_regex = Regex::new(filename_regex_str).context("compiling filename regex")?;

	// Collect files from the directory
	let dir = Path::new(&args.dir);
	let files = collect_files(dir, args.recursive, &filename_regex)?;

	// Count importers for all files at once (builds context only once)
	let importers_map = importers_impl::find_importers_batch(&root_str, files)?;

	// Convert to counts
	let mut counts: Vec<ImporterCount> = importers_map
		.into_iter()
		.map(|(file, importers)| ImporterCount {
			file,
			count: importers.len(),
		})
		.collect();

	// Sort by count (descending), then by file name
	counts.sort_by(|a, b| {
		if a.count == b.count {
			a.file.cmp(&b.file)
		} else {
			b.count.cmp(&a.count)
		}
	});

	// Output results
	for count in counts {
		writeln!(writer, "{}: {}", count.file, count.count)?;
	}

	// Add trailing newline to match tk output format
	writeln!(writer)?;

	Ok(())
}

/// Collect files from a directory, optionally recursively
fn collect_files(dir: &Path, recursive: bool, filename_regex: &Regex) -> Result<Vec<String>> {
	let mut files = Vec::new();

	let walker = if recursive {
		WalkDir::new(dir)
	} else {
		WalkDir::new(dir).max_depth(1)
	};

	for entry in walker {
		let entry = entry.context("walking directory")?;
		let path = entry.path();

		// Skip directories
		if !path.is_file() {
			continue;
		}

		// Skip main.jsonnet files (they are environments, not libraries)
		if path.file_name().and_then(|n| n.to_str()) == Some(DEFAULT_ENTRYPOINT) {
			continue;
		}

		// Check if filename matches the regex
		let path_str = path.to_string_lossy();
		if !filename_regex.is_match(&path_str) {
			continue;
		}

		files.push(path_str.to_string());
	}

	Ok(files)
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use super::*;

	fn test_root() -> PathBuf {
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/findImporters")
	}

	#[test]
	fn test_project_with_no_imports() {
		let args = ImportersCountArgs {
			dir: test_root()
				.join("environments/no-imports")
				.to_string_lossy()
				.to_string(),
			filename_regex: None,
			recursive: true,
			root: test_root().to_string_lossy().to_string(),
		};
		let mut output = Vec::new();

		run(args, &mut output).expect("importers-count should succeed");

		// No files other than main.jsonnet, so output is just a trailing newline (matching tk)
		let output_str = String::from_utf8(output).unwrap();
		assert_eq!(output_str, "\n");
	}

	#[test]
	fn test_project_with_imports() {
		let args = ImportersCountArgs {
			dir: test_root()
				.join("environments/imports-locals-and-vendored")
				.to_string_lossy()
				.to_string(),
			filename_regex: None,
			recursive: true,
			root: test_root().to_string_lossy().to_string(),
		};
		let mut output = Vec::new();

		run(args, &mut output).expect("importers-count should succeed");

		let output_str = String::from_utf8(output).unwrap();
		// Both local files are imported by one environment
		assert!(output_str.contains("local-file1.libsonnet: 1"));
		assert!(output_str.contains("local-file2.libsonnet: 1"));
	}

	#[test]
	fn test_lib_non_recursive() {
		let args = ImportersCountArgs {
			dir: test_root().join("lib/lib1").to_string_lossy().to_string(),
			filename_regex: None,
			recursive: false,
			root: test_root().to_string_lossy().to_string(),
		};
		let mut output = Vec::new();

		run(args, &mut output).expect("importers-count should succeed");

		let output_str = String::from_utf8(output).unwrap();
		// Only main.libsonnet (not subfolder) should be counted
		assert!(output_str.contains("main.libsonnet: 1"));
		assert!(!output_str.contains("subfolder"));
	}

	#[test]
	fn test_lib_recursive() {
		let args = ImportersCountArgs {
			dir: test_root().join("lib/lib1").to_string_lossy().to_string(),
			filename_regex: None,
			recursive: true,
			root: test_root().to_string_lossy().to_string(),
		};
		let mut output = Vec::new();

		run(args, &mut output).expect("importers-count should succeed");

		let output_str = String::from_utf8(output).unwrap();
		// Both main.libsonnet and subfolder/test.libsonnet should be counted
		assert!(output_str.contains("main.libsonnet: 1"));
		assert!(output_str.contains("test.libsonnet: 0"));
	}
}
