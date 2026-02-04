//! Show command handler.
//!
//! Evaluates a Tanka environment and outputs the resulting Kubernetes manifests
//! as YAML. This is equivalent to `tk show`.

use std::io::Write;

use anyhow::{Context, Result};
use clap::Args;
use tracing::instrument;

use super::util::UnimplementedArgs;
use crate::{
	eval::EvalOpts,
	spec::{self, EnvironmentData},
	yaml::sort_json_keys,
};

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
	UnimplementedArgs {
		jsonnet_implementation: Some(&args.jsonnet_implementation),
		cache_envs: None,
		cache_path: None,
		mem_ballast_size_bytes: None,
	}
	.warn_if_set();

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

	let eval_opts = build_eval_opts(&args);
	let opts = ShowOpts {
		target: args.target,
		name: args.name,
	};

	let output = show_environment(&args.path, eval_opts, opts)?;

	write!(writer, "{}", output)?;
	Ok(())
}

/// Show an environment and return the YAML output.
#[instrument(skip_all, fields(path = %path))]
pub fn show_environment(path: &str, eval_opts: EvalOpts, opts: ShowOpts) -> Result<String> {
	// Evaluate the environment
	tracing::debug!(path = %path, "evaluating environment");
	let eval_result = crate::eval::eval(path, eval_opts)
		.context(format!("evaluating environment at {}", path))?;

	// Extract environments (handles both inline and static environments)
	let mut environments = spec::extract_environments(&eval_result.value, &eval_result.spec);

	// For inline environments, set metadata.namespace to file path
	if eval_result.spec.is_none() {
		spec::set_inline_env_namespace(&mut environments, path);
	}

	// Filter by name if specified
	if let Some(ref target_name) = opts.name {
		environments = filter_environments_by_name(environments, target_name).map_err(|_| {
			anyhow::anyhow!(
				"no environment found matching name '{}'. Available environments: {}",
				target_name,
				get_environment_names(&spec::extract_environments(
					&eval_result.value,
					&eval_result.spec
				))
			)
		})?;
	}

	// For show, we only support a single environment
	let [env_data] = <[_; 1]>::try_from(environments).map_err(|envs: Vec<_>| {
		anyhow::anyhow!(
			"multiple inline environments found ({}). Use --name to select one: {}",
			envs.len(),
			get_environment_names(&envs)
		)
	})?;

	// Extract manifests from environment data
	let mut manifests = extract_manifests(&env_data.data, &opts.target)?;
	tracing::debug!(manifest_count = manifests.len(), "found manifests to show");

	// Inject tanka.dev/environment label if injectLabels is enabled
	for manifest in &mut manifests {
		spec::inject_environment_label(manifest, &env_data.spec);
	}

	// Strip empty annotations/labels (matches Tanka's pkg/process/namespace.go)
	for manifest in &mut manifests {
		spec::strip_null_metadata_fields(manifest);
	}

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

/// Parse key=value pairs into a HashMap.
fn parse_key_value_pairs(items: &[String]) -> std::collections::HashMap<String, String> {
	items
		.iter()
		.filter_map(|s| {
			s.split_once('=')
				.map(|(k, v)| (k.to_string(), v.to_string()))
		})
		.collect()
}

/// Build evaluation options from command args.
fn build_eval_opts(args: &ShowArgs) -> EvalOpts {
	EvalOpts {
		ext_str: parse_key_value_pairs(&args.ext_str),
		ext_code: parse_key_value_pairs(&args.ext_code),
		tla_str: parse_key_value_pairs(&args.tla_str),
		tla_code: parse_key_value_pairs(&args.tla_code),
		max_stack: Some(args.max_stack as usize),
		eval_expr: None,
		env_name: args.name.clone(),
		export_jsonnet_implementation: None,
	}
}

/// Extract Kubernetes manifests from the evaluation result.
///
/// The evaluation result can be:
/// - A single manifest object
/// - An array of manifests
/// - A nested object containing manifests (Tanka environment format)
fn extract_manifests(
	value: &serde_json::Value,
	target_filters: &[String],
) -> Result<Vec<serde_json::Value>> {
	let mut manifests = Vec::new();
	collect_manifests(value, &mut manifests);

	// Apply target filters if specified
	if !target_filters.is_empty() {
		let filters: Vec<regex::Regex> = target_filters
			.iter()
			.map(|f| regex::Regex::new(f))
			.collect::<Result<Vec<_>, _>>()
			.context("invalid target filter regex")?;

		manifests.retain(|m| {
			let kind = m.get("kind").and_then(|v| v.as_str()).unwrap_or("");
			let name = m
				.pointer("/metadata/name")
				.and_then(|v| v.as_str())
				.unwrap_or("");
			let target = format!("{}/{}", kind, name);

			filters.iter().any(|f| f.is_match(&target))
		});
	}

	Ok(manifests)
}

/// Recursively collect Kubernetes manifests from a JSON value.
fn collect_manifests(value: &serde_json::Value, manifests: &mut Vec<serde_json::Value>) {
	match value {
		serde_json::Value::Object(map) => {
			// Check if this looks like a Kubernetes manifest
			if map.contains_key("apiVersion") && map.contains_key("kind") {
				// Check if it's a List
				if map.get("kind").and_then(|v| v.as_str()) == Some("List") {
					if let Some(items) = map.get("items").and_then(|v| v.as_array()) {
						for item in items {
							collect_manifests(item, manifests);
						}
					}
				} else {
					// Regular manifest
					manifests.push(value.clone());
				}
			} else {
				// Nested object - recurse into values
				for (_, v) in map {
					collect_manifests(v, manifests);
				}
			}
		}
		serde_json::Value::Array(arr) => {
			for item in arr {
				collect_manifests(item, manifests);
			}
		}
		_ => {
			// Ignore primitives
		}
	}
}

/// Get the name of an environment, if available.
fn env_name(env_data: &EnvironmentData) -> Option<&str> {
	env_data
		.spec
		.as_ref()
		.and_then(|s| s.metadata.name.as_deref())
}

/// Format a list of environment names for error messages.
fn get_environment_names(environments: &[EnvironmentData]) -> String {
	let names: Vec<&str> = environments.iter().filter_map(env_name).collect();
	if names.is_empty() {
		"(unnamed environments)".to_string()
	} else {
		names.join(", ")
	}
}

/// Filter environments by name, trying exact match first, then substring.
fn filter_environments_by_name(
	environments: Vec<EnvironmentData>,
	target_name: &str,
) -> Result<Vec<EnvironmentData>> {
	// Try exact match first
	let exact: Vec<_> = environments
		.iter()
		.filter(|e| env_name(e) == Some(target_name))
		.cloned()
		.collect();

	if let [_single] = exact.as_slice() {
		return Ok(exact);
	}

	// Fall back to substring matching
	let matches: Vec<_> = environments
		.into_iter()
		.filter(|e| env_name(e).is_some_and(|n| n.contains(target_name)))
		.collect();

	if matches.is_empty() {
		anyhow::bail!("no environment found matching name '{}'", target_name);
	}

	Ok(matches)
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
