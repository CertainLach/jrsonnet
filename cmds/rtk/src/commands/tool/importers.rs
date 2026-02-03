//! Importers subcommand handler.

use std::io::Write;

use anyhow::Result;
use clap::Args;

use crate::importers as importers_impl;

#[derive(Args)]
pub struct ImportersArgs {
	/// Files to check
	pub files: Vec<String>,

	/// Root directory to search for environments
	#[arg(long, default_value = ".")]
	pub root: String,
}

/// Run the importers subcommand.
pub fn run<W: Write>(args: ImportersArgs, mut writer: W) -> Result<()> {
	let envs = importers_impl::find_importers(&args.root, args.files)?;
	if envs.is_empty() {
		writeln!(writer)?;
	} else {
		for env in envs {
			writeln!(writer, "{}", env)?;
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use assert_matches::assert_matches;

	use super::*;
	use crate::{commands::util::BrokenPipeGuard, test_utils::BrokenPipeWriter};

	fn make_args() -> ImportersArgs {
		ImportersArgs {
			files: vec![],
			root: ".".to_string(),
		}
	}

	#[test]
	fn test_importers_with_no_files_produces_empty_line() {
		let args = make_args();
		let mut output = Vec::new();

		run(args, &mut output).expect("importers should succeed with no files");

		assert_eq!(output, b"\n");
	}

	#[test]
	fn test_importers_exits_cleanly_on_broken_pipe() {
		let args = make_args();
		// Wrap BrokenPipeWriter with BrokenPipeGuard to test the guard handles broken pipes
		let writer = BrokenPipeGuard::new(BrokenPipeWriter);
		let result = run(args, writer);

		// With no files, there's nothing to write, so broken pipe won't be triggered.
		// This test verifies we don't panic in the edge case.
		assert_matches!(result, Ok(()));
	}
}
