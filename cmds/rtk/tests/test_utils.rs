use std::path::{Path, PathBuf};

use k8s_mock::{extract_crd_metadata, DiscoveryMode, HttpMockK8sServer, RunningHttpMockK8sServer};
use rtk::{k8s::client::ClusterConnection, spec::Spec};

/// Load manifests from YAML files in a directory.
pub fn load_manifests_from_dir(dir: &Path) -> Vec<serde_json::Value> {
	if !dir.exists() {
		return Vec::new();
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

	entries
		.into_iter()
		.map(|entry| std::fs::read_to_string(entry.path()).expect("failed to read file"))
		.map(|content| serde_yaml::from_str(&content).expect("failed to parse YAML"))
		.collect()
}

pub fn diff_fixture_dir(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/testdata/diff")
		.join(name)
}

/// Start a mock server and return a ClusterConnection for a fixture's cluster state.
///
/// If `include_crd_metadata` is true, CRDs are extracted from cluster manifests and
/// wired into discovery/OpenAPI responses for schema-aware diffing.
pub async fn setup_connection_from_cluster_state(
	cluster_state: Vec<serde_json::Value>,
	discovery_mode: DiscoveryMode,
	include_crd_metadata: bool,
) -> (RunningHttpMockK8sServer, ClusterConnection) {
	let server = if include_crd_metadata {
		let (crd_discovery, openapi_schemas) = extract_crd_metadata(&cluster_state);
		HttpMockK8sServer::builder()
			.discovery_mode(discovery_mode)
			.discovery(crd_discovery)
			.openapi_schemas(openapi_schemas)
			.resources(cluster_state)
			.build()
			.start()
			.await
	} else {
		HttpMockK8sServer::builder()
			.discovery_mode(discovery_mode)
			.resources(cluster_state)
			.build()
			.start()
			.await
	};

	let spec = Spec {
		context_names: Some(vec!["mock-context".to_string()]),
		..Spec::default()
	};
	let connection = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
		.await
		.expect("failed to create connection");

	(server, connection)
}
