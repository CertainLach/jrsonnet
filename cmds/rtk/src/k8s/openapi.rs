//! OpenAPI schema fetching and caching for strategic merge patch.
//!
//! This module fetches OpenAPI schemas from Kubernetes API servers to get
//! accurate merge key information for CRDs and other types not in the
//! built-in schemas.

use std::{
	collections::HashMap,
	fs,
	path::PathBuf,
	sync::Arc,
	time::{Duration, SystemTime},
};

use k8s::strategicpatch::{builtin_schemas::has_builtin_coverage, PatchStrategy};
use kube::{core::GroupVersionKind, Client};
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, instrument, warn};

/// Errors that can occur during OpenAPI schema operations.
#[derive(Debug, Error)]
pub enum OpenApiError {
	#[error("failed to fetch OpenAPI schema for {group}/{version}")]
	Fetch {
		group: String,
		version: String,
		#[source]
		source: Box<kube::Error>,
	},

	#[error("failed to parse OpenAPI schema")]
	Parse(#[source] serde_json::Error),

	#[error("failed to read cache file")]
	CacheRead(#[source] std::io::Error),

	#[error("failed to write cache file")]
	CacheWrite(#[source] std::io::Error),
}

/// Extracted OpenAPI schema data for a GVK.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExtractedSchema {
	/// Merge keys by "type.field" -> merge_keys.
	pub merge_keys: HashMap<String, Vec<String>>,
	/// Patch strategies by "type.field" -> strategies.
	pub patch_strategies: HashMap<String, Vec<PatchStrategy>>,
	/// Type references: "type.field" -> referenced type name.
	/// Used to resolve paths to schema types.
	pub type_refs: HashMap<String, String>,
}

/// Cache key for OpenAPI schemas (group + version).
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SchemaKey {
	group: String,
	version: String,
}

impl SchemaKey {
	fn from_gvk(gvk: &GroupVersionKind) -> Self {
		Self {
			group: gvk.group.clone(),
			version: gvk.version.clone(),
		}
	}

	fn api_path(&self) -> String {
		if self.group.is_empty() {
			format!("/openapi/v3/api/{}", self.version)
		} else {
			format!("/openapi/v3/apis/{}/{}", self.group, self.version)
		}
	}

	fn cache_filename(&self) -> String {
		if self.group.is_empty() {
			format!("core_{}.json", self.version)
		} else {
			format!("{}_{}.json", self.group.replace('/', "_"), self.version)
		}
	}
}

/// Cache entry with metadata for invalidation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedSchema {
	/// The extracted schema information.
	data: ExtractedSchema,
	/// Unix timestamp when the cache was created.
	cached_at: u64,
	/// Server git version when cached (e.g., "v1.28.0").
	/// Used to invalidate cache when cluster is upgraded.
	#[serde(default)]
	server_version: Option<String>,
}

impl CachedSchema {
	/// Cache TTL - entries older than this are considered stale.
	const TTL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

	fn new(data: ExtractedSchema, server_version: Option<String>) -> Self {
		let cached_at = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.map(|d| d.as_secs())
			.unwrap_or(0);
		Self {
			data,
			cached_at,
			server_version,
		}
	}

	fn is_stale(&self, current_server_version: Option<&str>) -> bool {
		// Check TTL
		let now = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.map(|d| d.as_secs())
			.unwrap_or(0);
		if now.saturating_sub(self.cached_at) > Self::TTL.as_secs() {
			return true;
		}

		// Check server version mismatch
		if let (Some(cached), Some(current)) = (&self.server_version, current_server_version) {
			if cached != current {
				return true;
			}
		}

		false
	}
}

/// OpenAPI schema cache with disk persistence.
pub struct OpenApiSchemaCache {
	client: Client,
	memory_cache: RwLock<HashMap<SchemaKey, Arc<ExtractedSchema>>>,
	semaphore: Arc<Semaphore>,
	cache_dir: Option<PathBuf>,
	/// Server version for cache invalidation.
	server_version: Option<String>,
}

impl OpenApiSchemaCache {
	const MAX_CONCURRENT_FETCHES: usize = 4;

	/// Create a cache with server version for invalidation.
	///
	/// The server version is used to invalidate cached schemas when the
	/// cluster is upgraded, ensuring CRD schema changes are picked up.
	pub fn new(client: Client, server_version: &str) -> Self {
		Self {
			client,
			memory_cache: RwLock::new(HashMap::new()),
			semaphore: Arc::new(Semaphore::new(Self::MAX_CONCURRENT_FETCHES)),
			cache_dir: dirs::cache_dir().map(|p| p.join("rtk").join("openapi")),
			server_version: Some(server_version.to_string()),
		}
	}

	/// Get extracted schema for a GVK including merge keys and type references.
	///
	/// This is used to pre-populate a `CombinedSchemaLookup` for strategic merge.
	/// Returns an empty schema if it cannot be fetched or if builtin schemas
	/// already cover this API group.
	#[instrument(skip(self))]
	pub async fn get_schema(&self, gvk: &GroupVersionKind) -> ExtractedSchema {
		// Map empty group to "core" for builtin lookup
		let group = if gvk.group.is_empty() {
			"core"
		} else {
			&gvk.group
		};

		// Skip fetching for standard K8s API groups - builtin schemas cover them
		if has_builtin_coverage(group, &gvk.version) {
			debug!("skipping OpenAPI fetch - builtins cover this API group/version");
			return ExtractedSchema::default();
		}

		match self.get_or_fetch(gvk).await {
			Ok(schema) => (*schema).clone(),
			Err(_) => ExtractedSchema::default(),
		}
	}

	#[instrument(skip(self))]
	async fn get_or_fetch(
		&self,
		gvk: &GroupVersionKind,
	) -> Result<Arc<ExtractedSchema>, OpenApiError> {
		let key = SchemaKey::from_gvk(gvk);

		// Check memory cache
		if let Some(schema) = self.memory_cache.read().await.get(&key) {
			debug!("cache hit (memory)");
			return Ok(schema.clone());
		}

		// Check disk cache
		if let Some(schema) = self.load_from_disk(&key)? {
			debug!("cache hit (disk)");
			let schema = Arc::new(schema);
			self.memory_cache.write().await.insert(key, schema.clone());
			return Ok(schema);
		}

		// Fetch from server (with semaphore for bounded parallelism)
		debug!("cache miss, fetching");
		let _permit = self.semaphore.acquire().await.expect("semaphore closed");

		let schema = self.fetch_from_server(&key).await?;
		let schema = Arc::new(schema);

		// Cache in memory
		self.memory_cache
			.write()
			.await
			.insert(key.clone(), schema.clone());

		// Cache to disk (best effort)
		if let Err(e) = self.save_to_disk(&key, &schema) {
			warn!(error = %e, "failed to write disk cache");
		}

		Ok(schema)
	}

	#[instrument(skip(self))]
	async fn fetch_from_server(&self, key: &SchemaKey) -> Result<ExtractedSchema, OpenApiError> {
		let path = key.api_path();
		debug!(path = %path, "fetching OpenAPI schema");

		let request = http::Request::get(&path)
			.body(vec![])
			.expect("valid request");

		let text = self
			.client
			.request_text(request)
			.await
			.map_err(|e| OpenApiError::Fetch {
				group: key.group.clone(),
				version: key.version.clone(),
				source: Box::new(e),
			})?;

		let doc: serde_json::Value = serde_json::from_str(&text).map_err(OpenApiError::Parse)?;

		Ok(extract_schema(&doc))
	}

	fn load_from_disk(&self, key: &SchemaKey) -> Result<Option<ExtractedSchema>, OpenApiError> {
		let Some(path) = self.disk_path(key) else {
			return Ok(None);
		};

		if !path.exists() {
			return Ok(None);
		}

		let content = fs::read_to_string(&path).map_err(OpenApiError::CacheRead)?;
		let cached: CachedSchema = serde_json::from_str(&content).map_err(OpenApiError::Parse)?;

		if cached.is_stale(self.server_version.as_deref()) {
			debug!(
				cached_version = ?cached.server_version,
				current_version = ?self.server_version,
				"cache stale (TTL expired or server version changed), will refetch"
			);
			let _ = fs::remove_file(&path);
			return Ok(None);
		}

		Ok(Some(cached.data))
	}

	fn save_to_disk(&self, key: &SchemaKey, schema: &ExtractedSchema) -> Result<(), OpenApiError> {
		let Some(path) = self.disk_path(key) else {
			return Ok(());
		};

		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).map_err(OpenApiError::CacheWrite)?;
		}

		let cached = CachedSchema::new(schema.clone(), self.server_version.clone());
		let content = serde_json::to_string(&cached).map_err(OpenApiError::Parse)?;
		fs::write(&path, content).map_err(OpenApiError::CacheWrite)?;

		debug!(path = %path.display(), "wrote disk cache");
		Ok(())
	}

	fn disk_path(&self, key: &SchemaKey) -> Option<PathBuf> {
		self.cache_dir
			.as_ref()
			.map(|d| d.join(key.cache_filename()))
	}
}

/// Extract schema info from OpenAPI v3 schema.
///
/// Returns merge keys and type references for path resolution.
fn extract_schema(doc: &serde_json::Value) -> ExtractedSchema {
	let mut result = ExtractedSchema::default();

	let Some(schemas) = doc
		.get("components")
		.and_then(|c| c.get("schemas"))
		.and_then(|s| s.as_object())
	else {
		return result;
	};

	for (type_name, type_schema) in schemas {
		let Some(properties) = type_schema.get("properties").and_then(|p| p.as_object()) else {
			continue;
		};

		for (field_name, field_schema) in properties {
			extract_field_info(&mut result, type_name, field_name, field_schema);
		}
	}

	result
}

/// Extract merge keys and type references from a field schema.
///
/// Recursively handles inline object schemas that don't have $ref.
fn extract_field_info(
	result: &mut ExtractedSchema,
	type_name: &str,
	field_name: &str,
	field_schema: &serde_json::Value,
) {
	extract_field_info_recursive(result, type_name, field_name, field_schema, 0);
}

/// Maximum recursion depth for inline schema extraction.
const MAX_INLINE_DEPTH: usize = 10;

/// Recursive implementation of field info extraction.
fn extract_field_info_recursive(
	result: &mut ExtractedSchema,
	type_name: &str,
	field_name: &str,
	field_schema: &serde_json::Value,
	depth: usize,
) {
	if depth > MAX_INLINE_DEPTH {
		return;
	}

	let key = format!("{}.{}", type_name, field_name);

	// Extract $ref for type resolution
	if let Some(ref_path) = get_ref_type(field_schema) {
		result.type_refs.insert(key.clone(), ref_path);
	}

	// Check for merge keys on the field itself
	if let Some(keys) = extract_merge_keys(field_schema) {
		result.merge_keys.insert(key.clone(), keys);
	}

	// Check for patch strategies on the field itself
	if let Some(strategies) = extract_patch_strategies(field_schema) {
		result.patch_strategies.insert(key.clone(), strategies);
	}

	// For array types, check items
	if let Some(items) = field_schema.get("items") {
		// Extract $ref from items
		if let Some(ref_path) = get_ref_type(items) {
			result.type_refs.insert(key.clone(), ref_path);
		}

		// Check items for merge keys
		if let Some(keys) = extract_merge_keys(items) {
			result.merge_keys.insert(key.clone(), keys);
		}

		// Recursively extract from inline object items
		if get_ref_type(items).is_none() {
			if let Some(item_props) = items.get("properties").and_then(|p| p.as_object()) {
				// Create synthetic type for inline array items
				let inline_type = format!("{}$items", key);
				result.type_refs.insert(key.clone(), inline_type.clone());

				for (nested_field, nested_schema) in item_props {
					extract_field_info_recursive(
						result,
						&inline_type,
						nested_field,
						nested_schema,
						depth + 1,
					);
				}
			}
		}
	}

	// For inline object fields (no $ref), recursively extract properties
	if get_ref_type(field_schema).is_none() {
		if let Some(props) = field_schema.get("properties").and_then(|p| p.as_object()) {
			// Create synthetic type for inline object
			let inline_type = format!("{}$inline", key);
			result.type_refs.insert(key, inline_type.clone());

			for (nested_field, nested_schema) in props {
				extract_field_info_recursive(
					result,
					&inline_type,
					nested_field,
					nested_schema,
					depth + 1,
				);
			}
		}
	}
}

/// Extract merge keys from a schema (field or items).
fn extract_merge_keys(schema: &serde_json::Value) -> Option<Vec<String>> {
	schema
		.get("x-kubernetes-list-map-keys")
		.and_then(|v| v.as_array())
		.map(|arr| {
			arr.iter()
				.filter_map(|v| v.as_str().map(String::from))
				.collect()
		})
		.or_else(|| {
			schema
				.get("x-kubernetes-patch-merge-key")
				.and_then(|v| v.as_str())
				.map(|s| vec![s.to_string()])
		})
}

/// Extract patch strategies from a schema field.
///
/// The `x-kubernetes-patch-strategy` can be:
/// - "merge" - merge strategy
/// - "replace" - replace strategy
/// - "retainKeys" - retain keys strategy
/// - "merge,retainKeys" - combined strategies
fn extract_patch_strategies(schema: &serde_json::Value) -> Option<Vec<PatchStrategy>> {
	schema
		.get("x-kubernetes-patch-strategy")
		.and_then(|v| v.as_str())
		.map(|s| s.split(',').filter_map(|s| s.trim().parse().ok()).collect())
}

/// Extract the referenced type name from a $ref.
fn get_ref_type(schema: &serde_json::Value) -> Option<String> {
	// Direct $ref
	if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
		return parse_ref(ref_str);
	}

	// $ref in items (for arrays)
	if let Some(items) = schema.get("items") {
		if let Some(ref_str) = items.get("$ref").and_then(|v| v.as_str()) {
			return parse_ref(ref_str);
		}
	}

	// allOf with $ref (common pattern)
	if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
		for item in all_of {
			if let Some(ref_str) = item.get("$ref").and_then(|v| v.as_str()) {
				return parse_ref(ref_str);
			}
		}
	}

	None
}

/// Parse a $ref string to extract the type name.
/// Format: "#/components/schemas/io.k8s.api.core.v1.PodSpec"
fn parse_ref(ref_str: &str) -> Option<String> {
	ref_str
		.strip_prefix("#/components/schemas/")
		.map(String::from)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_merge_keys_from_inline_objects() {
		// CRD with inline object items that have merge keys
		let doc = serde_json::json!({
			"components": {
				"schemas": {
					"com.example.v1.MyCRD": {
						"properties": {
							"spec": {
								"$ref": "#/components/schemas/com.example.v1.MyCRDSpec"
							}
						}
					},
					"com.example.v1.MyCRDSpec": {
						"properties": {
							"items": {
								"type": "array",
								"x-kubernetes-list-map-keys": ["name"],
								"items": {
									"type": "object",
									"properties": {
										"name": { "type": "string" },
										"nested": {
											"type": "array",
											"x-kubernetes-list-map-keys": ["id"],
											"items": {
												"type": "object",
												"properties": {
													"id": { "type": "string" }
												}
											}
										}
									}
								}
							}
						}
					}
				}
			}
		});

		let result = extract_schema(&doc);

		// Top-level items array should have merge key
		assert_eq!(
			result.merge_keys.get("com.example.v1.MyCRDSpec.items"),
			Some(&vec!["name".to_string()])
		);

		// Nested array within inline items should have merge key
		let items_type = result
			.type_refs
			.get("com.example.v1.MyCRDSpec.items")
			.expect("should have type ref for items");
		let nested_key = format!("{}.nested", items_type);
		assert_eq!(
			result.merge_keys.get(&nested_key),
			Some(&vec!["id".to_string()])
		);
	}

	#[test]
	fn test_extract_schema_with_refs() {
		// Standard CRD with $ref
		let doc = serde_json::json!({
			"components": {
				"schemas": {
					"com.example.v1.Parent": {
						"properties": {
							"items": {
								"type": "array",
								"x-kubernetes-list-map-keys": ["name"],
								"items": {
									"$ref": "#/components/schemas/com.example.v1.Item"
								}
							}
						}
					},
					"com.example.v1.Item": {
						"properties": {
							"name": { "type": "string" },
							"children": {
								"type": "array",
								"x-kubernetes-list-map-keys": ["id"],
								"items": {
									"$ref": "#/components/schemas/com.example.v1.Child"
								}
							}
						}
					},
					"com.example.v1.Child": {
						"properties": {
							"id": { "type": "string" }
						}
					}
				}
			}
		});

		let result = extract_schema(&doc);

		// Parent.items should have merge key and type ref
		assert_eq!(
			result.merge_keys.get("com.example.v1.Parent.items"),
			Some(&vec!["name".to_string()])
		);
		assert_eq!(
			result.type_refs.get("com.example.v1.Parent.items"),
			Some(&"com.example.v1.Item".to_string())
		);

		// Item.children should have merge key and type ref
		assert_eq!(
			result.merge_keys.get("com.example.v1.Item.children"),
			Some(&vec!["id".to_string()])
		);
		assert_eq!(
			result.type_refs.get("com.example.v1.Item.children"),
			Some(&"com.example.v1.Child".to_string())
		);
	}

	#[test]
	fn test_cache_staleness_ttl() {
		let cached = CachedSchema {
			data: ExtractedSchema::default(),
			cached_at: 0, // Very old
			server_version: Some("v1.28.0".to_string()),
		};
		assert!(cached.is_stale(Some("v1.28.0")));
	}

	#[test]
	fn test_cache_staleness_version_change() {
		let now = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_secs();
		let cached = CachedSchema {
			data: ExtractedSchema::default(),
			cached_at: now,
			server_version: Some("v1.28.0".to_string()),
		};
		// Same version - not stale
		assert!(!cached.is_stale(Some("v1.28.0")));
		// Different version - stale
		assert!(cached.is_stale(Some("v1.29.0")));
	}
}
