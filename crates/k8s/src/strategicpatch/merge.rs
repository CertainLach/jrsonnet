//! Patch merging for strategic merge patch operations.
//!
//! This module handles merging multiple patches together, which is needed
//! for the three-way merge algorithm.

use serde_json::{Map, Value};

use super::{
	diff::DiffError,
	directives::{
		extract_delete_from_primitive_list_field, extract_set_element_order_field, is_directive,
		DIRECTIVE_RETAIN_KEYS,
	},
	schema::{MergeKeys, SchemaLookup},
};

/// Options controlling merge behavior.
#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
	/// If true, null values in the patch that don't match existing fields in the
	/// original are discarded. If false, they are propagated to the result.
	///
	/// Server-side merges typically use `true` (ignore unmatched nulls).
	/// Patch merging (combining patches) typically uses `false` (preserve nulls).
	pub ignore_unmatched_nulls: bool,
	/// If true, merge parallel lists using the parallel list to delete items.
	/// This is used server-side when applying patches.
	pub merge_parallel_list: bool,
}

/// Merge two patches together.
///
/// This combines a deletion patch with a delta patch to produce a final patch.
/// Deletions (null values) from the first patch are preserved, and additions/changes
/// from the second patch are layered on top.
///
/// # Arguments
/// * `deletions` - Patch containing deletion markers (null values)
/// * `delta` - Patch containing additions and changes
/// * `schema` - Schema lookup for merge key information
/// * `path` - Current JSON path (for schema lookups)
pub fn merge_patches(
	deletions: &Value,
	delta: &Value,
	schema: &dyn SchemaLookup,
	path: &str,
) -> Result<Value, DiffError> {
	match (deletions, delta) {
		(Value::Object(del_map), Value::Object(delta_map)) => {
			merge_patch_maps(del_map, delta_map, schema, path)
		}
		(Value::Array(del_arr), Value::Array(delta_arr)) => {
			merge_patch_arrays(del_arr, delta_arr, schema, path)
		}
		// If delta has a value, prefer it
		(_, delta) if !is_empty_value(delta) => Ok(delta.clone()),
		// Otherwise use deletions
		(deletions, _) => Ok(deletions.clone()),
	}
}

/// Merge two patch objects.
fn merge_patch_maps(
	deletions: &Map<String, Value>,
	delta: &Map<String, Value>,
	schema: &dyn SchemaLookup,
	path: &str,
) -> Result<Value, DiffError> {
	let mut result = Map::new();

	// First, include all deletions
	for (key, value) in deletions {
		let Some(delta_value) = delta.get(key) else {
			// Only in deletions
			result.insert(key.clone(), value.clone());
			continue;
		};

		// Key exists in both - merge recursively
		let field_path = if path.is_empty() {
			key.clone()
		} else {
			format!("{}.{}", path, key)
		};

		let merged = merge_patches(value, delta_value, schema, &field_path)?;
		if !is_empty_value(&merged) {
			result.insert(key.clone(), merged);
		}
	}

	// Then, include additions from delta that aren't in deletions
	for (key, value) in delta {
		if !deletions.contains_key(key) {
			result.insert(key.clone(), value.clone());
		}
	}

	Ok(Value::Object(result))
}

/// Merge two patch arrays.
///
/// For strategic merge patch arrays, items with matching merge keys need to be merged.
/// Items only in delta are kept as-is. Items only in deletions that are delete directives
/// are appended.
fn merge_patch_arrays(
	deletions: &[Value],
	delta: &[Value],
	schema: &dyn SchemaLookup,
	path: &str,
) -> Result<Value, DiffError> {
	let merge_keys = schema.get_merge_keys(path);

	// If we have merge keys, try to merge items by key
	if let Some(ref keys) = merge_keys {
		return merge_patch_arrays_by_key(deletions, delta, schema, path, keys);
	}

	// No merge keys - just concatenate delta with delete directives
	let mut result = Vec::new();
	for item in delta {
		result.push(item.clone());
	}
	for item in deletions {
		if is_delete_directive(item) {
			result.push(item.clone());
		}
	}
	Ok(Value::Array(result))
}

/// Merge patch arrays using merge keys to match items.
fn merge_patch_arrays_by_key(
	deletions: &[Value],
	delta: &[Value],
	schema: &dyn SchemaLookup,
	path: &str,
	merge_keys: &MergeKeys<'_>,
) -> Result<Value, DiffError> {
	use std::collections::HashMap;

	// Index deletion items by merge key
	let deletion_index: HashMap<String, &Value> = deletions
		.iter()
		.filter_map(|item| get_merge_key_value(item, merge_keys).map(|key| (key, item)))
		.collect();

	let mut result = Vec::new();

	// Process delta items, merging with matching deletion items
	for delta_item in delta {
		let Some(key_value) = get_merge_key_value(delta_item, merge_keys) else {
			result.push(delta_item.clone());
			continue;
		};

		// Check if there's a matching deletion item with directives to merge
		if let Some(deletion_item) = deletion_index.get(&key_value) {
			// Merge the two items
			let item_path = format!("{}.{}", path, key_value);
			let merged = merge_patches(deletion_item, delta_item, schema, &item_path)?;
			result.push(merged);
		} else {
			result.push(delta_item.clone());
		}
	}

	// Add delete directives that don't have matching delta items
	let delta_keys: std::collections::HashSet<String> = delta
		.iter()
		.filter_map(|item| get_merge_key_value(item, merge_keys))
		.collect();

	for item in deletions {
		if is_delete_directive(item) {
			// Check if this delete directive has a matching delta item
			let key = get_merge_key_value(item, merge_keys);
			if key.map(|k| !delta_keys.contains(&k)).unwrap_or(true) {
				result.push(item.clone());
			}
		}
	}

	Ok(Value::Array(result))
}

/// Get the merge key value from an item.
fn get_merge_key_value(item: &Value, merge_keys: &MergeKeys<'_>) -> Option<String> {
	let obj = item.as_object()?;
	let mut parts = Vec::new();
	for key in merge_keys {
		let value = obj.get(key)?.as_str()?;
		parts.push(value.to_string());
	}
	Some(parts.join("::"))
}

/// Check if a value is empty (for patch purposes).
fn is_empty_value(value: &Value) -> bool {
	match value {
		Value::Object(map) => map.is_empty(),
		Value::Array(arr) => arr.is_empty(),
		Value::Null => false, // Null is a deletion marker, not empty
		_ => false,
	}
}

/// Check if a value is a delete directive.
fn is_delete_directive(value: &Value) -> bool {
	value
		.as_object()
		.and_then(|obj| obj.get("$patch"))
		.and_then(|v| v.as_str())
		.map(|s| s == "delete")
		.unwrap_or(false)
}

/// Apply a strategic merge patch to a base value.
///
/// This is used by the mock server to apply patches received from clients.
/// Uses default options (ignore_unmatched_nulls=true, merge_parallel_list=true).
///
/// # Arguments
/// * `base` - The current value
/// * `patch` - The patch to apply
/// * `schema` - Schema lookup for merge key information
/// * `path` - Current JSON path
pub fn apply_strategic_patch(
	base: &Value,
	patch: &Value,
	schema: &dyn SchemaLookup,
	path: &str,
) -> Result<Value, DiffError> {
	let options = MergeOptions {
		ignore_unmatched_nulls: true,
		merge_parallel_list: true,
	};
	apply_strategic_patch_with_options(base, patch, schema, path, &options)
}

/// Apply a strategic merge patch to a base value with custom options.
///
/// # Arguments
/// * `base` - The current value
/// * `patch` - The patch to apply
/// * `schema` - Schema lookup for merge key information
/// * `path` - Current JSON path
/// * `options` - Options controlling merge behavior
pub fn apply_strategic_patch_with_options(
	base: &Value,
	patch: &Value,
	schema: &dyn SchemaLookup,
	path: &str,
	options: &MergeOptions,
) -> Result<Value, DiffError> {
	match (base, patch) {
		// Both objects - merge fields
		(Value::Object(base_map), Value::Object(patch_map)) => {
			apply_patch_to_map(base_map, patch_map, schema, path, options)
		}

		// Both arrays with merge keys - strategic merge
		(Value::Array(base_arr), Value::Array(patch_arr)) => {
			let Some(ref merge_keys) = schema.get_merge_keys(path) else {
				// No merge key - replace entire array
				return Ok(discard_nulls_if_needed(patch.clone(), options));
			};
			apply_patch_to_array_strategic(base_arr, patch_arr, schema, path, merge_keys, options)
		}

		// Null in patch means delete
		(_, Value::Null) => Ok(Value::Null),

		// Otherwise, patch replaces base
		(_, _) => Ok(discard_nulls_if_needed(patch.clone(), options)),
	}
}

/// Recursively remove null values from a value if options require it.
fn discard_nulls_if_needed(value: Value, options: &MergeOptions) -> Value {
	if !options.ignore_unmatched_nulls {
		return value;
	}
	discard_null_values(value)
}

/// Recursively remove null values from a value.
fn discard_null_values(value: Value) -> Value {
	match value {
		Value::Object(map) => {
			let filtered: Map<String, Value> = map
				.into_iter()
				.filter(|(_, v)| !v.is_null())
				.map(|(k, v)| (k, discard_null_values(v)))
				.collect();
			Value::Object(filtered)
		}
		Value::Array(arr) => {
			let filtered: Vec<Value> = arr
				.into_iter()
				.filter(|v| !v.is_null())
				.map(discard_null_values)
				.collect();
			Value::Array(filtered)
		}
		_ => value,
	}
}

/// Apply a patch to a map/object.
fn apply_patch_to_map(
	base: &Map<String, Value>,
	patch: &Map<String, Value>,
	schema: &dyn SchemaLookup,
	path: &str,
	options: &MergeOptions,
) -> Result<Value, DiffError> {
	let mut result = base.clone();

	// Collect directives to apply after field merging
	let mut primitive_deletions: Vec<(&str, std::collections::HashSet<&Value>)> = Vec::new();
	let mut retain_keys: Option<std::collections::HashSet<&str>> = None;
	let mut set_element_orders: Vec<(&str, &Vec<Value>)> = Vec::new();

	for (key, patch_value) in patch {
		// Collect $deleteFromPrimitiveList directives for later
		if let Some(field_name) = extract_delete_from_primitive_list_field(key) {
			let items_to_delete = patch_value.as_array().map(|arr| arr.iter().collect());
			if let Some(items_set) = items_to_delete {
				primitive_deletions.push((field_name, items_set));
			}
			continue;
		}

		// Collect $setElementOrder directives for later (only when merge_parallel_list is true)
		if options.merge_parallel_list {
			if let Some(field_name) = extract_set_element_order_field(key) {
				if let Some(order_arr) = patch_value.as_array() {
					set_element_orders.push((field_name, order_arr));
				}
				continue;
			}
		}

		// Collect $retainKeys directive for later
		if key == DIRECTIVE_RETAIN_KEYS {
			if let Some(arr) = patch_value.as_array() {
				retain_keys = Some(arr.iter().filter_map(|v| v.as_str()).collect());
			}
			continue;
		}

		if is_directive(key) {
			continue;
		}

		// Handle null values
		if patch_value.is_null() {
			result.remove(key);
			// If not ignoring unmatched nulls and key wasn't in base, propagate null
			if !options.ignore_unmatched_nulls && !base.contains_key(key) {
				result.insert(key.clone(), Value::Null);
			}
			continue;
		}

		let Some(base_value) = base.get(key) else {
			// Key not in base - take patch value, potentially discarding nested nulls
			let value = discard_nulls_if_needed(patch_value.clone(), options);
			result.insert(key.clone(), value);
			continue;
		};

		let field_path = if path.is_empty() {
			key.clone()
		} else {
			format!("{}.{}", path, key)
		};

		let merged = apply_strategic_patch_with_options(
			base_value,
			patch_value,
			schema,
			&field_path,
			options,
		)?;
		if merged.is_null() {
			result.remove(key);
		} else {
			result.insert(key.clone(), merged);
		}
	}

	// Apply $retainKeys directive - remove keys not in the retain list
	if let Some(keys_to_keep) = retain_keys {
		result.retain(|k, _| keys_to_keep.contains(k.as_str()));
	}

	// Apply $deleteFromPrimitiveList directives after field merging
	for (field_name, items_set) in primitive_deletions {
		if let Some(Value::Array(arr)) = result.get(field_name) {
			let filtered: Vec<Value> = arr
				.iter()
				.filter(|v| !items_set.contains(v))
				.cloned()
				.collect();
			result.insert(field_name.to_string(), Value::Array(filtered));
		}
	}

	// Apply $setElementOrder directives to reorder arrays
	for (field_name, order_arr) in set_element_orders {
		if let Some(Value::Array(arr)) = result.get(field_name) {
			let field_path = if path.is_empty() {
				field_name.to_string()
			} else {
				format!("{}.{}", path, field_name)
			};
			if let Some(merge_keys) = schema.get_merge_keys(&field_path) {
				let reordered = reorder_array_by_element_order(arr, order_arr, &merge_keys);
				result.insert(field_name.to_string(), Value::Array(reordered));
			}
		}
	}

	Ok(Value::Object(result))
}

/// Reorder an array based on a $setElementOrder directive.
///
/// Elements are reordered to match the order specified in `order`. Elements
/// not in `order` are appended at the end in their original order.
fn reorder_array_by_element_order(
	arr: &[Value],
	order: &[Value],
	merge_keys: &MergeKeys<'_>,
) -> Vec<Value> {
	use std::collections::HashMap;

	// Build a map from merge key to element
	let mut elements_by_key: HashMap<String, Value> = arr
		.iter()
		.filter_map(|item| get_composite_key_value(item, merge_keys).map(|key| (key, item.clone())))
		.collect();

	// Track which keys we've seen in order
	let mut ordered_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

	// Build result in order
	let mut result: Vec<Value> = Vec::new();

	for order_item in order {
		let Some(key) = get_composite_key_value(order_item, merge_keys) else {
			continue;
		};
		let Some(element) = elements_by_key.remove(&key) else {
			continue;
		};
		result.push(element);
		ordered_keys.insert(key);
	}

	// Append remaining elements that weren't in the order directive
	// Preserve their original relative order
	for item in arr {
		let Some(key) = get_composite_key_value(item, merge_keys) else {
			continue;
		};
		if ordered_keys.contains(&key) {
			continue;
		}
		let Some(element) = elements_by_key.remove(&key) else {
			continue;
		};
		result.push(element);
	}

	result
}

/// Apply a patch to an array using strategic merge.
fn apply_patch_to_array_strategic(
	base: &[Value],
	patch: &[Value],
	schema: &dyn SchemaLookup,
	path: &str,
	merge_keys: &MergeKeys<'_>,
	options: &MergeOptions,
) -> Result<Value, DiffError> {
	let mut result: Vec<Value> = base.to_vec();

	// Build index of base items by composite merge key
	let base_indices: std::collections::HashMap<String, usize> = base
		.iter()
		.enumerate()
		.filter_map(|(i, item)| get_composite_key_value(item, merge_keys).map(|key| (key, i)))
		.collect();

	for patch_item in patch {
		// Check for delete directive
		if is_delete_directive(patch_item) {
			let Some(key_value) = get_composite_key_value(patch_item, merge_keys) else {
				continue;
			};
			result.retain(|item| {
				get_composite_key_value(item, merge_keys)
					.map(|k| k != key_value)
					.unwrap_or(true)
			});
			continue;
		}

		let Some(key_value) = get_composite_key_value(patch_item, merge_keys) else {
			// No merge key - add as new item, potentially discarding nested nulls
			let item = discard_nulls_if_needed(patch_item.clone(), options);
			result.push(item);
			continue;
		};

		let Some(&base_idx) = base_indices.get(&key_value) else {
			// New item not in base - add it, potentially discarding nested nulls
			let item = discard_nulls_if_needed(patch_item.clone(), options);
			result.push(item);
			continue;
		};

		let item_path = format!("{}.{}", path, key_value);
		let merged = apply_strategic_patch_with_options(
			&result[base_idx],
			patch_item,
			schema,
			&item_path,
			options,
		)?;
		result[base_idx] = merged;
	}

	Ok(Value::Array(result))
}

/// Build a composite key string from multiple merge key fields.
fn get_composite_key_value(item: &Value, merge_keys: &MergeKeys<'_>) -> Option<String> {
	let obj = item.as_object()?;

	let parts: Vec<String> = merge_keys
		.into_iter()
		.map(|key| extract_key_part(obj.get(key)))
		.collect();

	if parts.iter().all(String::is_empty) {
		return None;
	}

	Some(parts.join("::"))
}

fn extract_key_part(value: Option<&Value>) -> String {
	let Some(v) = value else {
		return String::new();
	};

	match v {
		Value::String(s) => s.clone(),
		Value::Number(n) => n.to_string(),
		Value::Bool(b) => b.to_string(),
		_ => String::new(),
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;
	use serde_json::json;

	use super::*;
	use crate::strategicpatch::schema::BuiltinSchemaLookup;

	fn podspec_schema() -> BuiltinSchemaLookup {
		BuiltinSchemaLookup::for_type("io.k8s.api.core.v1.PodSpec")
	}

	fn service_spec_schema() -> BuiltinSchemaLookup {
		// Use SSA merge type for composite key tests
		BuiltinSchemaLookup::for_type_with_merge_type(
			"io.k8s.api.core.v1.ServiceSpec",
			crate::strategicpatch::MergeType::ServerSideApply,
		)
	}

	#[test]
	fn test_merge_patches_simple() {
		let deletions = json!({"removed": null});
		let delta = json!({"added": "value"});
		let schema = podspec_schema();

		let result = merge_patches(&deletions, &delta, &schema, "").unwrap();
		assert_eq!(result, json!({"removed": null, "added": "value"}));
	}

	#[test]
	fn test_merge_patches_overlapping() {
		let deletions = json!({"field": null});
		let delta = json!({"field": "new_value"});
		let schema = podspec_schema();

		let result = merge_patches(&deletions, &delta, &schema, "").unwrap();
		// Delta wins over deletion
		assert_eq!(result, json!({"field": "new_value"}));
	}

	#[test]
	fn test_merge_patches_nested_nulls() {
		// Verify nested null handling matches kubectl's IgnoreUnmatchedNulls=false behavior
		// Nulls in deletions should be preserved in the merged result
		let deletions = json!({
			"a": {"nested_delete": null},
			"top_level_delete": null
		});
		let delta = json!({
			"a": {"nested_add": "value"},
			"top_level_add": "new"
		});
		let schema = podspec_schema();

		let result = merge_patches(&deletions, &delta, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"a": {"nested_delete": null, "nested_add": "value"},
				"top_level_delete": null,
				"top_level_add": "new"
			})
		);
	}

	#[test]
	fn test_apply_patch_add_field() {
		let base = json!({"existing": "value"});
		let patch = json!({"new": "field"});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(result, json!({"existing": "value", "new": "field"}));
	}

	#[test]
	fn test_apply_patch_remove_field() {
		let base = json!({"keep": "this", "remove": "that"});
		let patch = json!({"remove": null});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(result, json!({"keep": "this"}));
	}

	#[test]
	fn test_apply_patch_update_container() {
		let base = json!({
			"containers": [
				{"name": "app", "image": "nginx:1.0"}
			]
		});
		let patch = json!({
			"containers": [
				{"name": "app", "image": "nginx:2.0"}
			]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"containers": [
					{"name": "app", "image": "nginx:2.0"}
				]
			})
		);
	}

	#[test]
	fn test_apply_patch_add_container() {
		let base = json!({
			"containers": [
				{"name": "app", "image": "nginx"}
			]
		});
		let patch = json!({
			"containers": [
				{"name": "sidecar", "image": "proxy"}
			]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"containers": [
					{"name": "app", "image": "nginx"},
					{"name": "sidecar", "image": "proxy"}
				]
			})
		);
	}

	#[test]
	fn test_apply_patch_delete_container() {
		let base = json!({
			"containers": [
				{"name": "app", "image": "nginx"},
				{"name": "sidecar", "image": "proxy"}
			]
		});
		let patch = json!({
			"containers": [
				{"name": "sidecar", "$patch": "delete"}
			]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"containers": [
					{"name": "app", "image": "nginx"}
				]
			})
		);
	}

	#[test]
	fn test_apply_delete_from_primitive_list() {
		// Apply $deleteFromPrimitiveList directive to remove items
		let base = json!({
			"finalizers": ["kubernetes.io/pv-protection", "custom-finalizer", "another-finalizer"]
		});
		let patch = json!({
			"$deleteFromPrimitiveList/finalizers": ["custom-finalizer"]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"finalizers": ["kubernetes.io/pv-protection", "another-finalizer"]
			})
		);
	}

	#[test]
	fn test_apply_delete_from_primitive_list_multiple() {
		// Delete multiple items at once
		let base = json!({
			"args": ["--verbose", "--debug", "--old-flag", "--another-old"]
		});
		let patch = json!({
			"$deleteFromPrimitiveList/args": ["--old-flag", "--another-old"]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"args": ["--verbose", "--debug"]
			})
		);
	}

	#[test]
	fn test_apply_delete_from_primitive_list_nonexistent() {
		// Deleting items that don't exist should be a no-op
		let base = json!({
			"finalizers": ["kubernetes.io/pv-protection"]
		});
		let patch = json!({
			"$deleteFromPrimitiveList/finalizers": ["nonexistent-finalizer"]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"finalizers": ["kubernetes.io/pv-protection"]
			})
		);
	}

	#[test]
	fn test_apply_retain_keys() {
		// $retainKeys should remove keys not in the list
		let base = json!({
			"name": "my-vol",
			"configMap": {"name": "my-config"},
			"extraField": "should-be-removed"
		});
		let patch = json!({
			"secret": {"secretName": "my-secret"},
			"$retainKeys": ["name", "secret"]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"name": "my-vol",
				"secret": {"secretName": "my-secret"}
			})
		);
	}

	#[test]
	fn test_apply_retain_keys_preserves_listed_keys() {
		// Keys in $retainKeys that exist in base should be preserved
		let base = json!({
			"name": "my-vol",
			"configMap": {"name": "my-config"}
		});
		let patch = json!({
			"$retainKeys": ["name", "configMap"]
		});
		let schema = podspec_schema();

		let result = apply_strategic_patch(&base, &patch, &schema, "").unwrap();
		assert_eq!(
			result,
			json!({
				"name": "my-vol",
				"configMap": {"name": "my-config"}
			})
		);
	}

	#[rstest]
	#[case::update_one_protocol(
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]}),
		json!({"ports": [{"port": 80, "protocol": "TCP", "targetPort": 9090}]}),
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 9090},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]})
	)]
	#[case::delete_one_protocol(
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]}),
		json!({"ports": [{"port": 80, "protocol": "UDP", "$patch": "delete"}]}),
		json!({"ports": [{"port": 80, "protocol": "TCP", "targetPort": 8080}]})
	)]
	#[case::add_new_protocol(
		json!({"ports": [{"port": 80, "protocol": "TCP", "targetPort": 8080}]}),
		json!({"ports": [{"port": 80, "protocol": "UDP", "targetPort": 8080}]}),
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]})
	)]
	fn test_apply_composite_key_service_ports(
		#[case] base: serde_json::Value,
		#[case] patch: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		let result = apply_strategic_patch(&base, &patch, &service_spec_schema(), "").unwrap();
		assert_eq!(result, expected);
	}

	#[test]
	fn test_ignore_unmatched_nulls_true() {
		// With ignore_unmatched_nulls=true (default), null values in patch
		// that don't exist in base are discarded
		use super::apply_strategic_patch_with_options;

		let base = json!({"existing": "value"});
		let patch = json!({"existing": "updated", "nonexistent": null});
		let schema = podspec_schema();
		let options = MergeOptions {
			ignore_unmatched_nulls: true,
			merge_parallel_list: true,
		};

		let result =
			apply_strategic_patch_with_options(&base, &patch, &schema, "", &options).unwrap();
		// "nonexistent" should NOT be in the result
		assert_eq!(result, json!({"existing": "updated"}));
	}

	#[test]
	fn test_ignore_unmatched_nulls_false() {
		// With ignore_unmatched_nulls=false, null values in patch
		// that don't exist in base are propagated to the result
		use super::apply_strategic_patch_with_options;

		let base = json!({"existing": "value"});
		let patch = json!({"existing": "updated", "nonexistent": null});
		let schema = podspec_schema();
		let options = MergeOptions {
			ignore_unmatched_nulls: false,
			merge_parallel_list: true,
		};

		let result =
			apply_strategic_patch_with_options(&base, &patch, &schema, "", &options).unwrap();
		// "nonexistent" SHOULD be in the result as null
		assert_eq!(result, json!({"existing": "updated", "nonexistent": null}));
	}

	#[test]
	fn test_ignore_unmatched_nulls_nested() {
		// Nested nulls in new fields should be discarded when ignore_unmatched_nulls=true
		use super::apply_strategic_patch_with_options;

		let base = json!({});
		let patch = json!({"new_field": {"keep": "this", "discard": null}});
		let schema = podspec_schema();
		let options = MergeOptions {
			ignore_unmatched_nulls: true,
			merge_parallel_list: true,
		};

		let result =
			apply_strategic_patch_with_options(&base, &patch, &schema, "", &options).unwrap();
		// Nested null should be discarded
		assert_eq!(result, json!({"new_field": {"keep": "this"}}));
	}

	#[test]
	fn test_set_element_order_reorders_array() {
		// $setElementOrder directive should reorder array elements
		use super::apply_strategic_patch_with_options;
		use crate::strategicpatch::schema::BuiltinSchemaLookup;

		let base = json!({
			"containers": [
				{"name": "app", "image": "nginx"},
				{"name": "sidecar", "image": "proxy"},
				{"name": "logger", "image": "fluentd"}
			]
		});
		let patch = json!({
			"$setElementOrder/containers": [
				{"name": "logger"},
				{"name": "app"},
				{"name": "sidecar"}
			]
		});
		let schema = BuiltinSchemaLookup::for_type("io.k8s.api.core.v1.PodSpec");
		let options = MergeOptions {
			ignore_unmatched_nulls: true,
			merge_parallel_list: true,
		};

		let result =
			apply_strategic_patch_with_options(&base, &patch, &schema, "", &options).unwrap();

		// Array should be reordered according to $setElementOrder
		assert_eq!(
			result,
			json!({
				"containers": [
					{"name": "logger", "image": "fluentd"},
					{"name": "app", "image": "nginx"},
					{"name": "sidecar", "image": "proxy"}
				]
			})
		);
	}

	#[test]
	fn test_set_element_order_not_processed_when_disabled() {
		// When merge_parallel_list=false, $setElementOrder should not be processed
		use super::apply_strategic_patch_with_options;
		use crate::strategicpatch::schema::BuiltinSchemaLookup;

		let base = json!({
			"containers": [
				{"name": "app", "image": "nginx"},
				{"name": "sidecar", "image": "proxy"}
			]
		});
		let patch = json!({
			"$setElementOrder/containers": [
				{"name": "sidecar"},
				{"name": "app"}
			]
		});
		let schema = BuiltinSchemaLookup::for_type("io.k8s.api.core.v1.PodSpec");
		let options = MergeOptions {
			ignore_unmatched_nulls: true,
			merge_parallel_list: false, // Disabled
		};

		let result =
			apply_strategic_patch_with_options(&base, &patch, &schema, "", &options).unwrap();

		// Order should remain unchanged (directive not processed)
		assert_eq!(
			result,
			json!({
				"containers": [
					{"name": "app", "image": "nginx"},
					{"name": "sidecar", "image": "proxy"}
				]
			})
		);
	}
}
