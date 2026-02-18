//! Schema lookup for strategic merge patch operations.
//!
//! This module provides traits and implementations for looking up merge key
//! information for Kubernetes resource fields.

use super::{
	builtin_schemas::{self, MergeType},
	types::PatchStrategy,
};

/// Merge keys that can be either static strings or dynamically allocated.
///
/// This avoids allocation for builtin schemas (which use `&'static [&'static str]`)
/// while supporting OpenAPI-fetched data (which uses `&[String]`).
#[derive(Debug, Clone)]
pub enum MergeKeys<'a> {
	/// Static merge keys from builtin schemas.
	Static(&'static [&'static str]),
	/// Borrowed merge keys from OpenAPI cache.
	Borrowed(&'a [String]),
}

impl<'a> MergeKeys<'a> {
	/// Get the number of merge keys.
	pub fn len(&self) -> usize {
		match self {
			MergeKeys::Static(keys) => keys.len(),
			MergeKeys::Borrowed(keys) => keys.len(),
		}
	}

	/// Check if empty.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

/// Iterator over merge key names.
pub enum MergeKeysIter<'a> {
	/// Iterator over static string slices.
	Static(std::slice::Iter<'static, &'static str>),
	/// Iterator over borrowed Strings.
	Borrowed(std::slice::Iter<'a, String>),
}

impl<'a> Iterator for MergeKeysIter<'a> {
	type Item = &'a str;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			MergeKeysIter::Static(iter) => iter.next().copied(),
			MergeKeysIter::Borrowed(iter) => iter.next().map(|s| s.as_str()),
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		match self {
			MergeKeysIter::Static(iter) => iter.size_hint(),
			MergeKeysIter::Borrowed(iter) => iter.size_hint(),
		}
	}
}

impl<'a> ExactSizeIterator for MergeKeysIter<'a> {}

impl<'a> IntoIterator for &'a MergeKeys<'a> {
	type Item = &'a str;
	type IntoIter = MergeKeysIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		match self {
			MergeKeys::Static(keys) => MergeKeysIter::Static(keys.iter()),
			MergeKeys::Borrowed(keys) => MergeKeysIter::Borrowed(keys.iter()),
		}
	}
}

/// Trait for looking up strategic merge patch schema information.
pub trait SchemaLookup {
	/// Get the merge keys for an array field at the given path.
	///
	/// # Arguments
	/// * `path` - Dot-separated JSON path (e.g., "spec.template.spec.containers")
	///
	/// # Returns
	/// The merge key names (e.g., `["name"]` for containers, `["port", "protocol"]` for Service ports)
	/// if this field uses strategic merge by key, or `None` if the field should be treated atomically.
	fn get_merge_keys(&self, path: &str) -> Option<MergeKeys<'_>>;

	/// Get the patch strategies for a field at the given path.
	///
	/// # Arguments
	/// * `path` - Dot-separated JSON path
	///
	/// # Returns
	/// The patch strategies for this field, or an empty slice if not specified.
	fn get_patch_strategies(&self, path: &str) -> &[PatchStrategy] {
		let _ = path;
		&[]
	}

	/// Check if a field uses the retainKeys strategy.
	fn has_retain_keys(&self, path: &str) -> bool {
		self.get_patch_strategies(path)
			.contains(&PatchStrategy::RetainKeys)
	}

	/// Get the schema type for a nested field.
	///
	/// This is used when traversing into nested objects to determine their schema type.
	fn get_nested_type(&self, parent_type: &str, field: &str) -> Option<String>;
}

/// Schema lookup using built-in Kubernetes type schemas.
///
/// This uses the hardcoded merge key information extracted from Kubernetes API
/// type definitions.
#[derive(Debug)]
pub struct BuiltinSchemaLookup {
	/// The OpenAPI schema type for the root resource.
	root_type: String,
	/// The kind of the resource.
	kind: String,
	/// The merge type (SSA vs SMP) to use for lookups.
	merge_type: MergeType,
}

impl BuiltinSchemaLookup {
	/// Create a new schema lookup for a resource.
	///
	/// Uses `StrategicMergePatch` merge keys by default (matches kubectl behavior).
	pub fn new(api_version: &str, kind: &str) -> Self {
		Self::with_merge_type(api_version, kind, MergeType::default())
	}

	/// Create a new schema lookup with a specific merge type.
	pub fn with_merge_type(api_version: &str, kind: &str, merge_type: MergeType) -> Self {
		let root_type = builtin_schemas::gvk_to_schema_type(api_version, kind);
		Self {
			root_type,
			kind: kind.to_string(),
			merge_type,
		}
	}

	/// Create a schema lookup for a specific OpenAPI type.
	///
	/// This is useful for testing or when you know the exact schema type.
	/// Uses `StrategicMergePatch` merge keys by default.
	pub fn for_type(schema_type: &str) -> Self {
		Self {
			root_type: schema_type.to_string(),
			kind: String::new(),
			merge_type: MergeType::default(),
		}
	}

	/// Create a schema lookup for a specific OpenAPI type with a specific merge type.
	pub fn for_type_with_merge_type(schema_type: &str, merge_type: MergeType) -> Self {
		Self {
			root_type: schema_type.to_string(),
			kind: String::new(),
			merge_type,
		}
	}

	/// Resolve a JSON path to a schema type.
	///
	/// This handles the mapping from JSON paths to OpenAPI schema types,
	/// including nested types like PodTemplateSpec.spec -> PodSpec.
	fn resolve_schema_type(&self, path: &str) -> String {
		// Common path patterns for workload resources
		// Deployment/StatefulSet/DaemonSet/ReplicaSet: spec.template.spec -> PodSpec
		// Job: spec.template.spec -> PodSpec
		// CronJob: spec.jobTemplate.spec.template.spec -> PodSpec
		// Pod: spec -> PodSpec

		let _path_parts: Vec<&str> = path.split('.').collect();

		// Check for PodSpec patterns
		if path.ends_with("spec.template.spec") || path == "spec.template.spec" {
			return "io.k8s.api.core.v1.PodSpec".to_string();
		}
		if path.starts_with("spec.template.spec.") {
			let remainder = path.strip_prefix("spec.template.spec.").unwrap();
			return self.resolve_podspec_path(remainder);
		}

		// Pod direct spec
		if self.kind == "Pod" && path == "spec" {
			return "io.k8s.api.core.v1.PodSpec".to_string();
		}
		if self.kind == "Pod" && path.starts_with("spec.") {
			let remainder = path.strip_prefix("spec.").unwrap();
			return self.resolve_podspec_path(remainder);
		}

		// CronJob pattern
		if path.ends_with("spec.jobTemplate.spec.template.spec") {
			return "io.k8s.api.core.v1.PodSpec".to_string();
		}
		if path.starts_with("spec.jobTemplate.spec.template.spec.") {
			let remainder = path
				.strip_prefix("spec.jobTemplate.spec.template.spec.")
				.unwrap();
			return self.resolve_podspec_path(remainder);
		}

		// Service spec
		if self.kind == "Service" && path == "spec" {
			return "io.k8s.api.core.v1.ServiceSpec".to_string();
		}
		if self.kind == "Service" && path.starts_with("spec.") {
			return "io.k8s.api.core.v1.ServiceSpec".to_string();
		}

		// Default to root type with path
		self.root_type.clone()
	}

	/// Resolve a path to (schema_type, field_name) for lookups.
	fn resolve_path<'a>(&self, path: &'a str) -> Option<(String, &'a str)> {
		let field_name = path.rsplit('.').next()?;
		let parent_path = path.rfind('.').map(|pos| &path[..pos]).unwrap_or("");

		let schema_type = if parent_path.is_empty() {
			self.root_type.clone()
		} else {
			self.resolve_schema_type(parent_path)
		};

		Some((schema_type, field_name))
	}

	/// Resolve a path within PodSpec to the appropriate schema type.
	///
	/// When used by `get_merge_key`, the path is the parent path (everything except
	/// the field name). So if we're looking up `containers.env`, the path is `containers`
	/// and we need to return Container (the type that contains `env`).
	fn resolve_podspec_path(&self, path: &str) -> String {
		// When path ends with a container array field, return the element type
		// because we're looking for the schema that contains child fields
		if path == "containers" || path == "initContainers" {
			return "io.k8s.api.core.v1.Container".to_string();
		}
		if path == "ephemeralContainers" {
			return "io.k8s.api.core.v1.EphemeralContainer".to_string();
		}

		// Nested within containers (e.g., containers.mycontainer.env)
		if path.starts_with("containers.") || path.starts_with("initContainers.") {
			return "io.k8s.api.core.v1.Container".to_string();
		}
		if path.starts_with("ephemeralContainers.") {
			return "io.k8s.api.core.v1.EphemeralContainer".to_string();
		}

		"io.k8s.api.core.v1.PodSpec".to_string()
	}
}

impl SchemaLookup for BuiltinSchemaLookup {
	fn get_merge_keys(&self, path: &str) -> Option<MergeKeys<'_>> {
		let (schema_type, field_name) = self.resolve_path(path)?;
		builtin_schemas::get_merge_keys(&schema_type, field_name, self.merge_type)
			.map(MergeKeys::Static)
	}

	fn get_patch_strategies(&self, path: &str) -> &[PatchStrategy] {
		let Some((schema_type, field_name)) = self.resolve_path(path) else {
			return &[];
		};
		builtin_schemas::get_patch_strategies(&schema_type, field_name).unwrap_or(&[])
	}

	fn get_nested_type(&self, parent_type: &str, field: &str) -> Option<String> {
		builtin_schemas::get_embedded_schema(parent_type, field).map(|s| s.to_string())
	}
}

/// A schema lookup that combines multiple sources.
///
/// This tries the built-in schemas first, then falls back to pre-fetched
/// OpenAPI schema data with path-to-type resolution.
pub struct CombinedSchemaLookup {
	builtin: BuiltinSchemaLookup,
	/// Root schema type for this resource (e.g., "io.k8s.api.apps.v1.Deployment").
	root_type: String,
	/// Pre-fetched merge key data from OpenAPI.
	/// Map of "type.field" -> merge_keys (supports composite keys)
	openapi_merge_keys: std::collections::HashMap<String, Vec<String>>,
	/// Pre-fetched patch strategy data from OpenAPI.
	/// Map of "type.field" -> patch strategies (parsed from x-kubernetes-patch-strategy)
	openapi_patch_strategies: std::collections::HashMap<String, Vec<PatchStrategy>>,
	/// Type references from OpenAPI.
	/// Map of "type.field" -> referenced type name for path resolution.
	openapi_type_refs: std::collections::HashMap<String, String>,
}

impl CombinedSchemaLookup {
	/// Create a new combined schema lookup with only built-in schemas.
	pub fn new(api_version: &str, kind: &str) -> Self {
		let root_type = builtin_schemas::gvk_to_schema_type(api_version, kind);
		Self {
			builtin: BuiltinSchemaLookup::new(api_version, kind),
			root_type,
			openapi_merge_keys: std::collections::HashMap::new(),
			openapi_patch_strategies: std::collections::HashMap::new(),
			openapi_type_refs: std::collections::HashMap::new(),
		}
	}

	/// Create a combined schema lookup with pre-fetched OpenAPI data.
	///
	/// # Arguments
	/// * `api_version` - The API version of the resource
	/// * `kind` - The kind of the resource
	/// * `openapi_merge_keys` - Map of "type.field" -> merge_keys from OpenAPI
	/// * `openapi_patch_strategies` - Map of "type.field" -> strategies from OpenAPI
	/// * `openapi_type_refs` - Map of "type.field" -> referenced type for path resolution
	pub fn with_openapi_data(
		api_version: &str,
		kind: &str,
		openapi_merge_keys: std::collections::HashMap<String, Vec<String>>,
		openapi_patch_strategies: std::collections::HashMap<String, Vec<PatchStrategy>>,
		openapi_type_refs: std::collections::HashMap<String, String>,
	) -> Self {
		let root_type = builtin_schemas::gvk_to_schema_type(api_version, kind);
		Self {
			builtin: BuiltinSchemaLookup::new(api_version, kind),
			root_type,
			openapi_merge_keys,
			openapi_patch_strategies,
			openapi_type_refs,
		}
	}

	/// Look up merge keys for a type.field pair.
	fn lookup_merge_keys(&self, type_name: &str, field: &str) -> Option<&[String]> {
		let key = format!("{}.{}", type_name, field);
		self.openapi_merge_keys.get(&key).map(|v| v.as_slice())
	}

	/// Look up patch strategies for a type.field pair.
	fn lookup_patch_strategies(&self, type_name: &str, field: &str) -> Option<&[PatchStrategy]> {
		let key = format!("{}.{}", type_name, field);
		self.openapi_patch_strategies
			.get(&key)
			.map(|v| v.as_slice())
	}

	/// Look up the referenced type for a type.field pair.
	fn lookup_type_ref(&self, type_name: &str, field: &str) -> Option<String> {
		let key = format!("{}.{}", type_name, field);
		self.openapi_type_refs.get(&key).cloned()
	}

	/// Resolve a path to a (parent_type, field) pair using the type reference chain.
	///
	/// For path "spec.template.spec.containers" and root type "Deployment":
	/// 1. Look up "Deployment.spec" -> "DeploymentSpec"
	/// 2. Look up "DeploymentSpec.template" -> "PodTemplateSpec"
	/// 3. Look up "PodTemplateSpec.spec" -> "PodSpec"
	/// 4. Return ("PodSpec", "containers")
	///
	/// For paths that traverse INTO array elements (e.g., "spec.items.myitem.nested"
	/// where "items" is an array with merge key and "myitem" is a key value):
	/// 1. Look up "Root.spec" -> "Spec"
	/// 2. Look up "Spec.items" -> "Item" (the array item type)
	/// 3. "myitem" has no type ref in Item, so we stay at Item type
	/// 4. Look up "Item.nested" -> return ("Item", "nested")
	fn resolve_path(&self, path: &str) -> Option<(String, String)> {
		let parts: Vec<&str> = path.split('.').collect();
		if parts.is_empty() {
			return None;
		}

		let mut current_type = self.root_type.clone();

		// Walk through all but the last segment to find the parent type
		for &segment in &parts[..parts.len() - 1] {
			// Try to look up the type ref for this segment
			if let Some(next_type) = self.lookup_type_ref(&current_type, segment) {
				current_type = next_type;
			}
			// If lookup fails, this segment might be an array element identifier
			// (merge key value like "mycontainer" in "containers.mycontainer.env").
			// In that case, stay at the current type (which should be the array item type
			// from the previous segment's lookup).
		}

		let field = parts.last()?.to_string();
		Some((current_type, field))
	}
}

impl SchemaLookup for CombinedSchemaLookup {
	fn get_merge_keys(&self, path: &str) -> Option<MergeKeys<'_>> {
		// Try builtin first
		if let Some(keys) = self.builtin.get_merge_keys(path) {
			return Some(keys);
		}

		// Fall back to pre-fetched OpenAPI data with path resolution
		let (parent_type, field) = self.resolve_path(path)?;
		self.lookup_merge_keys(&parent_type, &field)
			.map(MergeKeys::Borrowed)
	}

	fn get_patch_strategies(&self, path: &str) -> &[PatchStrategy] {
		// Try builtin first
		let builtin_strategies = self.builtin.get_patch_strategies(path);
		if !builtin_strategies.is_empty() {
			return builtin_strategies;
		}

		// Fall back to pre-fetched OpenAPI data with path resolution
		let Some((parent_type, field)) = self.resolve_path(path) else {
			return &[];
		};
		self.lookup_patch_strategies(&parent_type, &field)
			.unwrap_or(&[])
	}

	fn get_nested_type(&self, parent_type: &str, field: &str) -> Option<String> {
		// Try builtin first
		if let Some(t) = self.builtin.get_nested_type(parent_type, field) {
			return Some(t);
		}

		// Fall back to OpenAPI type refs
		self.lookup_type_ref(parent_type, field)
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use rstest::rstest;

	use super::*;

	#[rstest]
	// Default merge type is StrategicMergePatch, so ports use single key
	#[case("apps/v1", "Deployment", "spec.template.spec.containers", Some(vec!["name"]))]
	#[case("apps/v1", "Deployment", "spec.template.spec.volumes", Some(vec!["name"]))]
	#[case("apps/v1", "Deployment", "spec.template.spec.containers.env", Some(vec!["name"]))]
	#[case("apps/v1", "Deployment", "spec.template.spec.containers.ports", Some(vec!["containerPort"]))]
	#[case("apps/v1", "Deployment", "spec.template.spec.containers.volumeMounts", Some(vec!["mountPath"]))]
	#[case("apps/v1", "Deployment", "spec.template.spec.initContainers", Some(vec!["name"]))]
	#[case("v1", "Pod", "spec.containers", Some(vec!["name"]))]
	#[case("v1", "Pod", "spec.volumes", Some(vec!["name"]))]
	#[case("v1", "Service", "spec.ports", Some(vec!["port"]))]
	#[case("apps/v1", "Deployment", "spec.replicas", None)]
	#[case("apps/v1", "Deployment", "metadata.name", None)]
	fn test_builtin_lookup(
		#[case] api_version: &str,
		#[case] kind: &str,
		#[case] path: &str,
		#[case] expected: Option<Vec<&str>>,
	) {
		let lookup = BuiltinSchemaLookup::new(api_version, kind);
		let result = lookup.get_merge_keys(path);
		let result_vec: Option<Vec<&str>> = result.as_ref().map(|k| k.into_iter().collect());
		assert_eq!(result_vec, expected);
	}

	#[test]
	fn test_builtin_lookup_ssa_vs_smp() {
		// Test that SSA and SMP return different keys for ports
		let smp_lookup = BuiltinSchemaLookup::new("v1", "Service");
		let ssa_lookup =
			BuiltinSchemaLookup::with_merge_type("v1", "Service", MergeType::ServerSideApply);

		let smp_keys = smp_lookup.get_merge_keys("spec.ports");
		let ssa_keys = ssa_lookup.get_merge_keys("spec.ports");

		assert_eq!(
			smp_keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["port"])
		);
		assert_eq!(
			ssa_keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["port", "protocol"])
		);
	}

	#[test]
	fn test_combined_lookup_with_openapi_data() {
		// Simulate a CRD: MyCRD with spec.items (array by "name") containing nested children (array by "id")
		// Note: gvk_to_schema_type("example.com/v1", "MyCRD") produces "io.k8s.api.example_com.v1.MyCRD"
		let root_type = "io.k8s.api.example_com.v1.MyCRD";

		let mut merge_keys = HashMap::new();
		merge_keys.insert(
			"io.k8s.api.example_com.v1.MyCRDSpec.items".to_string(),
			vec!["name".to_string()],
		);
		merge_keys.insert(
			"io.k8s.api.example_com.v1.Item.children".to_string(),
			vec!["id".to_string()],
		);

		let mut type_refs = HashMap::new();
		type_refs.insert(
			format!("{}.spec", root_type),
			"io.k8s.api.example_com.v1.MyCRDSpec".to_string(),
		);
		type_refs.insert(
			"io.k8s.api.example_com.v1.MyCRDSpec.items".to_string(),
			"io.k8s.api.example_com.v1.Item".to_string(),
		);
		type_refs.insert(
			"io.k8s.api.example_com.v1.Item.children".to_string(),
			"io.k8s.api.example_com.v1.Child".to_string(),
		);

		let lookup = CombinedSchemaLookup::with_openapi_data(
			"example.com/v1",
			"MyCRD",
			merge_keys,
			HashMap::new(),
			type_refs,
		);

		// Test: spec.items should have merge key "name"
		let keys = lookup.get_merge_keys("spec.items");
		assert_eq!(
			keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["name"])
		);

		// Test: spec.items.myitem.children should have merge key "id"
		// This tests the array element path handling - "myitem" is a merge key value
		let keys = lookup.get_merge_keys("spec.items.myitem.children");
		assert_eq!(
			keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["id"])
		);

		// Test: spec.items.anotheritem.children should also work with different key value
		let keys = lookup.get_merge_keys("spec.items.anotheritem.children");
		assert_eq!(
			keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["id"])
		);
	}

	#[test]
	fn test_combined_lookup_falls_back_to_builtin() {
		// CombinedSchemaLookup should fall back to builtin for standard K8s types
		let lookup = CombinedSchemaLookup::new("apps/v1", "Deployment");

		// These should work via builtin fallback
		let keys = lookup.get_merge_keys("spec.template.spec.containers");
		assert_eq!(
			keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["name"])
		);

		let keys = lookup.get_merge_keys("spec.template.spec.containers.env");
		assert_eq!(
			keys.as_ref().map(|k| k.into_iter().collect::<Vec<_>>()),
			Some(vec!["name"])
		);
	}
}
