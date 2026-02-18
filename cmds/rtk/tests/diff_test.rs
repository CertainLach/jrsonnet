//! Integration tests for the diff command using a mock Kubernetes API server.
//!
//! These tests call the actual `diff_environment` entrypoint with a mock
//! ClusterConnection, testing the full diff flow from Jsonnet evaluation through
//! to cluster comparison.

#[path = "test_utils.rs"]
mod test_utils;

use k8s_mock::{DiscoveryMode, HttpMockK8sServer};
use rtk::{
	commands::diff::{diff_environment, ColorMode, DiffOpts},
	k8s::client::ClusterConnection,
	spec::{DiffStrategy, Spec},
};

/// Run a diff test with custom options.
///
/// Test structure:
/// - `environment/` - Tanka environment directory
///   - `main.jsonnet` - environment producing manifests
///   - `jsonnetfile.json` - marks the project root
/// - `cluster/` - YAML files representing current cluster state (may include CRDs)
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
	let cluster_state = test_utils::load_manifests_from_dir(&cluster_dir);
	let (_server, connection) =
		test_utils::setup_connection_from_cluster_state(cluster_state, discovery_mode, true).await;

	// Capture diff output to a buffer
	let mut output = Vec::new();

	// Ensure color is disabled for consistent output
	let opts = DiffOpts {
		color: ColorMode::Never,
		..opts
	};

	// Call the diff_environment entrypoint (evaluates Jsonnet and diffs)
	diff_environment(
		env_dir.to_str().unwrap(),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await
	.expect("diff_environment failed");

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
	use assert_matches::assert_matches;
	use k8s_mock::DiscoveryMode;
	use rtk::k8s::{
		diff::{DiffEngine, DiffError},
		discovery::DiscoveryError,
	};

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

	/// Test that an unknown resource type fails API cache construction.
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

		let err = DiffEngine::new(
			connection,
			DiffStrategy::Native,
			"default".to_string(),
			&manifests,
			false,
		)
		.await
		.err()
		.expect("expected unknown resource type to fail during engine creation");

		assert_matches!(
			err,
			DiffError::BuildingApiCache(source)
				if matches!(
					*source,
					DiscoveryError::ResourceDiscovery {
						ref api_version,
						ref kind,
						..
					} if api_version == "custom.example.com/v1" && kind == "UnknownResource"
				)
		);
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
					let test_dir = test_utils::diff_fixture_dir(stringify!($name));
					run_diff_test_with_opts(&test_dir, DiscoveryMode::Aggregated, $opts).await;
				}

				#[tokio::test]
				async fn [<$name _legacy_discovery>]() {
					let test_dir = test_utils::diff_fixture_dir(stringify!($name));
					run_diff_test_with_opts(&test_dir, DiscoveryMode::Legacy, $opts).await;
				}
			}
		};
	}

	// Basic tests with default options
	diff_test!(cluster_scoped);
	diff_test!(configmap_modified);
	diff_test!(configmap_unchanged);
	diff_test!(deployment_modified);
	diff_test!(deployment_nested_changes);
	diff_test!(inject_labels);
	diff_test!(multiple_namespaces);
	diff_test!(multi_resource);
	diff_test!(namespace_not_exists);
	diff_test!(implicit_namespace);
	diff_test!(no_last_applied);
	diff_test!(non_inline_env);

	// Strategic merge patch tests
	diff_test!(strategic_merge_delete);
	diff_test!(strategic_merge_sidecar);
	diff_test!(delete_from_primitive_list);
	diff_test!(retain_keys_volumes);
	diff_test!(service_composite_key);

	// Pruning tests
	diff_test!(resource_deleted, {
		DiffOpts::builder().with_prune(true).build()
	});
	diff_test!(prune_labeled_only, {
		DiffOpts::builder().with_prune(true).build()
	});
	diff_test!(implicit_namespace_prune, {
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

	// CRD tests - custom merge keys from OpenAPI schemas
	diff_test!(crd_merge_keys);
}

/// Tests for CRD support with custom merge keys via OpenAPI schemas.
#[cfg(test)]
mod crd_tests {
	use std::collections::HashMap;

	use indoc::indoc;
	use k8s_mock::{DiscoveryMode, HttpMockK8sServer, MockApiResource, MockDiscovery};
	use rtk::{
		k8s::{client::ClusterConnection, diff::DiffEngine},
		spec::{DiffStrategy, Spec},
	};

	/// Test that CRD merge keys from OpenAPI schemas are respected during diff.
	///
	/// This test verifies that when a CRD has custom merge keys (x-kubernetes-list-map-keys)
	/// in its OpenAPI schema, the diff engine correctly uses them for strategic merge.
	#[tokio::test]
	async fn test_crd_merge_keys_from_openapi() {
		// Define a CRD OpenAPI schema with custom merge keys
		let crd_schema = serde_json::json!({
			"openapi": "3.0.0",
			"info": { "title": "Example CRD API", "version": "v1" },
			"components": {
				"schemas": {
					"com.example.v1.DatabaseCluster": {
						"properties": {
							"spec": {
								"$ref": "#/components/schemas/com.example.v1.DatabaseClusterSpec"
							}
						}
					},
					"com.example.v1.DatabaseClusterSpec": {
						"properties": {
							// This is the key field: a list with custom merge keys
							"instances": {
								"type": "array",
								"x-kubernetes-list-map-keys": ["name"],
								"x-kubernetes-list-type": "map",
								"items": {
									"$ref": "#/components/schemas/com.example.v1.Instance"
								}
							}
						}
					},
					"com.example.v1.Instance": {
						"properties": {
							"name": { "type": "string" },
							"replicas": { "type": "integer" },
							"storage": { "type": "string" }
						}
					}
				}
			}
		});

		let mut openapi_schemas = HashMap::new();
		openapi_schemas.insert("apis/example.com/v1".to_string(), crd_schema);

		// Create discovery with the CRD type
		let discovery = MockDiscovery::default().with_group(
			"example.com/v1",
			vec![MockApiResource::namespaced(
				"databaseclusters",
				"DatabaseCluster",
			)],
		);

		// Current cluster state: CRD with two instances
		let cluster_state = vec![serde_json::json!({
			"apiVersion": "example.com/v1",
			"kind": "DatabaseCluster",
			"metadata": {
				"name": "my-db",
				"namespace": "default"
			},
			"spec": {
				"instances": [
					{ "name": "primary", "replicas": 1, "storage": "10Gi" },
					{ "name": "replica", "replicas": 2, "storage": "10Gi" }
				]
			}
		})];

		// Start mock server with CRD support
		let server = HttpMockK8sServer::builder()
			.discovery_mode(DiscoveryMode::Aggregated)
			.discovery(discovery)
			.openapi_schemas(openapi_schemas)
			.resources(cluster_state)
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

		// Desired state: modify only the replica instance's replicas field
		// The merge key "name" should preserve the primary instance unchanged
		let manifests = vec![serde_json::json!({
			"apiVersion": "example.com/v1",
			"kind": "DatabaseCluster",
			"metadata": {
				"name": "my-db",
				"namespace": "default"
			},
			"spec": {
				"instances": [
					{ "name": "primary", "replicas": 1, "storage": "10Gi" },
					{ "name": "replica", "replicas": 3, "storage": "20Gi" }  // Changed: replicas 2->3, storage 10Gi->20Gi
				]
			}
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

		let diffs = engine
			.diff_all(&manifests, false, None, false)
			.await
			.expect("diff failed");

		// Get the unified diff output
		assert_eq!(diffs.len(), 1, "expected exactly one diff");
		let diff = &diffs[0];
		let diff_str = diff.unified_diff(DiffStrategy::Native);

		// Verify the exact diff output showing merge key-aware changes
		// The diff shows only the replica instance's changed fields (replicas: 2->3, storage: 10Gi->20Gi)
		// while the primary instance remains in context but unchanged
		let expected = indoc! {"
			--- a/example.com.v1.DatabaseCluster.default.my-db
			+++ b/example.com.v1.DatabaseCluster.default.my-db
			@@ -9,5 +9,5 @@
			     replicas: 1
			     storage: 10Gi
			   - name: replica
			-    replicas: 2
			-    storage: 10Gi
			+    replicas: 3
			+    storage: 20Gi
		"};
		assert_eq!(diff_str, expected);
	}
}
