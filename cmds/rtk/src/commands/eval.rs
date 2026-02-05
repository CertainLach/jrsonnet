//! Eval command handler.

use std::{io::Write, path::Path};

use anyhow::Result;
use clap::Args;
use jrsonnet_evaluator::ImportResolver;

use super::util::{JsonnetArgs, UnimplementedArgs};
use crate::{
	eval::{self, EvalOpts},
	spec::Environment,
};

#[derive(Args)]
pub struct EvalArgs {
	/// Path to evaluate
	pub path: String,

	/// Evaluate expression on output of jsonnet
	#[arg(short = 'e', long)]
	pub eval: Option<String>,

	/// Set code value of extVar (Format: key=<code>)
	#[arg(long)]
	pub ext_code: Vec<String>,

	/// Set string value of extVar (Format: key=value)
	#[arg(short = 'V', long)]
	pub ext_str: Vec<String>,

	/// Use `go` to use native go-jsonnet implementation and `binary:<path>` to delegate evaluation to a binary (with the same API as the regular `jsonnet` binary)
	#[arg(long, default_value = "go")]
	pub jsonnet_implementation: String,

	/// Jsonnet VM max stack. Increase this if you get: max stack frames exceeded
	#[arg(long, default_value = "500")]
	pub max_stack: i32,

	/// Set code value of top level function (Format: key=<code>)
	#[arg(long)]
	pub tla_code: Vec<String>,

	/// Set string value of top level function (Format: key=value)
	#[arg(short = 'A', long)]
	pub tla_str: Vec<String>,
}

impl JsonnetArgs for EvalArgs {
	fn ext_str(&self) -> &[String] {
		&self.ext_str
	}
	fn ext_code(&self) -> &[String] {
		&self.ext_code
	}
	fn tla_str(&self) -> &[String] {
		&self.tla_str
	}
	fn tla_code(&self) -> &[String] {
		&self.tla_code
	}
	fn max_stack(&self) -> i32 {
		self.max_stack
	}
	fn name(&self) -> Option<&str> {
		None
	}
}

/// Run the eval command with injected dependencies.
pub fn run<W: Write, R: ImportResolver>(
	import_resolver: R,
	entrypoint: &Path,
	config_base: Option<&Path>,
	spec: Option<Environment>,
	opts: EvalOpts,
	mut writer: W,
) -> Result<()> {
	let result = eval::eval_with_resolver(import_resolver, entrypoint, config_base, spec, opts)?;

	let output = serde_json::to_string_pretty(&result.value)?;
	write!(writer, "{}", output)?;
	Ok(())
}

/// Build EvalOpts from EvalArgs.
pub fn build_eval_opts(args: &EvalArgs) -> EvalOpts {
	UnimplementedArgs {
		jsonnet_implementation: Some(&args.jsonnet_implementation),
		cache_envs: None,
		cache_path: None,
		mem_ballast_size_bytes: None,
	}
	.warn_if_set();

	let mut opts = super::util::build_eval_opts(args);
	opts.eval_expr = args.eval.clone();
	opts
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use assert_matches::assert_matches;
	use indoc::indoc;

	use super::*;
	use crate::{
		commands::util::BrokenPipeGuard,
		test_utils::{BrokenPipeWriter, MemoryImportResolver},
	};

	const ENTRYPOINT: &str = "/test/main.jsonnet";

	fn entrypoint() -> PathBuf {
		PathBuf::from(ENTRYPOINT)
	}

	#[test]
	fn test_eval_outputs_json_object() {
		let resolver = MemoryImportResolver::new().with_file(
			ENTRYPOINT,
			indoc! {r#"
				{
					name: "test",
					value: 42,
				}
			"#},
		);

		let mut output = Vec::new();
		run(
			resolver,
			&entrypoint(),
			None,
			None,
			EvalOpts::default(),
			&mut output,
		)
		.expect("eval should succeed");

		let output_str = String::from_utf8(output).expect("output should be valid UTF-8");
		let parsed: serde_json::Value =
			serde_json::from_str(&output_str).expect("output should be valid JSON");

		assert_eq!(
			parsed,
			serde_json::json!({
				"name": "test",
				"value": 42
			})
		);
	}

	#[test]
	fn test_eval_exits_cleanly_on_broken_pipe() {
		let resolver = MemoryImportResolver::new().with_file(
			ENTRYPOINT,
			indoc! {r#"
				{
					name: "test",
				}
			"#},
		);

		// Wrap BrokenPipeWriter with BrokenPipeGuard to test the guard handles broken pipes
		let writer = BrokenPipeGuard::new(BrokenPipeWriter);
		let result = run(
			resolver,
			&entrypoint(),
			None,
			None,
			EvalOpts::default(),
			writer,
		);

		// The command should exit cleanly on broken pipe, not panic or error
		assert_matches!(result, Ok(()));
	}
}
