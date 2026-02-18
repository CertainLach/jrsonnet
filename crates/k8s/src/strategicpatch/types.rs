//! Core types for strategic merge patch operations.
//!
//! These types model Kubernetes strategic merge patch metadata from OpenAPI schemas.

use std::{collections::HashMap, str::FromStr};

/// Patch strategy for a field.
///
/// This controls how the field is merged during strategic merge patch operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchStrategy {
	/// Merge strategy: recursively merge objects, merge arrays by merge key.
	#[default]
	Merge,
	/// Replace strategy: replace the entire value.
	Replace,
	/// RetainKeys strategy: for maps, only keep keys present in the patch.
	RetainKeys,
}

/// Error returned when parsing an unknown patch strategy string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown patch strategy: {0}")]
pub struct UnknownPatchStrategy(pub String);

impl FromStr for PatchStrategy {
	type Err = UnknownPatchStrategy;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"merge" => Ok(PatchStrategy::Merge),
			"replace" => Ok(PatchStrategy::Replace),
			"retainKeys" => Ok(PatchStrategy::RetainKeys),
			_ => Err(UnknownPatchStrategy(s.to_string())),
		}
	}
}

/// Patch metadata for a field.
///
/// Contains the patch strategy and optional merge key(s) for arrays.
#[derive(Debug, Clone, Default)]
pub struct PatchMeta {
	/// The patch strategies for this field. Multiple strategies can be combined.
	pub strategies: Vec<PatchStrategy>,
	/// The merge key(s) for array elements.
	/// Single key: e.g., `["name"]` for containers
	/// Composite key: e.g., `["port", "protocol"]` for Service ports
	pub merge_keys: Option<Vec<String>>,
}

impl PatchMeta {
	/// Create a new PatchMeta with merge strategy and a single merge key.
	pub fn merge_with_key(key: impl Into<String>) -> Self {
		Self {
			strategies: vec![PatchStrategy::Merge],
			merge_keys: Some(vec![key.into()]),
		}
	}

	/// Create a new PatchMeta with merge strategy and composite merge keys.
	pub fn merge_with_keys(keys: Vec<String>) -> Self {
		Self {
			strategies: vec![PatchStrategy::Merge],
			merge_keys: Some(keys),
		}
	}

	/// Create a new PatchMeta with merge strategy (no merge key).
	pub fn merge() -> Self {
		Self {
			strategies: vec![PatchStrategy::Merge],
			merge_keys: None,
		}
	}

	/// Create a new PatchMeta with replace strategy.
	pub fn replace() -> Self {
		Self {
			strategies: vec![PatchStrategy::Replace],
			merge_keys: None,
		}
	}

	/// Check if this field uses merge strategy.
	pub fn is_merge(&self) -> bool {
		self.strategies.contains(&PatchStrategy::Merge)
	}

	/// Check if this field uses replace strategy.
	pub fn is_replace(&self) -> bool {
		self.strategies.contains(&PatchStrategy::Replace)
	}

	/// Check if this field has merge key(s) (for array merging).
	pub fn has_merge_key(&self) -> bool {
		self.merge_keys.as_ref().is_some_and(|k| !k.is_empty())
	}
}

/// Schema information for a field.
///
/// This contains the patch metadata and nested field schemas.
#[derive(Debug, Clone, Default)]
pub struct FieldSchema {
	/// Patch metadata for this field.
	pub patch_meta: PatchMeta,
	/// Nested field schemas for object types.
	pub properties: Option<HashMap<String, FieldSchema>>,
	/// Schema for array items.
	pub items: Option<Box<FieldSchema>>,
}

impl FieldSchema {
	/// Create a new field schema with the given patch meta.
	pub fn new(patch_meta: PatchMeta) -> Self {
		Self {
			patch_meta,
			properties: None,
			items: None,
		}
	}

	/// Create a field schema for an array with merge key.
	pub fn array_with_merge_key(key: impl Into<String>, items: FieldSchema) -> Self {
		Self {
			patch_meta: PatchMeta::merge_with_key(key),
			properties: None,
			items: Some(Box::new(items)),
		}
	}

	/// Create a field schema for an object with properties.
	pub fn object(properties: HashMap<String, FieldSchema>) -> Self {
		Self {
			patch_meta: PatchMeta::default(),
			properties: Some(properties),
			items: None,
		}
	}

	/// Get the patch meta for this field.
	pub fn patch_meta(&self) -> &PatchMeta {
		&self.patch_meta
	}

	/// Get a nested field schema by name.
	pub fn get_field(&self, name: &str) -> Option<&FieldSchema> {
		self.properties.as_ref()?.get(name)
	}

	/// Get the item schema for array types.
	pub fn item_schema(&self) -> Option<&FieldSchema> {
		self.items.as_deref()
	}
}

/// Type schema for a Kubernetes resource type.
///
/// Contains field schemas for all fields in the resource.
#[derive(Debug, Clone, Default)]
pub struct TypeSchema {
	/// Fields at the top level of the resource.
	pub fields: HashMap<String, FieldSchema>,
}

impl TypeSchema {
	/// Create a new type schema with the given fields.
	pub fn new(fields: HashMap<String, FieldSchema>) -> Self {
		Self { fields }
	}

	/// Get a field schema by path (e.g., "spec.containers").
	pub fn get_field_by_path(&self, path: &[&str]) -> Option<&FieldSchema> {
		if path.is_empty() {
			return None;
		}

		let mut current = self.fields.get(path[0])?;
		for segment in &path[1..] {
			current = current.get_field(segment)?;
		}
		Some(current)
	}

	/// Get the patch meta for a field path.
	pub fn get_patch_meta(&self, path: &[&str]) -> Option<&PatchMeta> {
		self.get_field_by_path(path).map(|f| &f.patch_meta)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_patch_meta_merge_with_key() {
		let meta = PatchMeta::merge_with_key("name");
		assert!(meta.is_merge());
		assert!(!meta.is_replace());
		assert!(meta.has_merge_key());
		assert_eq!(meta.merge_keys, Some(vec!["name".to_string()]));
	}

	#[test]
	fn test_patch_meta_merge_with_composite_keys() {
		let meta = PatchMeta::merge_with_keys(vec!["port".to_string(), "protocol".to_string()]);
		assert!(meta.is_merge());
		assert!(meta.has_merge_key());
		assert_eq!(
			meta.merge_keys,
			Some(vec!["port".to_string(), "protocol".to_string()])
		);
	}

	#[test]
	fn test_type_schema_get_field_by_path() {
		let mut container_fields = HashMap::new();
		container_fields.insert(
			"env".to_string(),
			FieldSchema::new(PatchMeta::merge_with_key("name")),
		);

		let container_schema = FieldSchema::object(container_fields);

		let mut spec_fields = HashMap::new();
		spec_fields.insert(
			"containers".to_string(),
			FieldSchema::array_with_merge_key("name", container_schema),
		);

		let spec_schema = FieldSchema::object(spec_fields);

		let mut root_fields = HashMap::new();
		root_fields.insert("spec".to_string(), spec_schema);

		let schema = TypeSchema::new(root_fields);

		// Test getting containers field
		let containers = schema.get_field_by_path(&["spec", "containers"]);
		assert!(containers.is_some());
		assert!(containers.unwrap().patch_meta.has_merge_key());
		assert_eq!(
			containers.unwrap().patch_meta.merge_keys,
			Some(vec!["name".to_string()])
		);
	}
}
