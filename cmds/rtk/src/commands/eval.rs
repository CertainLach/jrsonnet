//! Eval command handler.

use std::{io::Write, path::Path};

use anyhow::Result;
use clap::Args;
use jrsonnet_evaluator::ImportResolver;

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

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

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
	// Parse ext_code flags (format: key=value)
	let mut ext_code_map = std::collections::HashMap::new();
	for item in &args.ext_code {
		if let Some((key, value)) = item.split_once('=') {
			ext_code_map.insert(key.to_string(), value.to_string());
		}
	}

	// Parse ext_str flags
	let mut ext_str_map = std::collections::HashMap::new();
	for item in &args.ext_str {
		if let Some((key, value)) = item.split_once('=') {
			ext_str_map.insert(key.to_string(), value.to_string());
		}
	}

	// Parse tla_code flags
	let mut tla_code_map = std::collections::HashMap::new();
	for item in &args.tla_code {
		if let Some((key, value)) = item.split_once('=') {
			tla_code_map.insert(key.to_string(), value.to_string());
		}
	}

	// Parse tla_str flags
	let mut tla_str_map = std::collections::HashMap::new();
	for item in &args.tla_str {
		if let Some((key, value)) = item.split_once('=') {
			tla_str_map.insert(key.to_string(), value.to_string());
		}
	}

	EvalOpts {
		ext_str: ext_str_map,
		ext_code: ext_code_map,
		tla_str: tla_str_map,
		tla_code: tla_code_map,
		max_stack: Some(args.max_stack as usize),
		eval_expr: args.eval.clone(),
		env_name: None,
		export_jsonnet_implementation: None,
	}
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
