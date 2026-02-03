//! Integration tests for the diff command using a mock Kubernetes API server.
//!
//! These tests call the actual `diff_environment` entrypoint with a mock
//! ClusterConnection, testing the full diff flow from Jsonnet evaluation through
//! to cluster comparison.

use k8s_mock::{DiscoveryMode, HttpMockK8sServer};
use rtk::{
	commands::diff::{diff_environment, ColorMode, DiffOpts},
	k8s::client::ClusterConnection,
	spec::{DiffStrategy, Spec},
};

/// Load manifests from YAML files in a directory.
fn load_manifests_from_dir(dir: &std::path::Path) -> Vec<serde_json::Value> {
	let mut manifests = Vec::new();
	if !dir.exists() {
		return manifests;
	}

	let mut entries: Vec<_> = std::fs::read_dir(dir)
		.expect("failed to read dir")
		.filter_map(|e| e.ok())
		.filter(|e| {
			e.path()
				.extension()
				.map(|ext| ext == "yaml" || ext == "yml")
				.unwrap_or(false)
		})
		.collect();
	entries.sort_by_key(|e| e.path());

	for entry in entries {
		let content = std::fs::read_to_string(entry.path()).expect("failed to read file");
		let value: serde_json::Value =
			serde_yaml::from_str(&content).expect("failed to parse YAML");
		manifests.push(value);
	}
	manifests
}

/// Run a diff test with custom options.
///
/// Test structure:
/// - `environment/` - Tanka environment directory
///   - `main.jsonnet` - environment producing manifests
///   - `jsonnetfile.json` - marks the project root
/// - `cluster/` - YAML files representing current cluster state
/// - `expected.diff` - expected unified diff output (for normal diff mode)
/// - `expected.txt` - expected text output (for summarize mode, takes precedence)
async fn run_diff_test_with_opts(
	test_dir: &std::path::Path,
	discovery_mode: DiscoveryMode,
	opts: DiffOpts,
) {
	let env_dir = test_dir.join("environment");
	let cluster_dir = test_dir.join("cluster");

	// Load cluster state as manifests
	let cluster_state = load_manifests_from_dir(&cluster_dir);

	// Start mock server with cluster state
	let server = HttpMockK8sServer::builder()
		.discovery_mode(discovery_mode)
		.resources(cluster_state)
		.build()
		.start()
		.await;

	// Create connection using the mock server's kubeconfig
	let spec = Spec {
		context_names: Some(vec!["mock-context".to_string()]),
		..Spec::default()
	};
	let connection = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
		.await
		.expect("failed to create connection");

	// Capture diff output to a buffer
	let mut output = Vec::new();

	// Ensure color is disabled for consistent output
	let opts = DiffOpts {
		color: ColorMode::Never,
		..opts
	};

	// Call the diff_environment entrypoint (evaluates Jsonnet and diffs)
	let result = diff_environment(
		env_dir.to_str().unwrap(),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await;

	let _diffs = result.expect("diff failed");

	// Compare output against expected file (expected.diff or expected.txt)
	let actual = String::from_utf8(output).expect("diff output should be valid UTF-8");
	let expected_diff = test_dir.join("expected.diff");
	let expected_txt = test_dir.join("expected.txt");
	let expected_path = if expected_txt.exists() {
		expected_txt
	} else {
		expected_diff
	};
	let expected = std::fs::read_to_string(&expected_path)
		.unwrap_or_else(|_| panic!("failed to read {}", expected_path.display()));

	assert_eq!(actual, expected, "diff output mismatch");
}

#[cfg(test)]
mod error_tests {
	use k8s_mock::DiscoveryMode;
	use rtk::k8s::diff::{DiffEngine, DiffError};

	use super::*;

	/// Test that diffing a manifest missing apiVersion/kind returns an error.
	#[tokio::test]
	async fn test_diff_malformed_manifest_missing_api_version() {
		let server = HttpMockK8sServer::builder()
			.discovery_mode(DiscoveryMode::Aggregated)
			.build()
			.start()
			.await;

		let spec = Spec {
			context_names: Some(vec!["mock-context".to_string()]),
			..Spec::default()
		};
		let connection = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
			.await
			.expect("failed to create connection");

		let manifests = vec![serde_json::json!({
			"kind": "ConfigMap",
			"metadata": { "name": "test" }
		})];

		let engine = DiffEngine::new(
			connection,
			DiffStrategy::Native,
			"default".to_string(),
			&manifests,
			false,
		)
		.await
		.expect("failed to create engine");

		let result = engine.diff_manifest(&manifests[0]).await;
		assert!(matches!(result, Err(DiffError::MissingApiVersionOrKind)));
	}

	/// Test that diffing a manifest missing metadata.name returns an error.
	#[tokio::test]
	async fn test_diff_malformed_manifest_missing_name() {
		let server = HttpMockK8sServer::builder()
			.discovery_mode(DiscoveryMode::Aggregated)
			.build()
			.start()
			.await;

		let spec = Spec {
			context_names: Some(vec!["mock-context".to_string()]),
			..Spec::default()
		};
		let connection = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
			.await
			.expect("failed to create connection");

		let manifests = vec![serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {}
		})];

		let engine = DiffEngine::new(
			connection,
			DiffStrategy::Native,
			"default".to_string(),
			&manifests,
			false,
		)
		.await
		.expect("failed to create engine");

		let result = engine.diff_manifest(&manifests[0]).await;
		assert!(matches!(result, Err(DiffError::MissingName)));
	}

	/// Test that diffing a manifest with an unknown resource type returns an error.
	#[tokio::test]
	async fn test_diff_unknown_resource_type() {
		let server = HttpMockK8sServer::builder()
			.discovery_mode(DiscoveryMode::Aggregated)
			.build()
			.start()
			.await;

		let spec = Spec {
			context_names: Some(vec!["mock-context".to_string()]),
			..Spec::default()
		};
		let connection = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
			.await
			.expect("failed to create connection");

		// Use a CRD that doesn't exist in discovery
		let manifests = vec![serde_json::json!({
			"apiVersion": "custom.example.com/v1",
			"kind": "UnknownResource",
			"metadata": { "name": "test" }
		})];

		let engine = DiffEngine::new(
			connection,
			DiffStrategy::Native,
			"default".to_string(),
			&manifests,
			false,
		)
		.await
		.expect("failed to create engine");

		let result = engine.diff_manifest(&manifests[0]).await;
		assert!(matches!(result, Err(DiffError::UnknownResourceType { .. })));
	}

	/// Test that prune without injectLabels returns an error.
	#[tokio::test]
	async fn test_diff_prune_requires_inject_labels() {
		let server = HttpMockK8sServer::builder()
			.discovery_mode(DiscoveryMode::Aggregated)
			.build()
			.start()
			.await;

		let spec = Spec {
			context_names: Some(vec!["mock-context".to_string()]),
			..Spec::default()
		};
		let connection = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
			.await
			.expect("failed to create connection");

		let manifests = vec![serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": { "name": "test", "namespace": "default" },
			"data": {}
		})];

		let engine = DiffEngine::new(
			connection,
			DiffStrategy::Native,
			"default".to_string(),
			&manifests,
			true, // with_prune
		)
		.await
		.expect("failed to create engine");

		// Call diff_all with prune enabled but injectLabels=false
		let result = engine
			.diff_all(&manifests, true, Some("test-label"), false)
			.await;
		assert!(matches!(result, Err(DiffError::PruneRequiresInjectLabels)));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Generate diff tests for both aggregated and legacy discovery modes.
	///
	/// Usage:
	/// - `diff_test!(test_name)` - run with default DiffOpts
	/// - `diff_test!(test_name, { opts })` - run with custom DiffOpts expression
	macro_rules! diff_test {
		($name:ident) => {
			diff_test!($name, { DiffOpts::default() });
		};
		($name:ident, { $opts:expr }) => {
			paste::paste! {
				#[tokio::test]
				async fn [<$name _aggregated_discovery>]() {
					let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
						.join("tests/testdata/diff")
						.join(stringify!($name));
					run_diff_test_with_opts(&test_dir, DiscoveryMode::Aggregated, $opts).await;
				}

				#[tokio::test]
				async fn [<$name _legacy_discovery>]() {
					let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
						.join("tests/testdata/diff")
						.join(stringify!($name));
					run_diff_test_with_opts(&test_dir, DiscoveryMode::Legacy, $opts).await;
				}
			}
		};
	}

	// Basic tests with default options
	diff_test!(configmap_unchanged);
	diff_test!(configmap_modified);
	diff_test!(multi_resource);
	diff_test!(multiple_namespaces);
	diff_test!(non_inline_env);
	diff_test!(deployment_modified);
	diff_test!(deployment_nested_changes);
	diff_test!(cluster_scoped);
	diff_test!(namespace_not_exists);
	diff_test!(inject_labels);

	// Pruning tests
	diff_test!(resource_deleted, {
		DiffOpts::builder().with_prune(true).build()
	});
	diff_test!(prune_labeled_only, {
		DiffOpts::builder().with_prune(true).build()
	});

	// Name filtering test
	diff_test!(multi_inline_env, {
		DiffOpts::builder().name("env-a".to_string()).build()
	});

	// Target filtering test
	diff_test!(target_filter, {
		DiffOpts::builder()
			.target(vec!["ConfigMap/.*".to_string()])
			.build()
	});

	// Strategy tests
	diff_test!(strategy_subset, {
		DiffOpts::builder().strategy(DiffStrategy::Subset).build()
	});
	diff_test!(strategy_server, {
		DiffOpts::builder().strategy(DiffStrategy::Server).build()
	});
	diff_test!(strategy_validate, {
		DiffOpts::builder().strategy(DiffStrategy::Validate).build()
	});

	// Summarize mode test
	diff_test!(summarize, { DiffOpts::builder().summarize(true).build() });
}
