//! Mock Kubernetes server integration for diff command testing.
//!
//! This module provides functionality to start a mock K8s server with
//! pre-loaded cluster state for comparing `tk diff` and `rtk diff` output.
//!
//! CRD Support:
//! When the cluster directory contains CustomResourceDefinition manifests,
//! they are automatically processed to:
//! 1. Add the CRD types to API discovery
//! 2. Generate OpenAPI v3 endpoints from the CRD's openAPIV3Schema

use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use k8s_mock::{
	extract_crd_metadata, DiscoveryMode, HttpExchange, HttpMockK8sServer, RunningHttpMockK8sServer,
};

/// A running mock K8s server with its kubeconfig.
pub struct MockCluster {
	/// The running mock server (kept alive for the duration of the test)
	_server: RunningHttpMockK8sServer,
	/// Path to the kubeconfig file
	pub kubeconfig_path: String,
}

impl MockCluster {
	/// Start a mock K8s server with cluster state loaded from a directory.
	///
	/// The directory should contain YAML files representing the current cluster state.
	/// CRDs are automatically detected and used to configure discovery and OpenAPI endpoints.
	pub async fn start(cluster_dir: &str, discovery_mode: DiscoveryMode) -> Result<Self> {
		let cluster_path = Path::new(cluster_dir);

		// Load manifests from the cluster directory
		let manifests = load_manifests_from_dir(cluster_path)?;

		// Extract CRDs and generate discovery/OpenAPI from them
		let (discovery, openapi_schemas) = extract_crd_metadata(&manifests);

		// Start the mock server
		let server = HttpMockK8sServer::builder()
			.discovery_mode(discovery_mode)
			.discovery(discovery)
			.openapi_schemas(openapi_schemas)
			.resources(manifests)
			.build()
			.start()
			.await;

		// Generate kubeconfig
		let kubeconfig = server.kubeconfig_with_context("mock-context");
		let kubeconfig_yaml =
			serde_yaml::to_string(&kubeconfig).context("failed to serialize kubeconfig")?;

		// Write kubeconfig to a temporary file
		let kubeconfig_path = format!("/tmp/tk-compare-kubeconfig-{}.yaml", std::process::id());
		fs::write(&kubeconfig_path, &kubeconfig_yaml)
			.with_context(|| format!("failed to write kubeconfig to {}", kubeconfig_path))?;

		Ok(MockCluster {
			_server: server,
			kubeconfig_path,
		})
	}

	/// Get environment variables to pass to subprocesses.
	pub fn env_vars(&self) -> HashMap<String, String> {
		let mut vars = HashMap::new();
		vars.insert("KUBECONFIG".to_string(), self.kubeconfig_path.clone());
		vars
	}

	/// Captured HTTP request/response exchanges from the mock server.
	pub fn http_exchanges(&self) -> Vec<HttpExchange> {
		self._server.http_exchanges()
	}
}

impl Drop for MockCluster {
	fn drop(&mut self) {
		// Clean up the kubeconfig file
		let _ = fs::remove_file(&self.kubeconfig_path);
	}
}

/// Load YAML manifests from a directory.
fn load_manifests_from_dir(dir: &Path) -> Result<Vec<serde_json::Value>> {
	use serde::Deserialize;

	let mut manifests = Vec::new();

	if !dir.exists() {
		return Ok(manifests);
	}

	let mut entries: Vec<_> = fs::read_dir(dir)
		.with_context(|| format!("failed to read directory {}", dir.display()))?
		.filter_map(|e| e.ok())
		.filter(|e| {
			let path = e.path();
			// Only include YAML files directly in dir (not subdirectories)
			path.is_file()
				&& path
					.extension()
					.map(|ext| ext == "yaml" || ext == "yml")
					.unwrap_or(false)
		})
		.collect();
	entries.sort_by_key(|e| e.path());

	for entry in entries {
		let path = entry.path();
		let content = fs::read_to_string(&path)
			.with_context(|| format!("failed to read {}", path.display()))?;

		// Handle multi-document YAML files
		for doc in serde_yaml::Deserializer::from_str(&content) {
			let value = serde_json::Value::deserialize(doc)
				.with_context(|| format!("failed to parse YAML in {}", path.display()))?;

			// Skip empty documents
			if value.is_null() {
				continue;
			}

			manifests.push(value);
		}
	}

	Ok(manifests)
}
