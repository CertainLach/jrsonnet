//! Integration tests for the apply command using a mock Kubernetes API server.
//!
//! These tests call the actual `apply_environment` entrypoint with a mock
//! ClusterConnection, testing the full apply flow from Jsonnet evaluation through
//! to cluster apply.

use k8s_mock::{DiscoveryMode, HttpMockK8sServer};
use rtk::{
	commands::{
		apply::{apply_environment, ApplyOpts, AutoApprove},
		diff::ColorMode,
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

/// Run an apply test.
///
/// Test structure:
/// - `environment/` - Tanka environment directory
///   - `main.jsonnet` - environment producing manifests
///   - `jsonnetfile.json` - marks the project root
/// - `cluster/` - YAML files representing initial cluster state
///
/// The test:
/// 1. Applies the environment to the mock cluster
/// 2. Verifies no error occurred
/// 3. Runs another apply and verifies no changes are detected
async fn run_apply_test(test_dir: &std::path::Path, discovery_mode: DiscoveryMode) {
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

	// First apply: should succeed (with changes)
	let opts = ApplyOpts {
		auto_approve: AutoApprove::Always,
		color: ColorMode::Never,
		..Default::default()
	};

	let result = apply_environment(
		env_dir.to_str().unwrap(),
		Some(connection.clone()),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await;

	let _diffs = result.expect("first apply should succeed");
	// First apply may or may not have changes depending on initial cluster state

	// Second apply: should show no changes
	output.clear();
	let opts = ApplyOpts {
		auto_approve: AutoApprove::Always,
		color: ColorMode::Never,
		..Default::default()
	};

	let result = apply_environment(
		env_dir.to_str().unwrap(),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await;

	let diffs = result.expect("second apply should succeed");

	// After apply, there should be no changes
	let has_changes = diffs.iter().any(|d| d.has_changes());
	assert!(
		!has_changes,
		"expected no changes after apply, but found changes: {:?}",
		diffs
			.iter()
			.filter(|d| d.has_changes())
			.map(|d| format!("{}/{}", d.gvk.kind, d.name))
			.collect::<Vec<_>>()
	);
}

/// Run an apply test that expects no changes (idempotent case).
///
/// The environment's manifests already match the cluster state.
async fn run_apply_no_changes_test(test_dir: &std::path::Path, discovery_mode: DiscoveryMode) {
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

	let opts = ApplyOpts {
		auto_approve: AutoApprove::Always,
		color: ColorMode::Never,
		..Default::default()
	};

	let result = apply_environment(
		env_dir.to_str().unwrap(),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await;

	let diffs = result.expect("apply should succeed");

	// Should have no changes since cluster matches environment
	let has_changes = diffs.iter().any(|d| d.has_changes());
	assert!(
		!has_changes,
		"expected no changes when cluster matches environment"
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Generate apply tests for both aggregated and legacy discovery modes.
	macro_rules! apply_test {
		($name:ident) => {
			paste::paste! {
				#[tokio::test]
				async fn [<$name _aggregated_discovery>]() {
					// Use diff test fixtures since they have the same structure
					let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
						.join("tests/testdata/diff")
						.join(stringify!($name));
					run_apply_test(&test_dir, DiscoveryMode::Aggregated).await;
				}

				#[tokio::test]
				async fn [<$name _legacy_discovery>]() {
					let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
						.join("tests/testdata/diff")
						.join(stringify!($name));
					run_apply_test(&test_dir, DiscoveryMode::Legacy).await;
				}
			}
		};
	}

	/// Generate apply tests that expect no changes.
	macro_rules! apply_no_changes_test {
		($name:ident) => {
			paste::paste! {
				#[tokio::test]
				async fn [<$name _no_changes_aggregated>]() {
					let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
						.join("tests/testdata/diff")
						.join(stringify!($name));
					run_apply_no_changes_test(&test_dir, DiscoveryMode::Aggregated).await;
				}
			}
		};
	}

	// Apply tests using existing diff test fixtures
	// These test that apply works and subsequent apply shows no changes
	apply_test!(configmap_modified);
	apply_test!(multi_resource);
	apply_test!(deployment_modified);
	apply_test!(deployment_nested_changes);

	// No-changes tests: cluster already matches environment
	apply_no_changes_test!(configmap_unchanged);
}
