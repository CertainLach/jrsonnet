//! Imports subcommand handler.
//!
//! Lists all transitive imports of an environment.

use std::io::Write;

use anyhow::Result;
use clap::Args;
use serde_json;

use crate::imports as imports_impl;

#[derive(Args)]
pub struct ImportsArgs {
	/// Path to the environment (directory or main.jsonnet file)
	pub path: String,

	/// Git commit hash to check against (not implemented)
	#[arg(short = 'c', long)]
	pub check: Option<String>,
}

/// Run the imports subcommand.
///
/// Lists all files that are transitively imported by the environment's main.jsonnet.
/// Output is a JSON array of file paths (matching tk's output format).
pub fn run<W: Write>(args: ImportsArgs, mut writer: W) -> Result<()> {
	if args.check.is_some() {
		anyhow::bail!("--check flag is not implemented");
	}

	let imports = imports_impl::transitive_imports(&args.path)?;

	// Output as JSON array (matching tk's output format)
	let json = serde_json::to_string(&imports)?;
	writeln!(writer, "{}", json)?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use super::*;

	fn test_root() -> PathBuf {
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/importTree")
	}

	#[test]
	fn test_imports_command_output() {
		let args = ImportsArgs {
			path: test_root().to_string_lossy().to_string(),
			check: None,
		};
		let mut output = Vec::new();

		run(args, &mut output).expect("imports should succeed");

		let output_str = String::from_utf8(output).unwrap();
		// Output is JSON array format (matching tk)
		let imports: Vec<String> = serde_json::from_str(output_str.trim()).unwrap();

		assert_eq!(
			imports,
			vec![
				"main.jsonnet",
				"trees.jsonnet",
				"trees/apple.jsonnet",
				"trees/cherry.jsonnet",
				"trees/generic.libsonnet",
				"trees/peach.jsonnet",
			]
		);
	}

	#[test]
	fn test_imports_check_flag_not_implemented() {
		let args = ImportsArgs {
			path: test_root().to_string_lossy().to_string(),
			check: Some("abc123".to_string()),
		};
		let mut output = Vec::new();

		let result = run(args, &mut output);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("not implemented"));
	}
}
