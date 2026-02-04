//! Integration tests for the prune command using a mock Kubernetes API server.
//!
//! These tests call the actual `prune_environment` entrypoint with a mock
//! ClusterConnection, testing the full prune flow from Jsonnet evaluation through
//! to cluster deletion.

use k8s_mock::{DiscoveryMode, HttpMockK8sServer};
use rtk::{
	commands::{
		diff::ColorMode,
		prune::{prune_environment, AutoApprove, PruneOpts},
	},
	k8s::client::ClusterConnection,
	spec::Spec,
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

/// Run a prune test.
///
/// Test structure:
/// - `environment/` - Tanka environment directory
///   - `main.jsonnet` - environment producing manifests
///   - `jsonnetfile.json` - marks the project root
/// - `cluster/` - YAML files representing initial cluster state
///
/// The test:
/// 1. Prunes orphaned resources from the mock cluster
/// 2. Verifies the expected resources were deleted
async fn run_prune_test(
	test_dir: &std::path::Path,
	discovery_mode: DiscoveryMode,
	expected_deletions: &[(&str, &str)], // (kind, name) pairs
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

	// Capture output to a buffer
	let mut output = Vec::new();

	let opts = PruneOpts {
		auto_approve: AutoApprove::Always,
		color: ColorMode::Never,
		..Default::default()
	};

	let result = prune_environment(
		env_dir.to_str().unwrap(),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await;

	let diffs = result.expect("prune should succeed");

	// Verify expected deletions
	let mut actual_deletions: Vec<_> = diffs
		.iter()
		.map(|d| (d.gvk.kind.as_str(), d.name.as_str()))
		.collect();
	actual_deletions.sort();

	let mut expected: Vec<_> = expected_deletions.to_vec();
	expected.sort();

	assert_eq!(
		actual_deletions, expected,
		"deleted resources mismatch.\nExpected: {:?}\nActual: {:?}",
		expected, actual_deletions
	);
}

/// Run a prune test that expects no deletions.
async fn run_prune_no_deletions_test(test_dir: &std::path::Path, discovery_mode: DiscoveryMode) {
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

	// Capture output to a buffer
	let mut output = Vec::new();

	let opts = PruneOpts {
		auto_approve: AutoApprove::Always,
		color: ColorMode::Never,
		..Default::default()
	};

	let result = prune_environment(
		env_dir.to_str().unwrap(),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await;

	let diffs = result.expect("prune should succeed");

	assert!(
		diffs.is_empty(),
		"expected no deletions, but found: {:?}",
		diffs
			.iter()
			.map(|d| format!("{}/{}", d.gvk.kind, d.name))
			.collect::<Vec<_>>()
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Test pruning a resource that was removed from manifests.
	#[tokio::test]
	async fn resource_deleted_aggregated_discovery() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/diff")
			.join("resource_deleted");
		// The "delete-this" ConfigMap should be pruned
		run_prune_test(
			&test_dir,
			DiscoveryMode::Aggregated,
			&[("ConfigMap", "delete-this")],
		)
		.await;
	}

	#[tokio::test]
	async fn resource_deleted_legacy_discovery() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/diff")
			.join("resource_deleted");
		run_prune_test(
			&test_dir,
			DiscoveryMode::Legacy,
			&[("ConfigMap", "delete-this")],
		)
		.await;
	}

	/// Test that only resources with the tanka.dev/environment label are pruned.
	#[tokio::test]
	async fn prune_labeled_only_aggregated_discovery() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/diff")
			.join("prune_labeled_only");
		// Only "orphaned-config" has the tanka label and is not in manifests
		// "external-config" should not be pruned (no tanka label)
		run_prune_test(
			&test_dir,
			DiscoveryMode::Aggregated,
			&[("ConfigMap", "orphaned-config")],
		)
		.await;
	}

	#[tokio::test]
	async fn prune_labeled_only_legacy_discovery() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/diff")
			.join("prune_labeled_only");
		run_prune_test(
			&test_dir,
			DiscoveryMode::Legacy,
			&[("ConfigMap", "orphaned-config")],
		)
		.await;
	}

	/// Test that prune with no orphans results in no deletions.
	#[tokio::test]
	async fn prune_no_orphans_aggregated() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/diff")
			.join("inject_labels");
		// All resources in cluster are also in manifests
		run_prune_no_deletions_test(&test_dir, DiscoveryMode::Aggregated).await;
	}

	/// Test that manifests without explicit namespace match cluster resources.
	///
	/// BUG: When a manifest relies on spec.namespace (no explicit namespace),
	/// it should still match cluster resources that have the namespace set.
	/// Without the fix, the manifest key is (v1, ConfigMap, None, "keep-config")
	/// but cluster key is (v1, ConfigMap, Some("default"), "keep-config"),
	/// causing a false positive deletion.
	#[tokio::test]
	async fn prune_implicit_namespace_no_false_deletion() {
		let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/testdata/diff")
			.join("prune_implicit_namespace");
		// The resource should NOT be deleted - manifest without namespace should
		// match cluster resource with namespace
		run_prune_no_deletions_test(&test_dir, DiscoveryMode::Aggregated).await;
	}
}
