use std::{collections::BTreeMap, fmt};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Diff strategy selection.
///
/// Determines how rtk computes differences between local manifests and cluster state.
/// Each strategy has different trade-offs in terms of accuracy, performance, and
/// Kubernetes version requirements.
///
/// # Strategies
///
/// ## Native (default)
/// Uses client-side dry-run apply (`PATCH` with `dryRun=All`). The server computes
/// what the resource would look like after applying, and rtk diffs against that.
/// Requires Kubernetes 1.13+.
///
/// ## Server
/// Uses server-side apply with field manager and `--force-conflicts`. The server
/// fully processes the apply (including webhooks) in dry-run mode.
/// Requires Kubernetes 1.16+.
///
/// ## Validate
/// Validates all manifests on server first, then uses native strategy for diffs.
/// Useful when you want early validation errors before seeing diffs.
/// Requires Kubernetes 1.16+.
///
/// ## Subset
/// Fetches current state via GET and compares only fields present in the manifest.
/// Fallback for older Kubernetes versions without dry-run support.
/// Works with any Kubernetes version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffStrategy {
	/// Client-side dry-run apply (k8s 1.13+). Uses `PATCH` with `dryRun=All` to compute merged
	/// state.
	#[default]
	Native,

	/// Server-side dry-run with field manager (k8s 1.16+). Uses server-side apply with
	/// `--force-conflicts` for most accurate results including webhook mutations.
	Server,

	/// Server-side validation + native diff (k8s 1.16+). Validates all manifests on server
	/// first, then computes diffs using native strategy.
	Validate,

	/// GET + compare only manifest fields (any k8s version). Fetches current state and compares
	/// only fields present in the manifest. Fallback for older clusters.
	Subset,
}

impl fmt::Display for DiffStrategy {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DiffStrategy::Native => write!(f, "native"),
			DiffStrategy::Server => write!(f, "server"),
			DiffStrategy::Validate => write!(f, "validate"),
			DiffStrategy::Subset => write!(f, "subset"),
		}
	}
}

impl DiffStrategy {
	/// Select appropriate diff strategy based on spec and server version.
	///
	/// Priority:
	/// 1. If `spec.diffStrategy` is explicitly set → use that
	/// 2. If `spec.applyStrategy == "server"` → Server
	/// 3. Else if k8s >= 1.13 → Native
	/// 4. Else → Subset
	pub fn from_spec(
		spec: &Spec,
		server_version: &k8s_openapi::apimachinery::pkg::version::Info,
	) -> Self {
		// Respect explicit diffStrategy if set
		if let Some(strategy) = &spec.diff_strategy {
			match strategy.as_str() {
				"native" => return DiffStrategy::Native,
				"server" => return DiffStrategy::Server,
				"validate" => return DiffStrategy::Validate,
				"subset" => return DiffStrategy::Subset,
				_ => tracing::warn!(
					"Unknown diffStrategy '{}', using automatic selection",
					strategy
				),
			}
		}

		// Fall back to automatic selection
		if spec.apply_strategy.as_deref() == Some("server") {
			return DiffStrategy::Server;
		}

		// k8s 1.13+ supports client-side dry-run
		let major: u32 = server_version.major.parse().unwrap_or(1);
		let minor: u32 = server_version
			.minor
			.trim_end_matches('+')
			.parse()
			.unwrap_or(0);

		if major >= 1 && minor >= 13 {
			DiffStrategy::Native
		} else {
			DiffStrategy::Subset
		}
	}
}

/// Environment represents a Tanka environment (tanka.dev/v1alpha1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
	pub api_version: String,
	pub kind: String,
	pub metadata: Metadata,
	pub spec: Spec,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub namespace: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub api_server: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub context_names: Option<Vec<String>>,
	#[serde(default = "default_namespace")]
	pub namespace: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub diff_strategy: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub apply_strategy: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub inject_labels: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub resource_defaults: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub expect_versions: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub export_jsonnet_implementation: Option<String>,
}

fn default_namespace() -> String {
	"default".to_string()
}

impl Default for Spec {
	fn default() -> Self {
		Self {
			api_server: None,
			context_names: None,
			namespace: default_namespace(),
			diff_strategy: None,
			apply_strategy: None,
			inject_labels: None,
			resource_defaults: None,
			expect_versions: None,
			export_jsonnet_implementation: None,
		}
	}
}

impl Environment {
	/// Create a new default environment
	pub fn new() -> Self {
		Self {
			api_version: "tanka.dev/v1alpha1".to_string(),
			kind: "Environment".to_string(),
			metadata: Metadata {
				name: None,
				namespace: None,
				labels: Some(BTreeMap::new()),
			},
			spec: Spec::default(),
			data: None,
		}
	}
}

impl Default for Environment {
	fn default() -> Self {
		Self::new()
	}
}

/// Generate the tanka.dev/environment label value
/// This replicates Tanka's NameLabel() function which creates a SHA256 hash
/// of the environment's metadata.name and metadata.namespace
pub fn generate_environment_label(env: &Environment) -> String {
	use sha2::{Digest, Sha256};

	// By default, use metadata.name and metadata.namespace
	// Format: "name:namespace"
	let name = env.metadata.name.as_deref().unwrap_or("");
	let namespace = env.metadata.namespace.as_deref().unwrap_or("");
	let label_parts = format!("{}:{}", name, namespace);

	// Compute SHA256 hash
	let mut hasher = Sha256::new();
	hasher.update(label_parts.as_bytes());
	let result = hasher.finalize();

	// Convert to hex and take first 48 characters
	let hex = format!("{:x}", result);
	hex.chars().take(48).collect()
}

/// Inject tanka.dev/environment label into manifest metadata.
///
/// This replicates the behavior from Tanka's pkg/process/process.go.
/// Only injects if `env_spec` exists and `injectLabels` is true.
pub fn inject_environment_label(manifest: &mut serde_json::Value, env_spec: &Option<Environment>) {
	let Some(env) = env_spec else { return };
	if !env.spec.inject_labels.unwrap_or(false) {
		return;
	}

	let label_value = generate_environment_label(env);

	let serde_json::Value::Object(ref mut obj) = manifest else {
		return;
	};

	// Ensure metadata exists
	if !obj.contains_key("metadata") {
		obj.insert(
			"metadata".to_string(),
			serde_json::Value::Object(serde_json::Map::new()),
		);
	}

	let Some(serde_json::Value::Object(ref mut metadata)) = obj.get_mut("metadata") else {
		return;
	};

	// Ensure labels exists
	if !metadata.contains_key("labels") {
		metadata.insert(
			"labels".to_string(),
			serde_json::Value::Object(serde_json::Map::new()),
		);
	}

	let Some(serde_json::Value::Object(ref mut labels)) = metadata.get_mut("labels") else {
		return;
	};

	labels.insert(
		"tanka.dev/environment".to_string(),
		serde_json::Value::String(label_value),
	);
}

/// Data extracted from an inline or static environment.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentData {
	/// The environment spec (if available).
	pub spec: Option<Environment>,
	/// The manifest data (from `data` field for inline, or entire value for static).
	pub data: serde_json::Value,
}

/// Set metadata.namespace on inline environments.
///
/// For inline environments (those without spec.json), metadata.namespace should be
/// the relative path from project root to the entrypoint file.
pub fn set_inline_env_namespace(environments: &mut [EnvironmentData], path: &str) {
	let jpath_result = match crate::jpath::resolve(path) {
		Ok(r) => r,
		Err(_) => return,
	};

	let rel_entrypoint = match jpath_result.entrypoint.strip_prefix(&jpath_result.root) {
		Ok(r) => r,
		Err(_) => return,
	};

	let namespace = rel_entrypoint.to_string_lossy().to_string();

	for env_data in environments {
		let Some(ref mut spec) = env_data.spec else {
			continue;
		};
		spec.metadata.namespace = Some(namespace.clone());
	}
}

/// Check if a JSON value is an inline Environment object.
pub fn is_inline_environment(value: &serde_json::Value) -> bool {
	if let serde_json::Value::Object(obj) = value {
		obj.get("kind").and_then(|v| v.as_str()) == Some("Environment")
			&& obj.contains_key("apiVersion")
	} else {
		false
	}
}

/// Extract Environment objects from evaluated Jsonnet output.
///
/// For inline environments (output contains Environment objects), extracts each one.
/// For static environments (no Environment objects), wraps the output with the default spec.
pub fn extract_environments(
	value: &serde_json::Value,
	default_spec: &Option<Environment>,
) -> Vec<EnvironmentData> {
	let mut environments = Vec::new();
	collect_environments_recursive(value, &mut environments);

	// Deduplicate by name, keeping the last occurrence
	environments.reverse();
	let mut seen_names = std::collections::HashSet::new();
	environments.retain(|env_data| {
		let name = env_data
			.spec
			.as_ref()
			.and_then(|s| s.metadata.name.as_ref())
			.cloned()
			.unwrap_or_default();
		seen_names.insert(name)
	});
	environments.reverse();

	if !environments.is_empty() {
		return environments;
	}

	// No inline environments found - treat as static environment
	environments.push(EnvironmentData {
		spec: default_spec.clone(),
		data: value.clone(),
	});

	environments
}

/// Recursively collect Environment objects from a JSON value.
fn collect_environments_recursive(
	value: &serde_json::Value,
	environments: &mut Vec<EnvironmentData>,
) {
	match value {
		serde_json::Value::Object(obj) => {
			if is_inline_environment(value) {
				let data = obj.get("data").cloned().unwrap_or(serde_json::Value::Null);
				let env_spec: Result<Environment, _> = serde_json::from_value(value.clone());
				let env_spec_opt = match env_spec {
					Ok(spec) => Some(spec),
					Err(e) => {
						tracing::warn!("Failed to parse Environment object: {}", e);
						None
					}
				};
				environments.push(EnvironmentData {
					spec: env_spec_opt,
					data,
				});
			} else {
				for v in obj.values() {
					collect_environments_recursive(v, environments);
				}
			}
		}
		serde_json::Value::Array(arr) => {
			for item in arr {
				collect_environments_recursive(item, environments);
			}
		}
		_ => {}
	}
}

/// Strip null or empty values from metadata.annotations and metadata.labels
/// This matches Tanka/Kubernetes behavior where null and empty fields are omitted from output
pub fn strip_null_metadata_fields(manifest: &mut serde_json::Value) {
	if let serde_json::Value::Object(ref mut obj) = manifest {
		if let Some(serde_json::Value::Object(ref mut metadata)) = obj.get_mut("metadata") {
			// Remove annotations if it's null or empty
			if is_null_or_empty_object(metadata.get("annotations")) {
				metadata.remove("annotations");
			}
			// Remove labels if it's null or empty
			if is_null_or_empty_object(metadata.get("labels")) {
				metadata.remove("labels");
			}
		}
	}
}

fn is_null_or_empty_object(value: Option<&serde_json::Value>) -> bool {
	match value {
		Some(serde_json::Value::Null) => true,
		Some(serde_json::Value::Object(m)) if m.is_empty() => true,
		_ => false,
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[test]
	fn test_extract_environments_deduplicates_by_name() {
		// Use an array to have deterministic ordering - last-wins means env2 should be kept
		let value = serde_json::json!([
			{
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "my-env" },
				"spec": { "namespace": "default" },
				"data": {
					"cm": {
						"apiVersion": "v1",
						"kind": "ConfigMap",
						"metadata": { "name": "config1" }
					}
				}
			},
			{
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "my-env" },
				"spec": { "namespace": "ns2" },
				"data": {
					"cm": {
						"apiVersion": "v1",
						"kind": "ConfigMap",
						"metadata": { "name": "config2" }
					}
				}
			}
		]);

		let environments = extract_environments(&value, &None);

		// Only one environment should remain after deduplication (last-wins)
		assert_eq!(
			environments,
			vec![EnvironmentData {
				spec: Some(Environment {
					api_version: "tanka.dev/v1alpha1".to_string(),
					kind: "Environment".to_string(),
					metadata: Metadata {
						name: Some("my-env".to_string()),
						namespace: None,
						labels: None,
					},
					spec: Spec {
						api_server: None,
						context_names: None,
						namespace: "ns2".to_string(),
						diff_strategy: None,
						apply_strategy: None,
						inject_labels: None,
						resource_defaults: None,
						expect_versions: None,
						export_jsonnet_implementation: None,
					},
					data: Some(serde_json::json!({
						"cm": {
							"apiVersion": "v1",
							"kind": "ConfigMap",
							"metadata": { "name": "config2" }
						}
					})),
				}),
				data: serde_json::json!({
					"cm": {
						"apiVersion": "v1",
						"kind": "ConfigMap",
						"metadata": { "name": "config2" }
					}
				}),
			}]
		);
	}

	#[test]
	fn test_extract_environments_keeps_different_names() {
		let value = serde_json::json!({
			"env1": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "env-a" },
				"spec": { "namespace": "ns-a" },
				"data": { "key": "value-a" }
			},
			"env2": {
				"apiVersion": "tanka.dev/v1alpha1",
				"kind": "Environment",
				"metadata": { "name": "env-b" },
				"spec": { "namespace": "ns-b" },
				"data": { "key": "value-b" }
			}
		});

		let mut environments = extract_environments(&value, &None);
		environments.sort_by(|a, b| {
			let a_name = a.spec.as_ref().and_then(|s| s.metadata.name.as_deref());
			let b_name = b.spec.as_ref().and_then(|s| s.metadata.name.as_deref());
			a_name.cmp(&b_name)
		});

		assert_eq!(
			environments,
			vec![
				EnvironmentData {
					spec: Some(Environment {
						api_version: "tanka.dev/v1alpha1".to_string(),
						kind: "Environment".to_string(),
						metadata: Metadata {
							name: Some("env-a".to_string()),
							namespace: None,
							labels: None,
						},
						spec: Spec {
							api_server: None,
							context_names: None,
							namespace: "ns-a".to_string(),
							diff_strategy: None,
							apply_strategy: None,
							inject_labels: None,
							resource_defaults: None,
							expect_versions: None,
							export_jsonnet_implementation: None,
						},
						data: Some(serde_json::json!({ "key": "value-a" })),
					}),
					data: serde_json::json!({ "key": "value-a" }),
				},
				EnvironmentData {
					spec: Some(Environment {
						api_version: "tanka.dev/v1alpha1".to_string(),
						kind: "Environment".to_string(),
						metadata: Metadata {
							name: Some("env-b".to_string()),
							namespace: None,
							labels: None,
						},
						spec: Spec {
							api_server: None,
							context_names: None,
							namespace: "ns-b".to_string(),
							diff_strategy: None,
							apply_strategy: None,
							inject_labels: None,
							resource_defaults: None,
							expect_versions: None,
							export_jsonnet_implementation: None,
						},
						data: Some(serde_json::json!({ "key": "value-b" })),
					}),
					data: serde_json::json!({ "key": "value-b" }),
				}
			]
		);
	}

	#[rstest]
	#[case::environment(serde_json::json!({
		"apiVersion": "tanka.dev/v1alpha1",
		"kind": "Environment",
		"metadata": { "name": "test" },
		"spec": { "namespace": "default" }
	}), true)]
	#[case::configmap(serde_json::json!({
		"apiVersion": "v1",
		"kind": "ConfigMap",
		"metadata": { "name": "test" }
	}), false)]
	#[case::string(serde_json::json!("string"), false)]
	#[case::number(serde_json::json!(123), false)]
	#[case::null(serde_json::json!(null), false)]
	#[case::array(serde_json::json!([1, 2, 3]), false)]
	#[case::missing_kind(serde_json::json!({
		"apiVersion": "tanka.dev/v1alpha1",
		"metadata": { "name": "test" }
	}), false)]
	#[case::missing_api_version(serde_json::json!({
		"kind": "Environment",
		"metadata": { "name": "test" }
	}), false)]
	fn test_is_inline_environment(#[case] value: serde_json::Value, #[case] expected: bool) {
		assert_eq!(is_inline_environment(&value), expected);
	}

	fn make_version(major: &str, minor: &str) -> k8s_openapi::apimachinery::pkg::version::Info {
		k8s_openapi::apimachinery::pkg::version::Info {
			major: major.to_string(),
			minor: minor.to_string(),
			..Default::default()
		}
	}

	fn make_spec_for_strategy(diff_strategy: Option<&str>, apply_strategy: Option<&str>) -> Spec {
		Spec {
			api_server: None,
			context_names: None,
			namespace: "default".to_string(),
			diff_strategy: diff_strategy.map(|s| s.to_string()),
			apply_strategy: apply_strategy.map(|s| s.to_string()),
			inject_labels: Some(true),
			resource_defaults: None,
			expect_versions: None,
			export_jsonnet_implementation: None,
		}
	}

	#[test]
	fn test_diff_strategy_from_spec_explicit_diff_strategy() {
		let version = make_version("1", "28");

		// Explicit diffStrategy takes precedence over everything
		assert_eq!(
			DiffStrategy::from_spec(&make_spec_for_strategy(Some("native"), None), &version),
			DiffStrategy::Native
		);
		assert_eq!(
			DiffStrategy::from_spec(&make_spec_for_strategy(Some("server"), None), &version),
			DiffStrategy::Server
		);
		assert_eq!(
			DiffStrategy::from_spec(&make_spec_for_strategy(Some("validate"), None), &version),
			DiffStrategy::Validate
		);
		assert_eq!(
			DiffStrategy::from_spec(&make_spec_for_strategy(Some("subset"), None), &version),
			DiffStrategy::Subset
		);

		// Explicit diffStrategy overrides applyStrategy
		assert_eq!(
			DiffStrategy::from_spec(
				&make_spec_for_strategy(Some("native"), Some("server")),
				&version
			),
			DiffStrategy::Native
		);

		// Explicit diffStrategy overrides version-based selection
		let old_version = make_version("1", "10");
		assert_eq!(
			DiffStrategy::from_spec(&make_spec_for_strategy(Some("native"), None), &old_version),
			DiffStrategy::Native
		);
	}

	#[test]
	fn test_diff_strategy_from_spec_server_apply_strategy() {
		let spec = make_spec_for_strategy(None, Some("server"));
		let version = make_version("1", "28");
		assert_eq!(
			DiffStrategy::from_spec(&spec, &version),
			DiffStrategy::Server
		);

		// Server strategy overrides even on old k8s
		let old_version = make_version("1", "10");
		assert_eq!(
			DiffStrategy::from_spec(&spec, &old_version),
			DiffStrategy::Server
		);
	}

	#[test]
	fn test_diff_strategy_from_spec_version_threshold() {
		let spec = make_spec_for_strategy(None, None);

		// k8s 1.13+ gets Native
		assert_eq!(
			DiffStrategy::from_spec(&spec, &make_version("1", "13")),
			DiffStrategy::Native
		);
		assert_eq!(
			DiffStrategy::from_spec(&spec, &make_version("1", "13+")),
			DiffStrategy::Native
		);
		assert_eq!(
			DiffStrategy::from_spec(&spec, &make_version("1", "28")),
			DiffStrategy::Native
		);

		// k8s < 1.13 gets Subset
		assert_eq!(
			DiffStrategy::from_spec(&spec, &make_version("1", "12")),
			DiffStrategy::Subset
		);
		assert_eq!(
			DiffStrategy::from_spec(&spec, &make_version("1", "10")),
			DiffStrategy::Subset
		);
	}
}
