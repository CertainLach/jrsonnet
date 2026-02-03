//! Kubernetes API resource discovery and caching.
//!
//! This module handles discovering API resources from the cluster's
//! discovery API, caching the results for efficient lookups by
//! apiVersion and kind.

use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use kube::{
	core::GroupVersionKind,
	discovery::{oneshot::pinned_kind, ApiCapabilities, ApiResource, Scope},
	Client, Discovery,
};
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::instrument;

use super::ResourceScope;

/// Errors that can occur during API resource discovery.
#[derive(Debug, Error)]
pub enum DiscoveryError {
	#[error("full API discovery failed")]
	FullDiscovery(#[source] kube::Error),

	#[error("discovery task panicked")]
	TaskPanicked(#[source] tokio::task::JoinError),

	#[error("failed to discover resource {api_version}/{kind}")]
	ResourceDiscovery {
		api_version: String,
		kind: String,
		#[source]
		source: kube::Error,
	},
}

/// Extract a GroupVersionKind from a Kubernetes manifest.
pub fn gvk_from_manifest(manifest: &serde_json::Value) -> Option<GroupVersionKind> {
	let api_version = manifest.get("apiVersion")?.as_str()?;
	let kind = manifest.get("kind")?.as_str()?;
	gvk_from_api_version(api_version, kind)
}

/// Create a GroupVersionKind from an apiVersion string and kind.
fn gvk_from_api_version(api_version: &str, kind: &str) -> Option<GroupVersionKind> {
	let (group, version) = match api_version.split_once('/') {
		Some((g, v)) => (g, v),
		None => ("", api_version),
	};
	Some(GroupVersionKind::gvk(group, version, kind))
}

/// Create a GroupVersionKind from a kube ApiResource.
fn gvk_from_api_resource(ar: &ApiResource) -> GroupVersionKind {
	GroupVersionKind::gvk(&ar.group, &ar.version, &ar.kind)
}

/// Discovered API resource with scope and capabilities.
#[derive(Debug, Clone)]
pub struct DiscoveredResource {
	/// The kube ApiResource for making API calls.
	pub api_resource: ApiResource,
	/// Whether this resource is namespaced or cluster-wide.
	pub scope: ResourceScope,
	/// API capabilities (verbs, subresources, etc.)
	pub capabilities: ApiCapabilities,
}

/// Cached API resource discovery results.
///
/// This cache is built by querying the cluster's discovery API once,
/// then provides O(1) lookups for resources by apiVersion and kind.
#[derive(Clone)]
pub struct ApiResourceCache {
	resources: HashMap<GroupVersionKind, DiscoveredResource>,
}

impl ApiResourceCache {
	/// Maximum concurrent discovery requests for lazy fallback.
	const MAX_CONCURRENT_DISCOVERIES: usize = 8;

	/// Build the cache by querying the cluster's discovery API.
	///
	/// Uses the Aggregated Discovery API (K8s 1.26+) which requires only 2 API calls.
	/// Falls back to lazy discovery of only the specified keys for older clusters,
	/// unless `need_full_discovery` is true (e.g., for prune detection).
	///
	/// # Arguments
	/// * `client` - Kubernetes client
	/// * `required_keys` - Resource types needed (used for lazy fallback)
	/// * `need_full_discovery` - If true, fallback uses full discovery instead of lazy.
	///   Required for prune detection to find orphaned resources of any type.
	#[instrument(skip(client, required_keys), fields(key_count = required_keys.len(), need_full = need_full_discovery))]
	pub async fn build(
		client: &Client,
		required_keys: HashSet<GroupVersionKind>,
		need_full_discovery: bool,
	) -> Result<Self, DiscoveryError> {
		// Try aggregated discovery first (2 API calls, K8s 1.26+)
		match Discovery::new(client.clone()).run_aggregated().await {
			Ok(discovery) => {
				tracing::debug!("using aggregated discovery");
				Ok(Self::from_discovery(discovery))
			}
			Err(e) => {
				tracing::debug!(error = %e, "aggregated discovery not available");
				if need_full_discovery {
					tracing::debug!("using full discovery for prune support");
					Self::build_full(client).await
				} else {
					tracing::debug!("using lazy discovery");
					Self::build_lazy(client, required_keys).await
				}
			}
		}
	}

	/// Build cache using full discovery (N+2 API calls).
	///
	/// This is slower than aggregated discovery but works on older clusters
	/// and discovers all resource types (needed for prune detection).
	#[instrument(skip(client))]
	async fn build_full(client: &Client) -> Result<Self, DiscoveryError> {
		let discovery = Discovery::new(client.clone())
			.run()
			.await
			.map_err(DiscoveryError::FullDiscovery)?;
		Ok(Self::from_discovery(discovery))
	}

	/// Build cache from a completed Discovery.
	fn from_discovery(discovery: Discovery) -> Self {
		let mut resources = HashMap::new();

		for group in discovery.groups() {
			// Iterate all versions, not just recommended, so we can handle
			// manifests using older API versions (e.g., v1alpha1 vs v1beta1)
			for ver in group.versions() {
				for (ar, caps) in group.versioned_resources(ver) {
					let gvk = gvk_from_api_resource(&ar);

					let scope = match caps.scope {
						Scope::Namespaced => ResourceScope::Namespaced,
						Scope::Cluster => ResourceScope::ClusterWide,
					};

					resources.insert(
						gvk,
						DiscoveredResource {
							api_resource: ar,
							scope,
							capabilities: caps,
						},
					);
				}
			}
		}

		Self { resources }
	}

	/// Build cache lazily by discovering only the specified resource keys.
	///
	/// Uses bounded parallelism to discover multiple resources concurrently.
	#[instrument(skip(client, keys), fields(key_count = keys.len()))]
	async fn build_lazy(
		client: &Client,
		keys: HashSet<GroupVersionKind>,
	) -> Result<Self, DiscoveryError> {
		use tokio::task::JoinSet;

		let semaphore = Arc::new(Semaphore::new(Self::MAX_CONCURRENT_DISCOVERIES));
		let mut join_set = JoinSet::new();

		for gvk in keys {
			let client = client.clone();
			let sem = semaphore.clone();

			join_set.spawn(async move {
				let _permit = sem.acquire().await.expect("semaphore closed");

				tracing::debug!(
					api_version = %gvk.api_version(),
					kind = %gvk.kind,
					"discovering resource"
				);

				match pinned_kind(&client, &gvk).await {
					Ok((api_resource, capabilities)) => {
						let scope = match capabilities.scope {
							Scope::Namespaced => ResourceScope::Namespaced,
							Scope::Cluster => ResourceScope::ClusterWide,
						};
						Ok((
							gvk,
							DiscoveredResource {
								api_resource,
								scope,
								capabilities,
							},
						))
					}
					Err(e) => Err((gvk, e)),
				}
			});
		}

		let mut resources = HashMap::new();
		let mut errors = Vec::new();

		while let Some(result) = join_set.join_next().await {
			match result.map_err(DiscoveryError::TaskPanicked)? {
				Ok((gvk, discovered)) => {
					resources.insert(gvk, discovered);
				}
				Err((gvk, e)) => {
					tracing::warn!(
						api_version = %gvk.api_version(),
						kind = %gvk.kind,
						error = %e,
						"failed to discover resource"
					);
					errors.push((gvk, e));
				}
			}
		}

		// If all discoveries failed, return an error
		if resources.is_empty() && !errors.is_empty() {
			let (gvk, e) = errors.remove(0);
			return Err(DiscoveryError::ResourceDiscovery {
				api_version: gvk.api_version(),
				kind: gvk.kind,
				source: e,
			});
		}

		Ok(Self { resources })
	}

	/// Look up a resource by its GroupVersionKind.
	pub fn lookup(&self, gvk: &GroupVersionKind) -> Option<&DiscoveredResource> {
		self.resources.get(gvk)
	}

	/// Iterate over all cached resources.
	pub fn iter(&self) -> impl Iterator<Item = (&GroupVersionKind, &DiscoveredResource)> {
		self.resources.iter()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_gvk_from_manifest() {
		let manifest = serde_json::json!({
			"apiVersion": "apps/v1",
			"kind": "Deployment",
			"metadata": {
				"name": "test"
			}
		});

		let gvk = gvk_from_manifest(&manifest).unwrap();
		assert_eq!(gvk.group, "apps");
		assert_eq!(gvk.version, "v1");
		assert_eq!(gvk.kind, "Deployment");
		assert_eq!(gvk.api_version(), "apps/v1");
	}

	#[test]
	fn test_gvk_from_manifest_core_api() {
		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test"
			}
		});

		let gvk = gvk_from_manifest(&manifest).unwrap();
		assert_eq!(gvk.group, "");
		assert_eq!(gvk.version, "v1");
		assert_eq!(gvk.kind, "ConfigMap");
		assert_eq!(gvk.api_version(), "v1");
	}

	#[test]
	fn test_gvk_from_manifest_missing_fields() {
		let manifest = serde_json::json!({
			"kind": "Deployment"
		});

		assert!(gvk_from_manifest(&manifest).is_none());
	}

	#[test]
	fn test_gvk_equality() {
		let gvk1 = GroupVersionKind::gvk("", "v1", "ConfigMap");
		let gvk2 = GroupVersionKind::gvk("", "v1", "ConfigMap");
		let gvk3 = GroupVersionKind::gvk("", "v1", "Secret");

		assert_eq!(gvk1, gvk2);
		assert_ne!(gvk1, gvk3);
	}
}
