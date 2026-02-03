//! Export command handler.

use std::{io::Write, path::PathBuf};

use anyhow::Result;
use clap::Args;

use super::util::UnimplementedArgs;
use crate::{
	eval::EvalOpts,
	export::{self as export_impl, ExportMergeStrategy, ExportOpts},
};

#[derive(Args)]
pub struct ExportArgs {
	/// Output directory
	pub output_dir: String,

	/// Paths to export
	pub paths: Vec<String>,

	/// Regexes which define which environment should be cached (if caching is enabled)
	#[arg(short = 'e', long)]
	pub cache_envs: Vec<String>,

	/// Local file path where cached evaluations should be stored
	#[arg(short = 'c', long)]
	pub cache_path: Option<String>,

	/// Set code value of extVar (Format: key=<code>)
	#[arg(long)]
	pub ext_code: Vec<String>,

	/// Set string value of extVar (Format: key=value)
	#[arg(short = 'V', long)]
	pub ext_str: Vec<String>,

	/// File extension
	#[arg(long, default_value = "yaml")]
	pub extension: String,

	/// https://tanka.dev/exporting#filenames
	#[arg(
		long,
		default_value = "{{.apiVersion}}.{{.kind}}-{{or .metadata.name .metadata.generateName}}"
	)]
	pub format: String,

	/// Use `go` to use native go-jsonnet implementation and `binary:<path>` to delegate evaluation to a binary (with the same API as the regular `jsonnet` binary)
	#[arg(long, default_value = "go")]
	pub jsonnet_implementation: String,

	/// Log level (possible values: disabled, fatal, error, warn, info, debug, trace)
	#[arg(long, default_value = "info")]
	pub log_level: String,

	/// Jsonnet VM max stack. Increase this if you get: max stack frames exceeded
	#[arg(long, default_value = "500")]
	pub max_stack: i32,

	/// Size of memory ballast to allocate. This may improve performance for large environments.
	#[arg(long)]
	pub mem_ballast_size_bytes: Option<i64>,

	/// Tanka main files that have been deleted. This is used when using a merge strategy to also delete the files of these deleted environments.
	#[arg(long)]
	pub merge_deleted_envs: Vec<String>,

	/// What to do when exporting to an existing directory. The default setting is to disallow exporting to an existing directory. Values: 'fail-on-conflicts', 'replace-envs'
	#[arg(long)]
	pub merge_strategy: Option<String>,

	/// String that only a single inline environment contains in its name
	#[arg(long)]
	pub name: Option<String>,

	/// Number of environments to process in parallel
	#[arg(short = 'p', long, default_value = "8")]
	pub parallel: i32,

	/// Look recursively for Tanka environments
	#[arg(short = 'r', long)]
	pub recursive: bool,

	/// Label selector. Uses the same syntax as kubectl does
	#[arg(short = 'l', long)]
	pub selector: Option<String>,

	/// Skip generating manifest.json file that tracks exported files
	#[arg(long)]
	pub skip_manifest: bool,

	/// Regex filter on '<kind>/<name>'. See https://tanka.dev/output-filtering
	#[arg(short = 't', long)]
	pub target: Vec<String>,

	/// Set code value of top level function (Format: key=<code>)
	#[arg(long)]
	pub tla_code: Vec<String>,

	/// Set string value of top level function (Format: key=value)
	#[arg(short = 'A', long)]
	pub tla_str: Vec<String>,
}

/// Run the export command.
pub fn run<W: Write>(args: ExportArgs, mut writer: W) -> Result<()> {
	UnimplementedArgs {
		jsonnet_implementation: Some(&args.jsonnet_implementation),
		cache_envs: Some(&args.cache_envs),
		cache_path: Some(&args.cache_path),
		mem_ballast_size_bytes: Some(&args.mem_ballast_size_bytes),
	}
	.warn_if_set();

	let opts = build_export_opts(&args)?;
	let result = export_impl::export(&args.paths, opts)?;

	// Match tk behavior: silent on success, errors reported via the provided writer
	// But report fatal errors prominently and summarize skipped ones
	let mut fatal_error: Option<(PathBuf, String)> = None;
	let mut env_errors = Vec::new();
	let mut skipped_count = 0;

	for env_result in &result.results {
		if let Some(ref error) = env_result.error {
			if error.starts_with("FATAL:") && fatal_error.is_none() {
				// Capture the first fatal error
				fatal_error = Some((env_result.env_path.clone(), error.clone()));
			} else if error == "Skipped due to earlier fatal error" {
				skipped_count += 1;
			} else {
				// Regular environment error
				env_errors.push((env_result.env_path.clone(), error.clone()));
			}
		}
	}

	// Report fatal error first if present
	if let Some((path, error)) = fatal_error {
		writeln!(writer, "\n{}", "=".repeat(80))?;
		writeln!(writer, "FATAL ERROR during export:")?;
		writeln!(writer, "{}", "=".repeat(80))?;
		writeln!(writer, "  Environment: {:?}", path)?;
		writeln!(
			writer,
			"  Error: {}",
			error.strip_prefix("FATAL: ").unwrap_or(&error)
		)?;
		writeln!(writer, "{}", "=".repeat(80))?;
		writeln!(writer)?;
	}

	// Report individual environment errors
	for (path, error) in &env_errors {
		writeln!(writer, "  ✗ {:?}: {}", path, error)?;
	}

	// Summarize skipped environments
	if skipped_count > 0 {
		writeln!(
			writer,
			"\nSkipped {} environments due to earlier fatal error",
			skipped_count
		)?;
	}

	if result.failed > 0 {
		anyhow::bail!("{} environments failed to export", result.failed);
	}

	Ok(())
}

fn build_export_opts(args: &ExportArgs) -> Result<ExportOpts> {
	// Parse ext_code flags
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

	let eval_opts = EvalOpts {
		ext_str: ext_str_map,
		ext_code: ext_code_map,
		tla_str: tla_str_map,
		tla_code: tla_code_map,
		max_stack: Some(args.max_stack as usize),
		eval_expr: None,
		env_name: None,
		export_jsonnet_implementation: None,
	};

	// Parse merge strategy
	let merge_strategy = if let Some(ref strategy) = args.merge_strategy {
		strategy.parse::<ExportMergeStrategy>()?
	} else {
		ExportMergeStrategy::default()
	};

	Ok(ExportOpts {
		output_dir: PathBuf::from(&args.output_dir),
		extension: args.extension.clone(),
		format: args.format.clone(),
		parallelism: args.parallel as usize,
		eval_opts,
		name: args.name.clone(),
		recursive: args.recursive,
		selector: args.selector.clone(),
		skip_manifest: args.skip_manifest,
		target: args.target.clone(),
		merge_strategy,
		merge_deleted_envs: args.merge_deleted_envs.clone(),
		show_timing: false,
	})
}
