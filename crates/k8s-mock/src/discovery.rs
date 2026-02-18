//! Mock Kubernetes API discovery types.

use std::collections::HashMap;

/// Discovery mode for the mock server.
#[derive(Clone, Copy, Default)]
pub enum DiscoveryMode {
	/// Support aggregated discovery (APIGroupDiscoveryList).
	#[default]
	Aggregated,
	/// Return 406 for aggregated discovery, forcing fallback to legacy endpoints.
	Legacy,
}

/// Pre-configured discovery responses.
#[derive(Clone)]
pub struct MockDiscovery {
	pub core_resources: Vec<MockApiResource>,
	pub group_resources: HashMap<String, Vec<MockApiResource>>,
}

impl MockDiscovery {
	/// Add a custom API group with resources (e.g., for CRDs).
	pub fn with_group(mut self, group_version: &str, resources: Vec<MockApiResource>) -> Self {
		self.group_resources
			.insert(group_version.to_string(), resources);
		self
	}
}

impl Default for MockDiscovery {
	fn default() -> Self {
		Self {
			core_resources: vec![
				MockApiResource::namespaced("configmaps", "ConfigMap"),
				MockApiResource::namespaced("secrets", "Secret"),
				MockApiResource::namespaced("services", "Service"),
				MockApiResource::namespaced("pods", "Pod"),
				MockApiResource::cluster_scoped("namespaces", "Namespace"),
			],
			group_resources: HashMap::from([(
				"apps/v1".to_string(),
				vec![
					MockApiResource::namespaced("deployments", "Deployment"),
					MockApiResource::namespaced("statefulsets", "StatefulSet"),
					MockApiResource::namespaced("daemonsets", "DaemonSet"),
				],
			)]),
		}
	}
}

/// A mock API resource definition.
#[derive(Clone)]
pub struct MockApiResource {
	pub name: String,
	pub kind: String,
	pub namespaced: bool,
	pub verbs: Vec<String>,
}

impl MockApiResource {
	pub fn namespaced(name: &str, kind: &str) -> Self {
		Self {
			name: name.to_string(),
			kind: kind.to_string(),
			namespaced: true,
			verbs: vec![
				"create".into(),
				"delete".into(),
				"get".into(),
				"list".into(),
				"patch".into(),
				"update".into(),
				"watch".into(),
			],
		}
	}

	pub fn cluster_scoped(name: &str, kind: &str) -> Self {
		Self {
			name: name.to_string(),
			kind: kind.to_string(),
			namespaced: false,
			verbs: vec![
				"create".into(),
				"delete".into(),
				"get".into(),
				"list".into(),
				"patch".into(),
				"update".into(),
				"watch".into(),
			],
		}
	}
}
