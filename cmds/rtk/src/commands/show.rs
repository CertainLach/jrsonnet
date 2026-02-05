//! Show command handler.
//!
//! Evaluates a Tanka environment and outputs the resulting Kubernetes manifests
//! as YAML. This is equivalent to `tk show`.

use std::io::Write;

use anyhow::{Context, Result};
use clap::Args;
use tracing::instrument;

use super::util::{
	build_eval_opts, extract_manifests, process_manifests, JsonnetArgs, UnimplementedArgs,
};
use crate::{eval::EvalOpts, yaml::sort_json_keys};

#[derive(Args)]
pub struct ShowArgs {
	/// Path to the Tanka environment
	pub path: String,

	/// Allow redirecting output to a file or a pipe
	#[arg(long)]
	pub dangerous_allow_redirect: bool,

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

	/// String that only a single inline environment contains in its name
	#[arg(long)]
	pub name: Option<String>,

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

crate::impl_jsonnet_args!(ShowArgs);

/// Options for the show operation.
#[derive(Default)]
pub struct ShowOpts {
	/// Target filters.
	pub target: Vec<String>,
	/// Filter environments by name (exact match first, then substring).
	pub name: Option<String>,
}

/// Run the show command.
pub fn run<W: Write>(args: ShowArgs, mut writer: W) -> Result<()> {
	UnimplementedArgs::warn_jsonnet_impl(&args.jsonnet_implementation);

	// Check redirect safety (matches tk behavior)
	let is_terminal = std::io::IsTerminal::is_terminal(&std::io::stdout());
	let allow_redirect_env = std::env::var("TANKA_DANGEROUS_ALLOW_REDIRECT")
		.map(|v| v == "true")
		.unwrap_or(false);
	let allow_redirect = allow_redirect_env || args.dangerous_allow_redirect;

	if !is_terminal && !allow_redirect {
		eprintln!(
			"Redirection of the output of rtk show is discouraged and disabled by default.
If you want to export .yaml files for use with other tools, try 'rtk export'.
Otherwise run:
  rtk show --dangerous-allow-redirect 
or set the environment variable 
  TANKA_DANGEROUS_ALLOW_REDIRECT=true 
to bypass this check."
		);
		return Ok(());
	}

	let opts = ShowOpts {
		target: args.target.clone(),
		name: args.name.clone(),
	};

	let output = show_environment(&args.path, build_eval_opts(&args), opts)?;

	write!(writer, "{}", output)?;
	Ok(())
}

/// Show an environment and return the YAML output.
#[instrument(skip_all, fields(path = %path))]
pub fn show_environment(path: &str, eval_opts: EvalOpts, opts: ShowOpts) -> Result<String> {
	use super::util::evaluate_single_environment;

	let env_data = evaluate_single_environment(path, eval_opts, opts.name.as_deref())?;

	// Extract manifests from environment data
	let mut manifests = extract_manifests(&env_data.data, &opts.target)?;
	tracing::debug!(manifest_count = manifests.len(), "found manifests to show");

	process_manifests(&mut manifests, &env_data.spec);

	// Serialize all manifests to YAML
	manifests_to_yaml(&manifests)
}

/// Convert manifests to a YAML stream.
fn manifests_to_yaml(manifests: &[serde_json::Value]) -> Result<String> {
	let mut output = String::new();

	for (i, manifest) in manifests.iter().enumerate() {
		// Add document separator for subsequent documents
		if i > 0 {
			output.push_str("---\n");
		}

		// Sort keys and serialize to YAML (matches tk's output format)
		let sorted = sort_json_keys(manifest.clone());

		// Use serializer options to match Go's yaml.v2 output (used by tk for show)
		let options = serde_saphyr::SerializerOptions {
			indent_step: 2,
			indent_array: Some(0),
			prefer_block_scalars: true,
			empty_map_as_braces: true,
			empty_array_as_brackets: true,
			line_width: Some(80),
			scientific_notation_threshold: Some(1000000),
			scientific_notation_small_threshold: Some(0.0001),
			quote_ambiguous_keys: true,
			quote_numeric_strings: true,
			..Default::default()
		};

		serde_saphyr::to_fmt_writer_with_options(&mut output, &sorted, options)
			.context("serializing manifest to YAML")?;
	}

	Ok(output)
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	fn setup_test_env(temp: &TempDir, main_content: &str) -> std::path::PathBuf {
		let root = temp.path();
		fs::write(root.join("jsonnetfile.json"), r#"{"version": 1}"#).unwrap();
		fs::create_dir_all(root.join("env")).unwrap();
		fs::write(root.join("env/main.jsonnet"), main_content).unwrap();
		root.join("env")
	}

	#[test]
	fn test_show_single_manifest() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			r#"{
				apiVersion: 'v1',
				kind: 'ConfigMap',
				metadata: { name: 'test-cm', namespace: 'default' },
				data: { key: 'value' }
			}"#,
		);

		let output = show_environment(
			env_path.to_str().unwrap(),
			EvalOpts::default(),
			ShowOpts::default(),
		)
		.unwrap();

		assert!(output.contains("apiVersion: v1"));
		assert!(output.contains("kind: ConfigMap"));
		assert!(output.contains("name: test-cm"));
	}

	#[test]
	fn test_show_multiple_manifests() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			r#"{
				cm: {
					apiVersion: 'v1',
					kind: 'ConfigMap',
					metadata: { name: 'cm1' },
				},
				secret: {
					apiVersion: 'v1',
					kind: 'Secret',
					metadata: { name: 'secret1' },
				}
			}"#,
		);

		let output = show_environment(
			env_path.to_str().unwrap(),
			EvalOpts::default(),
			ShowOpts::default(),
		)
		.unwrap();

		// Should have document separator between manifests
		assert!(output.contains("---"));
		assert!(output.contains("kind: ConfigMap"));
		assert!(output.contains("kind: Secret"));
	}

	#[test]
	fn test_show_with_target_filter() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			r#"{
				cm: {
					apiVersion: 'v1',
					kind: 'ConfigMap',
					metadata: { name: 'cm1' },
				},
				secret: {
					apiVersion: 'v1',
					kind: 'Secret',
					metadata: { name: 'secret1' },
				}
			}"#,
		);

		let output = show_environment(
			env_path.to_str().unwrap(),
			EvalOpts::default(),
			ShowOpts {
				target: vec!["ConfigMap/.*".to_string()],
				..Default::default()
			},
		)
		.unwrap();

		assert!(output.contains("kind: ConfigMap"));
		assert!(!output.contains("kind: Secret"));
	}

	#[test]
	fn test_show_inline_environment() {
		let temp = TempDir::new().unwrap();
		let env_path = setup_test_env(
			&temp,
			r#"{
				apiVersion: 'tanka.dev/v1alpha1',
				kind: 'Environment',
				metadata: { name: 'my-env' },
				spec: { namespace: 'default' },
				data: {
					cm: {
						apiVersion: 'v1',
						kind: 'ConfigMap',
						metadata: { name: 'inline-cm' },
					}
				}
			}"#,
		);

		let output = show_environment(
			env_path.to_str().unwrap(),
			EvalOpts::default(),
			ShowOpts::default(),
		)
		.unwrap();

		assert!(output.contains("kind: ConfigMap"));
		assert!(output.contains("name: inline-cm"));
	}

	#[test]
	fn test_extract_manifests_from_array() {
		let value = serde_json::json!([
			{
				"apiVersion": "v1",
				"kind": "ConfigMap",
				"metadata": { "name": "cm1" }
			},
			{
				"apiVersion": "v1",
				"kind": "Secret",
				"metadata": { "name": "secret1" }
			}
		]);

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 2);
	}

	#[test]
	fn test_extract_manifests_from_list() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "List",
			"items": [
				{
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "cm1" }
				},
				{
					"apiVersion": "v1",
					"kind": "ConfigMap",
					"metadata": { "name": "cm2" }
				}
			]
		});

		let manifests = extract_manifests(&value, &[]).unwrap();
		assert_eq!(manifests.len(), 2);
	}
}
