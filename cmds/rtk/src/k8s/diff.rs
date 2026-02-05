//! Kubernetes resource diffing engine.
//!
//! This module implements the core diffing logic that compares local
//! manifests against cluster state using various strategies.

use std::{collections::HashSet, fmt, sync::Arc};

use kube::{
	api::{Api, DynamicObject, ListParams, Patch, PatchParams, PostParams},
	core::GroupVersionKind,
	Client,
};
use similar::TextDiff;
use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::instrument;

use super::{
	client::ClusterConnection,
	discovery::{gvk_from_manifest, ApiResourceCache, DiscoveredResource, DiscoveryError},
	ResourceScope,
};
use crate::spec::DiffStrategy;

/// Errors that can occur during diff operations.
#[derive(Debug, Error)]
pub enum DiffError {
	#[error("manifest missing apiVersion or kind")]
	MissingApiVersionOrKind,

	#[error("manifest missing metadata.name")]
	MissingName,

	#[error("unknown resource type: {api_version}/{kind}")]
	UnknownResourceType { api_version: String, kind: String },

	#[error(
		"spec.injectLabels is set to false in your spec.json. Tanka needs to add \
		 a label to your resources to reliably detect which were removed from Jsonnet. \
		 See https://tanka.dev/garbage-collection for more details"
	)]
	PruneRequiresInjectLabels,

	#[error("diff task panicked")]
	TaskPanicked(#[source] tokio::task::JoinError),

	#[error("internal error: concurrency semaphore unexpectedly closed")]
	SemaphoreClosed,

	#[error("building API resource cache")]
	BuildingApiCache(#[source] Box<DiscoveryError>),

	#[error("dry-run patch for {kind}/{name}")]
	DryRunPatch {
		kind: String,
		name: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("dry-run create for {kind}/{name}")]
	DryRunCreate {
		kind: String,
		name: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("server-side validation failed for {kind}/{name}")]
	ServerValidation {
		kind: String,
		name: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("checking if namespace '{namespace}' exists")]
	NamespaceCheck {
		namespace: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("fetching {kind}/{name} from cluster")]
	FetchResource {
		kind: String,
		name: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("converting manifest to DynamicObject")]
	ManifestConversion(#[source] serde_json::Error),

	#[error("serializing resource to JSON")]
	JsonSerialization(#[source] serde_json::Error),

	#[error("converting resource to YAML")]
	YamlConversion(#[source] serde_saphyr::ser_error::Error),
}

/// Status of a single resource comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
	/// Resource is identical in cluster and manifest.
	Unchanged,

	/// Resource exists but differs.
	Modified,

	/// Resource doesn't exist in cluster (will be created).
	Added,

	/// Resource in cluster but not in manifests (prune candidate).
	Deleted,

	/// Namespace doesn't exist yet, so resource comparison is deferred.
	SoonAdded,
}

impl DiffStatus {
	/// Returns true if this status represents a change.
	pub fn has_changes(&self) -> bool {
		!matches!(self, DiffStatus::Unchanged)
	}
}

impl fmt::Display for DiffStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DiffStatus::Unchanged => write!(f, "unchanged"),
			DiffStatus::Modified => write!(f, "modified"),
			DiffStatus::Added => write!(f, "added"),
			DiffStatus::Deleted => write!(f, "deleted"),
			DiffStatus::SoonAdded => write!(f, "soon-added"),
		}
	}
}

/// Result of diffing a single Kubernetes resource.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDiff {
	/// The resource's GroupVersionKind.
	pub gvk: GroupVersionKind,
	/// The namespace (None for cluster-scoped resources).
	pub namespace: Option<String>,
	/// The resource name.
	pub name: String,
	/// The diff status.
	pub status: DiffStatus,
	/// Current state YAML (empty string if resource doesn't exist).
	pub current_yaml: String,
	/// Desired state YAML (empty string if resource is being deleted).
	pub desired_yaml: String,
}

impl ResourceDiff {
	/// Returns true if this diff represents a change.
	pub fn has_changes(&self) -> bool {
		self.status.has_changes()
	}

	/// Get the filename for diff output (kubectl format).
	///
	/// Format: `group.version.kind.namespace.name` (group omitted if empty).
	/// Namespace is always included (empty string for cluster-scoped resources).
	/// Used by native, server, and validate strategies.
	pub fn display_name(&self) -> String {
		let group_prefix = if self.gvk.group.is_empty() {
			String::new()
		} else {
			format!("{}.", self.gvk.group)
		};
		let ns = self.namespace.as_deref().unwrap_or("");
		format!(
			"{}{}.{}.{}.{}",
			group_prefix, self.gvk.version, self.gvk.kind, ns, self.name
		)
	}

	/// Get the filename for diff output (subset format).
	///
	/// Format: `apiVersion.kind.namespace.name` with `/` replaced by `-`.
	/// Namespace is always included (empty string for cluster-scoped resources).
	/// Used by the subset strategy (k8s < 1.13 fallback).
	pub fn display_name_subset(&self) -> String {
		let api_version = self.gvk.api_version().replace('/', "-");
		let ns = self.namespace.as_deref().unwrap_or("");
		format!("{}.{}.{}.{}", api_version, self.gvk.kind, ns, self.name)
	}

	/// Compute the unified diff between current and desired state.
	pub fn unified_diff(&self, strategy: DiffStrategy) -> String {
		let name = match strategy {
			DiffStrategy::Subset => self.display_name_subset(),
			_ => self.display_name(),
		};
		let (old_header, new_header) = match self.status {
			DiffStatus::Added | DiffStatus::SoonAdded => {
				("/dev/null".to_string(), format!("b/{}", name))
			}
			DiffStatus::Deleted => (format!("a/{}", name), "/dev/null".to_string()),
			_ => (format!("a/{}", name), format!("b/{}", name)),
		};

		TextDiff::from_lines(&self.current_yaml, &self.desired_yaml)
			.unified_diff()
			.context_radius(3)
			.header(&old_header, &new_header)
			.to_string()
	}
}

/// Engine for computing diffs against a Kubernetes cluster.
#[derive(Clone)]
pub struct DiffEngine {
	client: Client,
	api_cache: ApiResourceCache,
	strategy: DiffStrategy,
	default_namespace: String,
}

/// Information extracted from a manifest for diff operations.
struct ManifestInfo {
	gvk: GroupVersionKind,
	name: String,
	namespace: Option<String>,
	discovered: DiscoveredResource,
}

/// Maximum concurrent API operations for parallel diffing.
const MAX_CONCURRENT_DIFF_OPS: usize = 8;

/// Result type for tasks in the shared worker pool.
enum DiffTaskResult {
	/// Result from diffing a manifest.
	Diff(Result<ResourceDiff, DiffError>),
	/// Result from scanning a resource type for deleted resources.
	Prune(Result<Vec<ResourceDiff>, DiffError>),
}

impl DiffEngine {
	/// Create a new diff engine from a cluster connection.
	///
	/// The `manifests` parameter is used to determine which resource types need
	/// to be discovered. This enables lazy discovery on older clusters that don't
	/// support aggregated discovery.
	///
	/// The `with_prune` parameter indicates whether prune detection will be used.
	/// When true, full discovery is used on older clusters to find all resource types.
	#[instrument(skip(connection, manifests), fields(manifest_count = manifests.len(), with_prune))]
	pub async fn new(
		connection: ClusterConnection,
		strategy: DiffStrategy,
		default_namespace: String,
		manifests: &[serde_json::Value],
		with_prune: bool,
	) -> Result<Self, DiffError> {
		// Extract unique resource keys from manifests for lazy discovery fallback
		let required_keys: std::collections::HashSet<_> =
			manifests.iter().filter_map(gvk_from_manifest).collect();

		let api_cache = ApiResourceCache::build(connection.client(), required_keys, with_prune)
			.await
			.map_err(|e| DiffError::BuildingApiCache(Box::new(e)))?;

		Ok(Self {
			client: connection.client().clone(),
			api_cache,
			strategy,
			default_namespace,
		})
	}

	/// Extract common manifest information needed for diff/validate operations.
	fn extract_manifest_info(
		&self,
		manifest: &serde_json::Value,
	) -> Result<ManifestInfo, DiffError> {
		let gvk = gvk_from_manifest(manifest).ok_or(DiffError::MissingApiVersionOrKind)?;

		let name = manifest
			.pointer("/metadata/name")
			.and_then(|v| v.as_str())
			.ok_or(DiffError::MissingName)?
			.to_string();

		let discovered = self
			.api_cache
			.lookup(&gvk)
			.ok_or_else(|| DiffError::UnknownResourceType {
				api_version: gvk.api_version(),
				kind: gvk.kind.clone(),
			})?
			.clone();

		let namespace = match discovered.scope {
			ResourceScope::Namespaced => {
				let ns = manifest
					.pointer("/metadata/namespace")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string())
					.unwrap_or_else(|| self.default_namespace.clone());
				Some(ns)
			}
			ResourceScope::ClusterWide => None,
		};

		Ok(ManifestInfo {
			gvk,
			name,
			namespace,
			discovered,
		})
	}

	/// Diff a single manifest against cluster state.
	#[instrument(skip(self, manifest), fields(
		kind = manifest.get("kind").and_then(|v| v.as_str()).unwrap_or("unknown"),
		name = manifest.pointer("/metadata/name").and_then(|v| v.as_str()).unwrap_or("unknown"),
	))]
	pub async fn diff_manifest(
		&self,
		manifest: &serde_json::Value,
	) -> Result<ResourceDiff, DiffError> {
		let ManifestInfo {
			gvk,
			name,
			namespace,
			discovered,
		} = self.extract_manifest_info(manifest)?;

		// Check if namespace exists (for namespaced resources)
		if let Some(ref ns) = namespace {
			if !self.namespace_exists(ns).await? {
				return Ok(ResourceDiff {
					gvk,
					namespace,
					name,
					status: DiffStatus::SoonAdded,
					current_yaml: String::new(),
					desired_yaml: self.manifest_to_yaml(manifest)?,
				});
			}
		}

		// Execute the appropriate diff strategy
		match self.strategy {
			DiffStrategy::Native => {
				self.diff_native(&gvk, &name, namespace, manifest, &discovered)
					.await
			}
			DiffStrategy::Server => {
				self.diff_server(&gvk, &name, namespace, manifest, &discovered)
					.await
			}
			DiffStrategy::Validate => {
				self.diff_validate(&gvk, &name, namespace, manifest, &discovered)
					.await
			}
			DiffStrategy::Subset => {
				self.diff_subset(&gvk, &name, namespace, manifest, &discovered)
					.await
			}
		}
	}

	/// Validate a single manifest on the server using server-side apply dry-run.
	///
	/// This performs validation only, without computing a diff. Used by the
	/// validate strategy to validate all resources before diffing any.
	#[instrument(skip_all)]
	async fn validate_manifest(&self, manifest: &serde_json::Value) -> Result<(), DiffError> {
		let ManifestInfo {
			gvk,
			name,
			namespace,
			discovered,
		} = self.extract_manifest_info(manifest)?;

		let api = self.dynamic_api(&discovered.api_resource, namespace.as_deref());

		// Validate on server using server-side apply with dry-run
		let validate_params = PatchParams {
			dry_run: true,
			field_manager: Some("tanka".to_string()),
			force: true,
			..Default::default()
		};

		// Ensure annotations exists (kubectl always includes this)
		let manifest = ensure_annotations(manifest);

		api.patch(&name, &validate_params, &Patch::Apply(&manifest))
			.await
			.map_err(|e| DiffError::ServerValidation {
				kind: gvk.kind.clone(),
				name: name.to_string(),
				source: Box::new(e),
			})?;

		Ok(())
	}

	/// Diff a single manifest using the native strategy.
	///
	/// This forces the native strategy regardless of self.strategy. Used by the
	/// validate strategy to diff all resources after validation.
	#[instrument(skip_all)]
	async fn diff_manifest_native(
		&self,
		manifest: &serde_json::Value,
	) -> Result<ResourceDiff, DiffError> {
		let ManifestInfo {
			gvk,
			name,
			namespace,
			discovered,
		} = self.extract_manifest_info(manifest)?;

		// Check if namespace exists (for namespaced resources)
		if let Some(ref ns) = namespace {
			if !self.namespace_exists(ns).await? {
				return Ok(ResourceDiff {
					gvk,
					namespace,
					name,
					status: DiffStatus::SoonAdded,
					current_yaml: String::new(),
					desired_yaml: self.manifest_to_yaml(manifest)?,
				});
			}
		}

		// Always use native strategy
		self.diff_native(&gvk, &name, namespace, manifest, &discovered)
			.await
	}

	/// Diff all manifests against cluster state.
	///
	/// Returns diffs for all resources, optionally including prune candidates.
	/// All operations (diffing and prune detection) run concurrently in a shared
	/// worker pool with bounded parallelism.
	///
	/// # Arguments
	/// * `manifests` - The local manifests to diff
	/// * `with_prune` - Whether to detect orphaned resources for pruning
	/// * `env_label` - The environment label hash (from generate_environment_label)
	/// * `inject_labels` - Whether spec.injectLabels is true (required for prune)
	#[instrument(skip(self, manifests), fields(manifest_count = manifests.len()))]
	pub async fn diff_all(
		&self,
		manifests: &[serde_json::Value],
		with_prune: bool,
		env_label: Option<&str>,
		inject_labels: bool,
	) -> Result<Vec<ResourceDiff>, DiffError> {
		// Validate prune requirements early
		let do_prune = if with_prune {
			if !inject_labels {
				return Err(DiffError::PruneRequiresInjectLabels);
			}
			if env_label.is_none() {
				tracing::warn!(
					"--with-prune specified but environment has no label; skipping prune detection"
				);
				false
			} else {
				true
			}
		} else {
			false
		};

		// Validate strategy: validate ALL resources first, then diff
		if self.strategy == DiffStrategy::Validate {
			self.validate_all_parallel(manifests).await?;
		}

		// Shared worker pool for all operations
		let engine = Arc::new(self.clone());
		let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DIFF_OPS));
		let mut join_set: JoinSet<DiffTaskResult> = JoinSet::new();

		let manifests: Vec<Arc<serde_json::Value>> =
			manifests.iter().cloned().map(Arc::new).collect();

		// Spawn diff tasks
		let use_native = self.strategy == DiffStrategy::Validate;
		Self::spawn_diff_tasks(&mut join_set, &engine, &semaphore, &manifests, use_native);

		// Spawn prune tasks (one per resource type)
		if do_prune {
			let label = env_label.expect("checked above");
			Self::spawn_prune_tasks(
				&mut join_set,
				&engine,
				&semaphore,
				&manifests,
				label,
				&self.api_cache,
			);
		}

		// Collect all results
		let mut diffs = Vec::with_capacity(manifests.len());
		while let Some(result) = join_set.join_next().await {
			match result.map_err(DiffError::TaskPanicked)? {
				DiffTaskResult::Diff(Ok(diff)) => diffs.push(diff),
				DiffTaskResult::Diff(Err(e)) => return Err(e),
				DiffTaskResult::Prune(Ok(deleted)) => diffs.extend(deleted),
				DiffTaskResult::Prune(Err(e)) => {
					tracing::warn!(error = %e, "prune task failed")
				}
			}
		}

		diffs.sort_by(|a, b| {
			a.gvk
				.group
				.cmp(&b.gvk.group)
				.then_with(|| a.gvk.version.cmp(&b.gvk.version))
				.then_with(|| a.gvk.kind.cmp(&b.gvk.kind))
				.then_with(|| a.namespace.cmp(&b.namespace))
				.then_with(|| a.name.cmp(&b.name))
		});

		Ok(diffs)
	}

	/// Spawn diff tasks into the shared worker pool.
	fn spawn_diff_tasks(
		join_set: &mut JoinSet<DiffTaskResult>,
		engine: &Arc<Self>,
		semaphore: &Arc<Semaphore>,
		manifests: &[Arc<serde_json::Value>],
		use_native: bool,
	) {
		for manifest in manifests {
			let manifest = manifest.clone();
			let engine = engine.clone();
			let sem = semaphore.clone();

			join_set.spawn(async move {
				let _permit = match sem.acquire().await {
					Ok(permit) => permit,
					Err(_) => return DiffTaskResult::Diff(Err(DiffError::SemaphoreClosed)),
				};
				let result = if use_native {
					engine.diff_manifest_native(&manifest).await
				} else {
					engine.diff_manifest(&manifest).await
				};
				DiffTaskResult::Diff(result)
			});
		}
	}

	/// Build a set of (apiVersion, kind, namespace, name) from manifests for prune detection.
	///
	/// For namespaced resources without an explicit namespace, uses the default namespace.
	/// This ensures manifests relying on spec.namespace match cluster resources.
	fn build_manifest_keys(
		manifests: &[Arc<serde_json::Value>],
		default_namespace: &str,
		api_cache: &ApiResourceCache,
	) -> HashSet<(String, String, Option<String>, String)> {
		manifests
			.iter()
			.filter_map(|m| {
				let api_version = m.get("apiVersion")?.as_str()?.to_string();
				let kind = m.get("kind")?.as_str()?.to_string();
				let name = m.pointer("/metadata/name")?.as_str()?.to_string();

				// Get explicit namespace from manifest
				let explicit_namespace = m
					.pointer("/metadata/namespace")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string());

				// Determine if this resource is namespaced using api_cache
				let gvk = gvk_from_manifest(m)?;
				let is_namespaced = api_cache
					.lookup(&gvk)
					.map(|d| d.scope == ResourceScope::Namespaced)
					.unwrap_or(true); // Default to namespaced if unknown

				// Use default namespace for namespaced resources without explicit namespace
				let namespace = if is_namespaced {
					Some(explicit_namespace.unwrap_or_else(|| default_namespace.to_string()))
				} else {
					None
				};

				Some((api_version, kind, namespace, name))
			})
			.collect()
	}

	/// Spawn prune detection tasks into the shared worker pool.
	fn spawn_prune_tasks(
		join_set: &mut JoinSet<DiffTaskResult>,
		engine: &Arc<Self>,
		semaphore: &Arc<Semaphore>,
		manifests: &[Arc<serde_json::Value>],
		env_label: &str,
		api_cache: &ApiResourceCache,
	) {
		let manifest_keys = Arc::new(Self::build_manifest_keys(
			manifests,
			&engine.default_namespace,
			api_cache,
		));
		let label = env_label.to_string();

		for (gvk, discovered) in api_cache.iter() {
			// Skip resources that don't support list operation
			if !discovered
				.capabilities
				.supports_operation(kube::discovery::verbs::LIST)
			{
				continue;
			}

			let engine = engine.clone();
			let sem = semaphore.clone();
			let gvk = gvk.clone();
			let discovered = discovered.clone();
			let manifest_keys = manifest_keys.clone();
			let label = label.clone();

			join_set.spawn(async move {
				let _permit = match sem.acquire().await {
					Ok(permit) => permit,
					Err(_) => return DiffTaskResult::Prune(Err(DiffError::SemaphoreClosed)),
				};
				let result = engine
					.find_deleted_for_type(&gvk, &discovered, &manifest_keys, &label)
					.await;
				DiffTaskResult::Prune(result)
			});
		}
	}

	/// Find deleted resources for a single resource type.
	#[instrument(skip_all, fields(kind = %gvk.kind))]
	async fn find_deleted_for_type(
		&self,
		gvk: &GroupVersionKind,
		discovered: &DiscoveredResource,
		manifest_keys: &HashSet<(String, String, Option<String>, String)>,
		env_label: &str,
	) -> Result<Vec<ResourceDiff>, DiffError> {
		let label_selector = format!("tanka.dev/environment={}", env_label);
		let list_params = ListParams::default().labels(&label_selector);

		let api: Api<DynamicObject> = Api::all_with(self.client.clone(), &discovered.api_resource);

		let resources = match api.list(&list_params).await {
			Ok(list) => list,
			Err(e) => {
				tracing::debug!(kind = %gvk.kind, error = %e, "skipping resource type during prune");
				return Ok(Vec::new());
			}
		};

		let mut deleted = Vec::new();
		for resource in resources {
			let Some(name) = resource.metadata.name.clone() else {
				continue;
			};
			let namespace = resource.metadata.namespace.clone();

			let resource_key = (
				gvk.api_version(),
				gvk.kind.clone(),
				namespace.clone(),
				name.clone(),
			);

			if manifest_keys.contains(&resource_key) {
				continue;
			}

			if !Self::is_directly_created(&resource) {
				tracing::debug!(
					kind = %gvk.kind,
					name = %name,
					"skipping resource - not directly created by Tanka/kubectl"
				);
				continue;
			}

			let current_yaml = serde_json::to_value(&resource)
				.ok()
				.and_then(|v| value_to_yaml(&v).ok())
				.unwrap_or_default();

			deleted.push(ResourceDiff {
				gvk: gvk.clone(),
				namespace,
				name,
				status: DiffStatus::Deleted,
				current_yaml,
				desired_yaml: String::new(),
			});
		}

		Ok(deleted)
	}

	/// Validate all manifests in parallel using server-side apply dry-run.
	#[instrument(skip_all, fields(manifest_count = manifests.len()))]
	async fn validate_all_parallel(
		&self,
		manifests: &[serde_json::Value],
	) -> Result<(), DiffError> {
		let engine = Arc::new(self.clone());
		let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DIFF_OPS));
		let mut join_set = JoinSet::new();

		for manifest in manifests {
			let manifest = manifest.clone();
			let engine = engine.clone();
			let sem = semaphore.clone();

			join_set.spawn(async move {
				let _permit = match sem.acquire().await {
					Ok(permit) => permit,
					Err(_) => return Err(DiffError::SemaphoreClosed),
				};
				engine.validate_manifest(&manifest).await
			});
		}

		// Collect results, failing fast on first error
		while let Some(result) = join_set.join_next().await {
			result.map_err(DiffError::TaskPanicked)??;
		}

		Ok(())
	}

	/// Check if a resource was directly created by Tanka or kubectl.
	///
	/// Returns true if the resource has either:
	/// - The `kubectl.kubernetes.io/last-applied-configuration` annotation (client-side apply)
	/// - A managed field with manager "tanka", "kubectl-client-side-apply", or "kustomize-controller"
	fn is_directly_created(resource: &DynamicObject) -> bool {
		const ANNOTATION_LAST_APPLIED: &str = "kubectl.kubernetes.io/last-applied-configuration";
		const VALID_MANAGERS: &[&str] =
			&["tanka", "kubectl-client-side-apply", "kustomize-controller"];

		// Check for last-applied-configuration annotation (client-side apply)
		if resource
			.metadata
			.annotations
			.as_ref()
			.is_some_and(|a| a.contains_key(ANNOTATION_LAST_APPLIED))
		{
			return true;
		}

		// Check for known field managers (server-side apply)
		resource
			.metadata
			.managed_fields
			.as_ref()
			.is_some_and(|fields| {
				fields.iter().any(|f| {
					f.manager
						.as_ref()
						.is_some_and(|m| VALID_MANAGERS.contains(&m.as_str()))
				})
			})
	}

	/// Native diff strategy: client-side patch.
	///
	/// Tries strategic merge patch first (better array handling for built-in types),
	/// falls back to JSON merge patch for CRDs that don't support strategic merge.
	///
	/// For new resources, performs a dry-run CREATE to get the server-mutated version.
	#[instrument(skip_all)]
	async fn diff_native(
		&self,
		gvk: &GroupVersionKind,
		name: &str,
		namespace: Option<String>,
		manifest: &serde_json::Value,
		discovered: &DiscoveredResource,
	) -> Result<ResourceDiff, DiffError> {
		let api = self.dynamic_api(&discovered.api_resource, namespace.as_deref());

		// Try to get current state
		let current = match api
			.get_opt(name)
			.await
			.map_err(|e| DiffError::FetchResource {
				kind: gvk.kind.clone(),
				name: name.to_string(),
				source: Box::new(e),
			})? {
			Some(obj) => obj,
			None => {
				// Resource doesn't exist - do dry-run create to get server-mutated version
				return self.diff_create(gvk, name, namespace, manifest, &api).await;
			}
		};

		let patch_params = PatchParams {
			dry_run: true,
			..Default::default()
		};

		// Ensure annotations exists (kubectl always includes this)
		let manifest = ensure_annotations(manifest);

		// Try strategic merge patch first (works for built-in types)
		let merged = match api
			.patch(name, &patch_params, &Patch::Strategic(&manifest))
			.await
		{
			Ok(result) => result,
			Err(kube::Error::Api(ref err)) if err.code == 415 => {
				// UnsupportedMediaType - CRD doesn't support strategic merge, fall back to merge patch
				api.patch(name, &patch_params, &Patch::Merge(manifest))
					.await
					.map_err(|e| DiffError::DryRunPatch {
						kind: gvk.kind.clone(),
						name: name.to_string(),
						source: Box::new(e),
					})?
			}
			Err(e) => {
				return Err(DiffError::DryRunPatch {
					kind: gvk.kind.clone(),
					name: name.to_string(),
					source: Box::new(e),
				})
			}
		};

		// Compare current vs merged
		self.compute_diff(gvk, name, namespace, &current, &merged)
	}

	/// Diff a new resource by performing a dry-run CREATE.
	///
	/// This sends the manifest to the API server with dry-run to get the
	/// server-mutated version (including generated fields like uid, creationTimestamp).
	#[instrument(skip_all)]
	async fn diff_create(
		&self,
		gvk: &GroupVersionKind,
		name: &str,
		namespace: Option<String>,
		manifest: &serde_json::Value,
		api: &Api<DynamicObject>,
	) -> Result<ResourceDiff, DiffError> {
		let post_params = PostParams {
			dry_run: true,
			..Default::default()
		};

		// Convert manifest to DynamicObject for create
		let obj: DynamicObject =
			serde_json::from_value(manifest.clone()).map_err(DiffError::ManifestConversion)?;

		let created =
			api.create(&post_params, &obj)
				.await
				.map_err(|e| DiffError::DryRunCreate {
					kind: gvk.kind.clone(),
					name: name.to_string(),
					source: Box::new(e),
				})?;

		// Diff empty vs server-returned object
		let desired_yaml = self.object_to_yaml(&created)?;

		Ok(ResourceDiff {
			gvk: gvk.clone(),
			namespace,
			name: name.to_string(),
			status: DiffStatus::Added,
			current_yaml: String::new(),
			desired_yaml,
		})
	}

	/// Server diff strategy: server-side dry-run with force-conflicts.
	///
	/// For new resources, performs a dry-run CREATE to get the server-mutated version.
	#[instrument(skip_all)]
	async fn diff_server(
		&self,
		gvk: &GroupVersionKind,
		name: &str,
		namespace: Option<String>,
		manifest: &serde_json::Value,
		discovered: &DiscoveredResource,
	) -> Result<ResourceDiff, DiffError> {
		let api = self.dynamic_api(&discovered.api_resource, namespace.as_deref());

		// Try to get current state
		let current = match api
			.get_opt(name)
			.await
			.map_err(|e| DiffError::FetchResource {
				kind: gvk.kind.clone(),
				name: name.to_string(),
				source: Box::new(e),
			})? {
			Some(obj) => obj,
			None => {
				// Resource doesn't exist - do dry-run create to get server-mutated version
				return self.diff_create(gvk, name, namespace, manifest, &api).await;
			}
		};

		// Server-side apply with dry-run and force
		let patch_params = PatchParams {
			dry_run: true,
			field_manager: Some("tanka".to_string()),
			force: true,
			..Default::default()
		};

		// Ensure annotations exists (kubectl always includes this)
		let manifest = ensure_annotations(manifest);

		let merged = api
			.patch(name, &patch_params, &Patch::Apply(&manifest))
			.await
			.map_err(|e| DiffError::DryRunPatch {
				kind: gvk.kind.clone(),
				name: name.to_string(),
				source: Box::new(e),
			})?;

		self.compute_diff(gvk, name, namespace, &current, &merged)
	}

	/// Validate diff strategy: server-side validation + client-side diff.
	///
	/// First validates the manifest on the server using dry-run apply,
	/// then computes the diff client-side for output.
	#[instrument(skip_all)]
	async fn diff_validate(
		&self,
		gvk: &GroupVersionKind,
		name: &str,
		namespace: Option<String>,
		manifest: &serde_json::Value,
		discovered: &DiscoveredResource,
	) -> Result<ResourceDiff, DiffError> {
		let api = self.dynamic_api(&discovered.api_resource, namespace.as_deref());

		// First, validate on server using server-side apply with dry-run
		let validate_params = PatchParams {
			dry_run: true,
			field_manager: Some("tanka".to_string()),
			force: true,
			..Default::default()
		};

		// Ensure annotations exists (kubectl always includes this)
		let manifest = ensure_annotations(manifest);

		// This will fail if the manifest is invalid
		api.patch(name, &validate_params, &Patch::Apply(&manifest))
			.await
			.map_err(|e| DiffError::ServerValidation {
				kind: gvk.kind.clone(),
				name: name.to_string(),
				source: Box::new(e),
			})?;

		// Now do client-side diff using native strategy
		self.diff_native(gvk, name, namespace, &manifest, discovered)
			.await
	}

	/// Subset diff strategy: GET + compare only manifest fields.
	#[instrument(skip_all)]
	async fn diff_subset(
		&self,
		gvk: &GroupVersionKind,
		name: &str,
		namespace: Option<String>,
		manifest: &serde_json::Value,
		discovered: &DiscoveredResource,
	) -> Result<ResourceDiff, DiffError> {
		let api = self.dynamic_api(&discovered.api_resource, namespace.as_deref());

		// Get current state
		let current = match api
			.get_opt(name)
			.await
			.map_err(|e| DiffError::FetchResource {
				kind: gvk.kind.clone(),
				name: name.to_string(),
				source: Box::new(e),
			})? {
			Some(obj) => obj,
			None => {
				return Ok(ResourceDiff {
					gvk: gvk.clone(),
					namespace,
					name: name.to_string(),
					status: DiffStatus::Added,
					current_yaml: String::new(),
					desired_yaml: self.manifest_to_yaml(manifest)?,
				});
			}
		};

		// Filter current to only fields in manifest
		let current_json: serde_json::Value =
			serde_json::to_value(&current).map_err(DiffError::JsonSerialization)?;
		let filtered_current = filter_to_manifest_fields(&current_json, manifest);

		// Compare filtered current vs manifest
		let current_yaml = value_to_yaml(&filtered_current)?;
		let desired_yaml = value_to_yaml(manifest)?;

		let status = if current_yaml == desired_yaml {
			DiffStatus::Unchanged
		} else {
			DiffStatus::Modified
		};
		Ok(ResourceDiff {
			gvk: gvk.clone(),
			namespace,
			name: name.to_string(),
			status,
			current_yaml,
			desired_yaml,
		})
	}

	/// Compute diff between current and merged states.
	fn compute_diff(
		&self,
		gvk: &GroupVersionKind,
		name: &str,
		namespace: Option<String>,
		current: &DynamicObject,
		merged: &DynamicObject,
	) -> Result<ResourceDiff, DiffError> {
		// Convert to YAML for comparison
		let current_yaml = self.object_to_yaml(current)?;
		let desired_yaml = self.object_to_yaml(merged)?;

		let status = if current_yaml == desired_yaml {
			DiffStatus::Unchanged
		} else {
			DiffStatus::Modified
		};
		Ok(ResourceDiff {
			gvk: gvk.clone(),
			namespace,
			name: name.to_string(),
			status,
			current_yaml,
			desired_yaml,
		})
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

	/// Check if a namespace exists.
	#[instrument(skip(self))]
	async fn namespace_exists(&self, name: &str) -> Result<bool, DiffError> {
		use k8s_openapi::api::core::v1::Namespace;
		let api: Api<Namespace> = Api::all(self.client.clone());
		match api
			.get_opt(name)
			.await
			.map_err(|e| DiffError::NamespaceCheck {
				namespace: name.to_string(),
				source: Box::new(e),
			})? {
			Some(_) => Ok(true),
			None => Ok(false),
		}
	}

	/// Convert a manifest to YAML string.
	fn manifest_to_yaml(&self, manifest: &serde_json::Value) -> Result<String, DiffError> {
		value_to_yaml(manifest)
	}

	/// Convert a DynamicObject to YAML string.
	fn object_to_yaml(&self, obj: &DynamicObject) -> Result<String, DiffError> {
		let json = serde_json::to_value(obj).map_err(DiffError::JsonSerialization)?;
		// Strip managed fields and other noise for cleaner diffs
		let cleaned = strip_kubectl_fields(&json);
		value_to_yaml(&cleaned)
	}
}

/// Convert a serde_json::Value to YAML string.
fn value_to_yaml(value: &serde_json::Value) -> Result<String, DiffError> {
	crate::yaml::to_yaml(value).map_err(DiffError::YamlConversion)
}

/// Ensure metadata.annotations exists in the manifest.
///
/// kubectl's GetModifiedConfiguration ensures annotations is always present
/// (even as an empty object) before sending patches. This matches that behavior.
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

/// Fields to strip from resources for diffs.
///
/// Only managedFields is stripped by default (matching kubectl diff --show-managed-fields=false).
/// Other fields like uid, creationTimestamp, generation, resourceVersion are kept
/// because kubectl keeps them in its diff output.
const KUBECTL_STRIP_FIELDS: &[&str] = &["managedFields"];

/// Strip managed fields for cleaner diffs.
///
/// This matches kubectl's behavior with --show-managed-fields=false (the default).
/// Also strips empty annotations maps that are added by `ensure_annotations` for patch
/// compatibility but shouldn't appear in diff output.
fn strip_kubectl_fields(value: &serde_json::Value) -> serde_json::Value {
	if let serde_json::Value::Object(map) = value {
		let mut cleaned = serde_json::Map::new();

		for (k, v) in map {
			// Skip managed fields
			if KUBECTL_STRIP_FIELDS.contains(&k.as_str()) {
				continue;
			}

			// Recursively clean nested objects (especially metadata)
			if k == "metadata" {
				if let serde_json::Value::Object(meta) = v {
					let mut cleaned_meta = serde_json::Map::new();
					for (mk, mv) in meta {
						if KUBECTL_STRIP_FIELDS.contains(&mk.as_str()) {
							continue;
						}
						// Skip empty annotations (added by ensure_annotations for patch compat)
						if mk == "annotations" {
							if let serde_json::Value::Object(ann) = mv {
								if ann.is_empty() {
									continue;
								}
							}
						}
						cleaned_meta.insert(mk.clone(), strip_kubectl_fields(mv));
					}
					cleaned.insert(k.clone(), serde_json::Value::Object(cleaned_meta));
					continue;
				}
			}

			cleaned.insert(k.clone(), strip_kubectl_fields(v));
		}

		serde_json::Value::Object(cleaned)
	} else if let serde_json::Value::Array(arr) = value {
		serde_json::Value::Array(arr.iter().map(strip_kubectl_fields).collect())
	} else {
		value.clone()
	}
}

/// Filter current state to only include fields present in the manifest.
///
/// For subset diff, we only compare fields that are specified in the manifest.
/// This allows partial resource specifications to be diffed correctly.
fn filter_to_manifest_fields(
	current: &serde_json::Value,
	manifest: &serde_json::Value,
) -> serde_json::Value {
	match (current, manifest) {
		(serde_json::Value::Object(curr_map), serde_json::Value::Object(man_map)) => {
			let mut filtered = serde_json::Map::new();

			for (key, man_val) in man_map {
				if let Some(curr_val) = curr_map.get(key) {
					filtered.insert(key.clone(), filter_to_manifest_fields(curr_val, man_val));
				}
			}

			serde_json::Value::Object(filtered)
		}
		(serde_json::Value::Array(curr_arr), serde_json::Value::Array(man_arr)) => {
			// For arrays, process only indices that exist in the manifest.
			// For each manifest index, take the corresponding current element if it exists.
			let filtered: Vec<_> = man_arr
				.iter()
				.enumerate()
				.map(|(i, man_elem)| {
					if let Some(curr_elem) = curr_arr.get(i) {
						filter_to_manifest_fields(curr_elem, man_elem)
					} else {
						// Manifest has more elements than current - use manifest value
						man_elem.clone()
					}
				})
				.collect();
			serde_json::Value::Array(filtered)
		}
		// For primitives, return the current value
		(curr, _) => curr.clone(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_display_name_core_api() {
		let diff = ResourceDiff {
			gvk: GroupVersionKind::gvk("", "v1", "ConfigMap"),
			namespace: Some("default".to_string()),
			name: "my-config".to_string(),
			status: DiffStatus::Unchanged,
			current_yaml: String::new(),
			desired_yaml: String::new(),
		};
		assert_eq!(diff.display_name(), "v1.ConfigMap.default.my-config");
		assert_eq!(diff.display_name_subset(), "v1.ConfigMap.default.my-config");
	}

	#[test]
	fn test_display_name_with_group() {
		let diff = ResourceDiff {
			gvk: GroupVersionKind::gvk("apps", "v1", "Deployment"),
			namespace: Some("kube-system".to_string()),
			name: "coredns".to_string(),
			status: DiffStatus::Unchanged,
			current_yaml: String::new(),
			desired_yaml: String::new(),
		};
		// Native: group.version.kind.namespace.name
		assert_eq!(
			diff.display_name(),
			"apps.v1.Deployment.kube-system.coredns"
		);
		// Subset: apiVersion (with / replaced by -).kind.namespace.name
		assert_eq!(
			diff.display_name_subset(),
			"apps-v1.Deployment.kube-system.coredns"
		);
	}

	#[test]
	fn test_display_name_cluster_scoped() {
		let diff = ResourceDiff {
			gvk: GroupVersionKind::gvk("", "v1", "Namespace"),
			namespace: None,
			name: "production".to_string(),
			status: DiffStatus::Unchanged,
			current_yaml: String::new(),
			desired_yaml: String::new(),
		};
		// Cluster-scoped resources have empty namespace, resulting in ".."
		assert_eq!(diff.display_name(), "v1.Namespace..production");
		assert_eq!(diff.display_name_subset(), "v1.Namespace..production");
	}

	#[test]
	fn test_display_name_complex_group() {
		let diff = ResourceDiff {
			gvk: GroupVersionKind::gvk("cloud.grafana.net.namespaced", "v1alpha1", "AccessPolicy"),
			namespace: Some("tokens".to_string()),
			name: "my-policy".to_string(),
			status: DiffStatus::Unchanged,
			current_yaml: String::new(),
			desired_yaml: String::new(),
		};
		assert_eq!(
			diff.display_name(),
			"cloud.grafana.net.namespaced.v1alpha1.AccessPolicy.tokens.my-policy"
		);
		assert_eq!(
			diff.display_name_subset(),
			"cloud.grafana.net.namespaced-v1alpha1.AccessPolicy.tokens.my-policy"
		);
	}

	#[test]
	fn test_unified_diff_native_strategy() {
		let resource_diff = ResourceDiff {
			gvk: GroupVersionKind::gvk("apps", "v1", "Deployment"),
			namespace: Some("default".to_string()),
			name: "test".to_string(),
			status: DiffStatus::Modified,
			current_yaml: "old\n".to_string(),
			desired_yaml: "new\n".to_string(),
		};

		let diff_str = resource_diff.unified_diff(DiffStrategy::Native);
		let patch = patch::Patch::from_single(&diff_str).expect("valid unified diff");

		assert_eq!(patch.old.path, "a/apps.v1.Deployment.default.test");
		assert_eq!(patch.new.path, "b/apps.v1.Deployment.default.test");
	}

	#[test]
	fn test_unified_diff_subset_strategy() {
		let resource_diff = ResourceDiff {
			gvk: GroupVersionKind::gvk("apps", "v1", "Deployment"),
			namespace: Some("default".to_string()),
			name: "test".to_string(),
			status: DiffStatus::Modified,
			current_yaml: "old\n".to_string(),
			desired_yaml: "new\n".to_string(),
		};

		let diff_str = resource_diff.unified_diff(DiffStrategy::Subset);
		let patch = patch::Patch::from_single(&diff_str).expect("valid unified diff");

		assert_eq!(patch.old.path, "a/apps-v1.Deployment.default.test");
		assert_eq!(patch.new.path, "b/apps-v1.Deployment.default.test");
	}

	#[test]
	fn test_strip_kubectl_fields() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test",
				"namespace": "default",
				"resourceVersion": "12345",
				"uid": "abc-123",
				"creationTimestamp": "2024-01-01T00:00:00Z",
				"generation": 1,
				"managedFields": [],
				"annotations": {}
			},
			"data": {
				"key": "value"
			}
		});

		let cleaned = strip_kubectl_fields(&value);

		// managedFields and empty annotations should be stripped
		let expected = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test",
				"namespace": "default",
				"resourceVersion": "12345",
				"uid": "abc-123",
				"creationTimestamp": "2024-01-01T00:00:00Z",
				"generation": 1
			},
			"data": {
				"key": "value"
			}
		});

		assert_eq!(cleaned, expected);
	}

	#[test]
	fn test_strip_kubectl_fields_preserves_non_empty_annotations() {
		let value = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test",
				"annotations": {
					"key": "value"
				}
			}
		});

		let cleaned = strip_kubectl_fields(&value);

		// Non-empty annotations should be preserved
		assert_eq!(
			cleaned.pointer("/metadata/annotations/key"),
			Some(&serde_json::json!("value"))
		);
	}

	#[test]
	fn test_filter_to_manifest_fields() {
		let current = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test",
				"namespace": "default",
				"labels": {
					"app": "test",
					"added-by-controller": "true"
				}
			},
			"data": {
				"key1": "value1",
				"key2": "value2"
			}
		});

		let manifest = serde_json::json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {
				"name": "test",
				"labels": {
					"app": "test"
				}
			},
			"data": {
				"key1": "value1"
			}
		});

		let filtered = filter_to_manifest_fields(&current, &manifest);

		// Should only include fields from manifest
		assert_eq!(
			filtered.pointer("/data/key1"),
			Some(&serde_json::json!("value1"))
		);
		assert!(filtered.pointer("/data/key2").is_none());
		assert!(filtered.pointer("/metadata/namespace").is_none());
	}
}
