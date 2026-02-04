//! Kubernetes resource apply engine.
//!
//! This module implements the logic for applying manifests to a cluster
//! using either client-side or server-side apply strategies.

use kube::{
	api::{Api, DynamicObject, Patch, PatchParams},
	Client,
};
use thiserror::Error;
use tracing::instrument;

use super::discovery::{gvk_from_manifest, ApiResourceCache, DiscoveredResource, DiscoveryError};

/// Errors that can occur during apply operations.
#[derive(Debug, Error)]
pub enum ApplyError {
	#[error("manifest missing apiVersion or kind")]
	MissingApiVersionOrKind,

	#[error("manifest missing metadata.name")]
	MissingName,

	#[error("unknown resource type: {api_version}/{kind}")]
	UnknownResourceType { api_version: String, kind: String },

	#[error("building API resource cache")]
	BuildingApiCache(#[source] Box<DiscoveryError>),

	#[error("applying {kind}/{name}")]
	ApplyFailed {
		kind: String,
		name: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("converting manifest to DynamicObject")]
	ManifestConversion(#[source] serde_json::Error),
}

/// Engine for applying resources to a Kubernetes cluster.
pub struct ApplyEngine {
	client: Client,
	api_cache: Option<ApiResourceCache>,
	default_namespace: String,
	server_side: bool,
	force: bool,
}

impl ApplyEngine {
	/// Create a new apply engine.
	pub fn new(client: Client, default_namespace: String, server_side: bool, force: bool) -> Self {
		Self {
			client,
			api_cache: None,
			default_namespace,
			server_side,
			force,
		}
	}

	/// Apply a single manifest to the cluster.
	#[instrument(skip(self, manifest), fields(
		kind = manifest.get("kind").and_then(|v| v.as_str()).unwrap_or("unknown"),
		name = manifest.pointer("/metadata/name").and_then(|v| v.as_str()).unwrap_or("unknown"),
	))]
	pub async fn apply_manifest(&self, manifest: &serde_json::Value) -> Result<(), ApplyError> {
		let gvk = gvk_from_manifest(manifest).ok_or(ApplyError::MissingApiVersionOrKind)?;

		let name = manifest
			.pointer("/metadata/name")
			.and_then(|v| v.as_str())
			.ok_or(ApplyError::MissingName)?
			.to_string();

		// Build API cache if not already done
		let api_cache = if let Some(ref cache) = self.api_cache {
			cache.clone()
		} else {
			let required_keys = std::iter::once(gvk.clone()).collect();
			ApiResourceCache::build(&self.client, required_keys, false)
				.await
				.map_err(|e| ApplyError::BuildingApiCache(Box::new(e)))?
		};

		let discovered = api_cache
			.lookup(&gvk)
			.ok_or_else(|| ApplyError::UnknownResourceType {
				api_version: gvk.api_version(),
				kind: gvk.kind.clone(),
			})?
			.clone();

		let namespace = self.get_namespace(manifest, &discovered);
		let api = self.dynamic_api(&discovered.api_resource, namespace.as_deref());

		// Ensure annotations exists (kubectl always includes this)
		let manifest = ensure_annotations(manifest);

		if self.server_side {
			// Server-side apply
			let patch_params = PatchParams {
				dry_run: false,
				field_manager: Some("tanka".to_string()),
				force: self.force,
				..Default::default()
			};

			api.patch(&name, &patch_params, &Patch::Apply(&manifest))
				.await
				.map_err(|e| ApplyError::ApplyFailed {
					kind: gvk.kind.clone(),
					name: name.clone(),
					source: Box::new(e),
				})?;
		} else {
			// Client-side apply using strategic merge patch
			let patch_params = PatchParams {
				dry_run: false,
				force: self.force,
				..Default::default()
			};

			// Try strategic merge patch first, fall back to merge patch for CRDs
			let result = api
				.patch(&name, &patch_params, &Patch::Strategic(&manifest))
				.await;

			match result {
				Ok(_) => {}
				Err(kube::Error::Api(ref err)) if err.code == 415 => {
					// UnsupportedMediaType - CRD doesn't support strategic merge
					api.patch(&name, &patch_params, &Patch::Merge(manifest))
						.await
						.map_err(|e| ApplyError::ApplyFailed {
							kind: gvk.kind.clone(),
							name: name.clone(),
							source: Box::new(e),
						})?;
				}
				Err(kube::Error::Api(ref err)) if err.code == 404 => {
					// Resource doesn't exist, create it
					let obj: DynamicObject = serde_json::from_value(manifest.clone())
						.map_err(ApplyError::ManifestConversion)?;

					api.create(&Default::default(), &obj).await.map_err(|e| {
						ApplyError::ApplyFailed {
							kind: gvk.kind.clone(),
							name: name.clone(),
							source: Box::new(e),
						}
					})?;
				}
				Err(e) => {
					return Err(ApplyError::ApplyFailed {
						kind: gvk.kind,
						name,
						source: Box::new(e),
					});
				}
			}
		}

		Ok(())
	}

	/// Get the namespace for a manifest.
	fn get_namespace(
		&self,
		manifest: &serde_json::Value,
		discovered: &DiscoveredResource,
	) -> Option<String> {
		use super::ResourceScope;

		match discovered.scope {
			ResourceScope::Namespaced => {
				let ns = manifest
					.pointer("/metadata/namespace")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string())
					.unwrap_or_else(|| self.default_namespace.clone());
				Some(ns)
			}
			ResourceScope::ClusterWide => None,
		}
	}

	/// Create a dynamic API for the given resource.
	fn dynamic_api(
		&self,
		ar: &kube::discovery::ApiResource,
		namespace: Option<&str>,
	) -> Api<DynamicObject> {
		match namespace {
			Some(ns) => Api::namespaced_with(self.client.clone(), ns, ar),
			None => Api::all_with(self.client.clone(), ar),
		}
	}
}

/// Ensure metadata.annotations exists in the manifest.
fn ensure_annotations(manifest: &serde_json::Value) -> serde_json::Value {
	let mut manifest = manifest.clone();
	if let serde_json::Value::Object(ref mut obj) = manifest {
		if let Some(serde_json::Value::Object(ref mut metadata)) = obj.get_mut("metadata") {
			metadata
				.entry("annotations")
				.or_insert(serde_json::json!({}));
		}
	}
	manifest
}
