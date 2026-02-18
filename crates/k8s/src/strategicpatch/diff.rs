//! Strategic merge patch diff algorithm.
//!
//! This module implements the diff algorithm that computes strategic merge patches
//! between Kubernetes resources.

use serde_json::{Map, Value};

use super::{
	directives::{
		delete_from_primitive_list_key, is_directive, set_element_order_key, DIRECTIVE_RETAIN_KEYS,
	},
	merge::merge_patches,
	schema::{MergeKeys, SchemaLookup},
};

/// A precondition function that validates a patch before it's returned.
///
/// Returns `None` if the precondition passes, or `Some(key)` with the key that
/// caused the failure if it fails.
pub type PreconditionFunc = Box<dyn Fn(&Value) -> Option<String>>;

/// Create a precondition that fails if a key is present in the patch.
///
/// If the key is present in the patch, it means its value has changed,
/// so the precondition fails.
pub fn require_key_unchanged(key: &'static str) -> PreconditionFunc {
	Box::new(move |patch: &Value| patch.as_object()?.get(key).map(|_| key.to_string()))
}

/// Create a precondition that fails if a metadata key is present in the patch.
///
/// Checks for changes to `metadata.<key>`.
pub fn require_metadata_key_unchanged(key: &'static str) -> PreconditionFunc {
	Box::new(move |patch: &Value| {
		patch
			.as_object()?
			.get("metadata")?
			.as_object()?
			.get(key)
			.map(|_| format!("metadata.{}", key))
	})
}

/// Options controlling the diff algorithm behavior.
#[derive(Debug, Clone, Default)]
pub struct DiffOptions<'a> {
	/// If true, ignore deletions (fields in `from` but not in `to`).
	pub ignore_deletions: bool,
	/// If true, ignore changes and additions (only compute deletions).
	pub ignore_changes_and_additions: bool,
	/// If true, generate `$setElementOrder` directives for arrays.
	pub build_element_order: bool,
	/// Original state for three-way merge. Used to identify server-injected elements
	/// when building `$setElementOrder`. Server-injected elements (in `from` but not
	/// in `original`) are included in the element order at their relative positions.
	pub original: Option<&'a Value>,
}

/// Create a three-way strategic merge patch.
///
/// This implements kubectl's three-way merge algorithm:
/// 1. Compute what changed between current and modified (delta)
/// 2. Compute what was deleted between original and modified (deletions)
/// 3. Merge deletions into delta to get the final patch
///
/// # Arguments
/// * `original` - The last-applied configuration (from annotation), or None
/// * `modified` - The desired state (from manifest)
/// * `current` - The current state (from cluster)
/// * `schema` - Schema lookup for merge key information
///
/// # Returns
/// A strategic merge patch that transforms `current` into `modified`.
pub fn create_three_way_merge_patch(
	original: Option<&Value>,
	modified: &Value,
	current: &Value,
	schema: &dyn SchemaLookup,
) -> Result<Value, DiffError> {
	create_three_way_merge_patch_with_options(original, modified, current, schema, true, &[])
}

/// Create a three-way strategic merge patch with conflict detection control.
///
/// # Arguments
/// * `original` - The last-applied configuration (from annotation), or None
/// * `modified` - The desired state (from manifest)
/// * `current` - The current state (from cluster)
/// * `schema` - Schema lookup for merge key information
/// * `overwrite` - If false, detect conflicts between user and server changes
/// * `preconditions` - Functions to validate the patch before returning
///
/// # Returns
/// A strategic merge patch, or an error if conflicts are detected, `overwrite` is false,
/// or any precondition fails.
pub fn create_three_way_merge_patch_with_options(
	original: Option<&Value>,
	modified: &Value,
	current: &Value,
	schema: &dyn SchemaLookup,
	overwrite: bool,
	preconditions: &[PreconditionFunc],
) -> Result<Value, DiffError> {
	// Step 1: Compute delta (current → modified), ignoring deletions
	// This tells us what we need to add/change to get from current to modified
	// Pass original so we can identify server-injected elements for $setElementOrder
	let delta_options = DiffOptions {
		ignore_deletions: true,
		ignore_changes_and_additions: false,
		build_element_order: true,
		original,
	};
	let delta = diff_values(current, modified, schema, "", &delta_options)?;

	// Step 2: If we have an original, compute deletions (original → modified)
	// This tells us what the user intentionally deleted
	let patch = if let Some(original) = original {
		let deletion_options = DiffOptions {
			ignore_deletions: false,
			ignore_changes_and_additions: true,
			build_element_order: false,
			original: None,
		};
		let deletions = diff_values(original, modified, schema, "", &deletion_options)?;

		// Step 3: Merge deletions into delta
		let patch = merge_patches(&deletions, &delta, schema, "")?;

		// Step 4: Apply preconditions
		for precondition in preconditions {
			if let Some(key) = precondition(&patch) {
				return Err(DiffError::PreconditionFailed { key });
			}
		}

		// Step 5: If not overwriting, check for conflicts
		// Compare what USER changed (original → modified) vs what SERVER changed (original → current)
		// This is the correct comparison per kubectl - conflicts occur when both parties
		// modified the same field, not when our patch would revert server changes.
		if !overwrite {
			// Compute what the user changed (original → modified)
			let user_changes_options = DiffOptions {
				ignore_deletions: true,
				ignore_changes_and_additions: false,
				build_element_order: false,
				original: None,
			};
			let user_changes = diff_values(original, modified, schema, "", &user_changes_options)?;

			// Compute what the server changed (original → current)
			let server_changes = diff_values(original, current, schema, "", &user_changes_options)?;

			// Check if user and server modified the same fields to different values
			if let Some(conflict) = find_conflicts(&user_changes, &server_changes, schema, "") {
				return Err(DiffError::Conflict {
					path: conflict.path,
					modified: conflict.user_value,
					current: conflict.server_value,
				});
			}
		}

		patch
	} else {
		// No original (no last-applied-configuration annotation)
		// Without knowing what the user previously applied, we can't determine
		// what was intentionally deleted vs server-injected. Just use the delta.
		// No conflict detection possible without original.

		// Apply preconditions
		for precondition in preconditions {
			if let Some(key) = precondition(&delta) {
				return Err(DiffError::PreconditionFailed { key });
			}
		}

		delta
	};

	Ok(patch)
}

/// Information about a detected conflict.
struct ConflictInfo {
	path: String,
	user_value: String,
	server_value: String,
}

/// Find conflicts between two patches.
///
/// Returns the first conflict found, or None if no conflicts.
/// A conflict occurs when both patches modify the same field to different values.
fn find_conflicts(
	user_patch: &Value,
	server_changes: &Value,
	schema: &dyn SchemaLookup,
	path: &str,
) -> Option<ConflictInfo> {
	match (user_patch, server_changes) {
		(Value::Object(user_map), Value::Object(server_map)) => {
			for (key, user_val) in user_map {
				// Skip directives
				if is_directive(key) {
					continue;
				}

				let child_path = if path.is_empty() {
					key.clone()
				} else {
					format!("{}.{}", path, key)
				};

				if let Some(server_val) = server_map.get(key) {
					// Both modified the same key
					if let Some(conflict) =
						find_conflicts(user_val, server_val, schema, &child_path)
					{
						return Some(conflict);
					}
				}
			}
			None
		}
		(Value::Array(user_arr), Value::Array(server_arr)) => {
			find_array_conflicts(user_arr, server_arr, schema, path)
		}
		(user_val, server_val) => {
			// Scalar values - conflict if both changed to different values
			if user_val != server_val
				&& !matches!(user_val, Value::Object(_))
				&& !matches!(server_val, Value::Object(_))
			{
				Some(ConflictInfo {
					path: path.to_string(),
					user_value: format_value_for_error(user_val),
					server_value: format_value_for_error(server_val),
				})
			} else {
				None
			}
		}
	}
}

/// Format a JSON value for error messages.
fn format_value_for_error(value: &Value) -> String {
	match value {
		Value::String(s) => format!("{:?}", s),
		Value::Number(n) => n.to_string(),
		Value::Bool(b) => b.to_string(),
		Value::Null => "null".to_string(),
		Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
	}
}

/// Find conflicts in arrays.
///
/// For arrays with merge keys, checks if items with the same key have conflicting values.
/// For primitive arrays or arrays without merge keys, no conflict is possible since
/// strategic merge replaces the entire array.
fn find_array_conflicts(
	user_arr: &[Value],
	server_arr: &[Value],
	schema: &dyn SchemaLookup,
	path: &str,
) -> Option<ConflictInfo> {
	let Some(merge_keys) = schema.get_merge_keys(path) else {
		// No merge keys - arrays are replaced atomically, no conflict possible
		return None;
	};

	// Check if elements are objects (merging lists of scalars have no conflicts)
	let user_is_objects = user_arr.first().map(|v| v.is_object()).unwrap_or(false);
	let server_is_objects = server_arr.first().map(|v| v.is_object()).unwrap_or(false);

	if !user_is_objects || !server_is_objects {
		// Scalar arrays - no conflicts by definition for merge strategy
		return None;
	}

	// Build index of server items by merge key
	let server_by_key: std::collections::HashMap<String, &Value> = server_arr
		.iter()
		.filter_map(|item| get_composite_key_value(item, &merge_keys).map(|key| (key, item)))
		.collect();

	// Check each user item against server items with matching keys
	for user_item in user_arr {
		let Some(key_value) = get_composite_key_value(user_item, &merge_keys) else {
			continue;
		};

		let Some(server_item) = server_by_key.get(&key_value) else {
			continue;
		};

		// Both modified the same array element - check for nested conflicts
		let item_path = format!("{}[{}]", path, key_value);
		if let Some(conflict) = find_conflicts(user_item, server_item, schema, &item_path) {
			return Some(conflict);
		}
	}

	None
}

/// Diff two JSON values using strategic merge semantics.
///
/// # Arguments
/// * `from` - The source value
/// * `to` - The target value
/// * `schema` - Schema lookup for merge key information
/// * `path` - Current JSON path (for schema lookups)
/// * `options` - Options controlling diff behavior
pub fn diff_values(
	from: &Value,
	to: &Value,
	schema: &dyn SchemaLookup,
	path: &str,
	options: &DiffOptions<'_>,
) -> Result<Value, DiffError> {
	match (from, to) {
		// Both are objects - recursive merge
		(Value::Object(from_map), Value::Object(to_map)) => {
			diff_maps(from_map, to_map, schema, path, options)
		}

		// Both are arrays - may need strategic merge
		(Value::Array(from_arr), Value::Array(to_arr)) => {
			diff_arrays(from_arr, to_arr, schema, path, options)
		}

		// Different types or scalar values
		_ => {
			if from == to {
				// No change
				Ok(Value::Object(Map::new()))
			} else if options.ignore_changes_and_additions {
				// Only tracking deletions, and this is a change
				Ok(Value::Object(Map::new()))
			} else {
				// Value changed - return the new value
				Ok(to.clone())
			}
		}
	}
}

/// Insert a diff result into the result map, extracting array wrapper directives if present.
///
/// Array diffs may return a wrapper object containing directives like `$setElementOrder/field`
/// or `$deleteFromPrimitiveList/field`. These directives need to be placed at the parent level,
/// not nested under the field key.
fn insert_diff_result(result: &mut Map<String, Value>, key: &str, diff: Value) {
	if let Value::Object(diff_map) = &diff {
		let order_key = format!("$setElementOrder/{}", key);
		let delete_key = format!("$deleteFromPrimitiveList/{}", key);
		let is_array_wrapper = diff_map.len() <= 2
			&& (diff_map.contains_key(&order_key) || diff_map.contains_key(&delete_key))
			&& diff_map
				.keys()
				.all(|k| k == key || k == &order_key || k == &delete_key);

		if is_array_wrapper {
			if let Some(order) = diff_map.get(&order_key) {
				result.insert(order_key, order.clone());
			}
			if let Some(delete) = diff_map.get(&delete_key) {
				result.insert(delete_key, delete.clone());
			}
			if let Some(changes) = diff_map.get(key) {
				if !is_empty_patch(changes) {
					result.insert(key.to_string(), changes.clone());
				}
			}
			return;
		}
	}

	result.insert(key.to_string(), diff);
}

/// Diff two JSON objects.
fn diff_maps(
	from: &Map<String, Value>,
	to: &Map<String, Value>,
	schema: &dyn SchemaLookup,
	path: &str,
	options: &DiffOptions<'_>,
) -> Result<Value, DiffError> {
	let mut result = Map::new();

	// Check if any ancestor array field has retainKeys strategy.
	// Path format is like "spec.template.spec.volumes.my-volume-name.nested.deep"
	// We need to check all parent paths, not just the immediate parent, because
	// deletions within nested objects of a retainKeys field should still use $retainKeys.
	let any_ancestor_has_retain_keys = has_retain_keys_ancestor(path, schema);

	// Track if we need $retainKeys directive
	let mut needs_retain_keys = false;

	// Process keys in `to` (additions and changes)
	if !options.ignore_changes_and_additions {
		for (key, to_value) in to {
			// Skip directive keys
			if is_directive(key) {
				continue;
			}

			let field_path = if path.is_empty() {
				key.clone()
			} else {
				format!("{}.{}", path, key)
			};

			let Some(from_value) = from.get(key) else {
				// Key only in `to` - addition
				result.insert(key.clone(), to_value.clone());
				continue;
			};

			// Key exists in both - recurse
			let diff = diff_values(from_value, to_value, schema, &field_path, options)?;
			if !is_empty_patch(&diff) {
				insert_diff_result(&mut result, key, diff);
			}
		}
	}

	// Process keys in `from` but not in `to` (deletions)
	// Also recurse into nested objects when looking for deletions only
	if !options.ignore_deletions {
		for (key, from_value) in from {
			// Skip directive keys
			if is_directive(key) {
				continue;
			}

			let Some(to_value) = to.get(key) else {
				// Key deleted
				if any_ancestor_has_retain_keys {
					// For retainKeys fields, we'll use $retainKeys instead of null
					needs_retain_keys = true;
				} else {
					// Regular deletion - mark with null
					result.insert(key.clone(), Value::Null);
				}
				continue;
			};

			// Key exists in both - recurse to find nested deletions
			// (only when ignoring changes/additions, otherwise this was handled above)
			if options.ignore_changes_and_additions {
				let field_path = if path.is_empty() {
					key.clone()
				} else {
					format!("{}.{}", path, key)
				};
				let diff = diff_values(from_value, to_value, schema, &field_path, options)?;
				if !is_empty_patch(&diff) {
					insert_diff_result(&mut result, key, diff);
				}
			}
		}
	}

	// Generate $retainKeys directive if needed
	// This happens during the deletions pass (ignore_changes_and_additions: true)
	// We need to include $retainKeys to tell the server which keys to keep
	if needs_retain_keys {
		let keys_to_retain: Vec<Value> = to
			.keys()
			.filter(|k| !is_directive(k))
			.map(|k| Value::String(k.clone()))
			.collect();
		result.insert(
			DIRECTIVE_RETAIN_KEYS.to_string(),
			Value::Array(keys_to_retain),
		);
	}

	Ok(Value::Object(result))
}

/// Diff two JSON arrays using strategic merge semantics.
fn diff_arrays(
	from: &[Value],
	to: &[Value],
	schema: &dyn SchemaLookup,
	path: &str,
	options: &DiffOptions<'_>,
) -> Result<Value, DiffError> {
	// Check if this array has merge keys
	if let Some(merge_keys) = schema.get_merge_keys(path) {
		diff_arrays_strategic(from, to, schema, path, &merge_keys, options)
	} else if is_primitive_array(from) && is_primitive_array(to) {
		// Primitive array - use $deleteFromPrimitiveList for deletions
		diff_arrays_primitive(from, to, path, options)
	} else {
		// No merge key and not primitive - treat as atomic (replace entire array)
		if options.ignore_changes_and_additions || from == to {
			Ok(Value::Object(Map::new()))
		} else {
			Ok(Value::Array(to.to_vec()))
		}
	}
}

/// Check if an array contains only primitive values (strings, numbers, booleans, null).
fn is_primitive_array(arr: &[Value]) -> bool {
	arr.iter().all(|v| {
		matches!(
			v,
			Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
		)
	})
}

/// Diff two primitive arrays, generating $deleteFromPrimitiveList for deletions.
fn diff_arrays_primitive(
	from: &[Value],
	to: &[Value],
	path: &str,
	options: &DiffOptions,
) -> Result<Value, DiffError> {
	// Early return if arrays are identical
	if from == to {
		return Ok(Value::Object(Map::new()));
	}

	let field_name = path.rsplit('.').next().unwrap_or(path);

	// Build sets once for efficient lookups
	let from_set: std::collections::HashSet<_> = from.iter().collect();
	let to_set: std::collections::HashSet<_> = to.iter().collect();

	// Compute additions and deletions
	let has_additions =
		!options.ignore_changes_and_additions && to.iter().any(|v| !from_set.contains(v));
	let deletions: Vec<Value> = if options.ignore_deletions {
		vec![]
	} else {
		from.iter()
			.filter(|v| !to_set.contains(v))
			.cloned()
			.collect()
	};

	// Return based on what changed:
	// - Additions only: return the array directly (parent will insert under key)
	// - Deletions only: return wrapper with directive
	// - Both: return wrapper with array and directive
	match (has_additions, !deletions.is_empty()) {
		(false, false) => Ok(Value::Object(Map::new())),
		(true, false) => Ok(Value::Array(to.to_vec())),
		(false, true) => {
			let mut result = Map::new();
			let delete_key = delete_from_primitive_list_key(field_name);
			result.insert(delete_key, Value::Array(deletions));
			Ok(Value::Object(result))
		}
		(true, true) => {
			let mut result = Map::new();
			result.insert(field_name.to_string(), Value::Array(to.to_vec()));
			let delete_key = delete_from_primitive_list_key(field_name);
			result.insert(delete_key, Value::Array(deletions));
			Ok(Value::Object(result))
		}
	}
}

/// Diff arrays using strategic merge by key.
fn diff_arrays_strategic(
	from: &[Value],
	to: &[Value],
	schema: &dyn SchemaLookup,
	path: &str,
	merge_keys: &MergeKeys<'_>,
	options: &DiffOptions<'_>,
) -> Result<Value, DiffError> {
	let mut result = Vec::new();
	let mut element_order = Vec::new();

	// Index `from` array by composite merge key
	let from_index: std::collections::HashMap<String, &Value> = from
		.iter()
		.filter_map(|item| get_composite_key_value(item, merge_keys).map(|key| (key, item)))
		.collect();

	// Build set of keys in `to` for quick lookup
	let to_keys: std::collections::HashSet<String> = to
		.iter()
		.filter_map(|item| get_composite_key_value(item, merge_keys))
		.collect();

	// Build set of keys in original (if provided) to identify server-injected elements.
	// Server-injected elements are those present in `from` (current) but not in original.
	// We can only identify these if original is provided.
	let original_keys: Option<std::collections::HashSet<String>> = options
		.original
		.and_then(|v| extract_array_at_path(v, path))
		.map(|arr| {
			arr.iter()
				.filter_map(|item| get_composite_key_value(item, merge_keys))
				.collect()
		});

	// Process items in `to`
	// When ignore_changes_and_additions is true, we still need to recurse into
	// matching items to find nested deletions (e.g., deletions in primitive arrays
	// within containers).
	for to_item in to {
		let Some(key_value) = get_composite_key_value(to_item, merge_keys) else {
			if !options.ignore_changes_and_additions {
				result.push(to_item.clone());
			}
			continue;
		};

		if options.build_element_order && !options.ignore_changes_and_additions {
			let order_entry = build_key_object(to_item, merge_keys);
			element_order.push(Value::Object(order_entry));

			// After adding this element, insert any server-injected elements that
			// follow it in `from` (current). Server-injected elements are those
			// present in `from` but not in `original`.
			append_server_injected_elements(
				&key_value,
				from,
				&from_index,
				&to_keys,
				&original_keys,
				merge_keys,
				&mut element_order,
			);
		}

		let Some(from_item) = from_index.get(&key_value) else {
			if !options.ignore_changes_and_additions {
				result.push(to_item.clone());
			}
			continue;
		};

		// Recurse to find nested changes/deletions
		let item_path = format!("{}.{}", path, key_value);
		let diff = diff_values(from_item, to_item, schema, &item_path, options)?;
		if is_empty_patch(&diff) {
			continue;
		}

		let Value::Object(mut diff_map) = diff else {
			continue;
		};

		// Add all merge keys to the diff for identification
		add_merge_keys_to_map(&mut diff_map, to_item, merge_keys);
		result.push(Value::Object(diff_map));
	}

	// Process deletions (items in `from` but not in `to`)
	if !options.ignore_deletions {
		let to_keys: std::collections::HashSet<String> = to
			.iter()
			.filter_map(|item| get_composite_key_value(item, merge_keys))
			.collect();

		// Count items to delete
		let items_to_delete: Vec<_> = from
			.iter()
			.filter(|from_item| {
				get_composite_key_value(from_item, merge_keys)
					.map(|key| !to_keys.contains(&key))
					.unwrap_or(false)
			})
			.collect();

		// If ALL items from `from` are being deleted (to is empty),
		// generate a single $patch: replace directive instead of individual delete directives.
		if !items_to_delete.is_empty() && items_to_delete.len() == from.len() && to.is_empty() {
			let mut replace_directive = Map::new();
			replace_directive.insert(
				super::directives::DIRECTIVE_PATCH.to_string(),
				Value::String(super::directives::patch_value::REPLACE.to_string()),
			);
			result.push(Value::Object(replace_directive));
		} else {
			for from_item in items_to_delete {
				let delete_directive = create_composite_delete_directive(from_item, merge_keys);
				result.push(delete_directive);
			}
		}
	}

	// If no changes, return empty array
	if result.is_empty() {
		return Ok(Value::Array(vec![]));
	}

	// Wrap result in an object with $setElementOrder if needed
	// Only include element order when there are actual changes
	if options.build_element_order && !element_order.is_empty() {
		// Extract the field name from the path
		let field_name = path.rsplit('.').next().unwrap_or(path);
		let order_key = set_element_order_key(field_name);

		// We need to return the array changes plus the order directive
		// This will be merged at the parent level
		let mut wrapper = Map::new();
		wrapper.insert(field_name.to_string(), Value::Array(result));
		wrapper.insert(order_key, Value::Array(element_order));
		return Ok(Value::Object(wrapper));
	}

	Ok(Value::Array(result))
}

/// Build a composite key string from multiple merge key fields.
/// For single key ["name"], returns "app"
/// For composite keys ["port", "protocol"], returns "80::TCP"
fn get_composite_key_value(item: &Value, merge_keys: &MergeKeys<'_>) -> Option<String> {
	let obj = item.as_object()?;
	let parts: Vec<String> = merge_keys
		.into_iter()
		.map(|key| {
			obj.get(key)
				.and_then(|v| match v {
					Value::String(s) => Some(s.clone()),
					Value::Number(n) => Some(n.to_string()),
					Value::Bool(b) => Some(b.to_string()),
					_ => None,
				})
				.unwrap_or_default()
		})
		.collect();

	if parts.iter().all(|p| p.is_empty()) {
		return None;
	}

	Some(parts.join("::"))
}

/// Build an object containing just the merge key fields from an item.
fn build_key_object(item: &Value, merge_keys: &MergeKeys<'_>) -> Map<String, Value> {
	let mut result = Map::new();
	if let Some(obj) = item.as_object() {
		for key in merge_keys {
			if let Some(value) = obj.get(key) {
				result.insert(key.to_string(), value.clone());
			}
		}
	}
	result
}

/// Append server-injected elements to the element order.
///
/// Server-injected elements are those present in `from` (current state) but not in `original`.
/// They are inserted after the anchor element to preserve their relative position.
fn append_server_injected_elements(
	anchor_key: &str,
	from: &[Value],
	from_index: &std::collections::HashMap<String, &Value>,
	to_keys: &std::collections::HashSet<String>,
	original_keys: &Option<std::collections::HashSet<String>>,
	merge_keys: &MergeKeys<'_>,
	element_order: &mut Vec<Value>,
) {
	// Only do this if we have original to identify server-injected elements
	let Some(orig_keys) = original_keys else {
		return;
	};
	let Some(from_item_val) = from_index.get(anchor_key) else {
		return;
	};
	let Some(pos) = from
		.iter()
		.position(|item| std::ptr::eq(item, *from_item_val))
	else {
		return;
	};

	// Look at subsequent elements in `from`
	for subsequent in from.iter().skip(pos + 1) {
		let Some(subsequent_key) = get_composite_key_value(subsequent, merge_keys) else {
			break;
		};
		// Stop when we hit an element that's in `to`
		if to_keys.contains(&subsequent_key) {
			break;
		}
		// If this element is server-injected (in from but not in original),
		// include it in the element order
		if !orig_keys.contains(&subsequent_key) {
			let order_entry = build_key_object(subsequent, merge_keys);
			element_order.push(Value::Object(order_entry));
		}
	}
}

/// Add all merge key fields to a diff map for element identification.
fn add_merge_keys_to_map(
	diff_map: &mut Map<String, Value>,
	item: &Value,
	merge_keys: &MergeKeys<'_>,
) {
	if let Some(obj) = item.as_object() {
		for key in merge_keys {
			if let Some(value) = obj.get(key) {
				diff_map.insert(key.to_string(), value.clone());
			}
		}
	}
}

/// Create a delete directive with all merge key fields.
fn create_composite_delete_directive(item: &Value, merge_keys: &MergeKeys<'_>) -> Value {
	let mut directive = build_key_object(item, merge_keys);
	directive.insert(
		super::directives::DIRECTIVE_PATCH.to_string(),
		Value::String(super::directives::patch_value::DELETE.to_string()),
	);
	Value::Object(directive)
}

/// Extract an array at a given path from a JSON value.
///
/// Path format: "spec.template.spec.containers" (dot-separated).
/// Returns None if the path doesn't exist or doesn't lead to an array.
fn extract_array_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a [Value]> {
	let mut current = value;
	for segment in path.split('.') {
		// Skip empty segments (e.g., from leading dots)
		if segment.is_empty() {
			continue;
		}
		current = current.as_object()?.get(segment)?;
	}
	current.as_array().map(|v| v.as_slice())
}

/// Check if any ancestor path has retainKeys strategy.
///
/// This walks up the path hierarchy checking each ancestor for retainKeys.
/// Used to determine if deletions should use $retainKeys instead of null.
fn has_retain_keys_ancestor(path: &str, schema: &dyn SchemaLookup) -> bool {
	let mut current_path = path;
	while let Some(dot_pos) = current_path.rfind('.') {
		current_path = &current_path[..dot_pos];
		if schema.has_retain_keys(current_path) {
			return true;
		}
	}
	false
}

/// Check if a patch is empty (no changes).
fn is_empty_patch(patch: &Value) -> bool {
	match patch {
		Value::Object(map) => map.is_empty(),
		Value::Array(arr) => arr.is_empty(),
		Value::Null => true,
		_ => false,
	}
}

/// Error type for diff operations.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
	#[error("Schema lookup failed: {0}")]
	SchemaLookup(String),

	#[error("Invalid patch structure: {0}")]
	InvalidPatch(String),

	#[error("Merge error: {0}")]
	MergeError(String),

	#[error("conflict at {path}: modified={modified}, current={current}")]
	Conflict {
		path: String,
		modified: String,
		current: String,
	},

	#[error("precondition failed: key {key:?} was modified")]
	PreconditionFailed { key: String },
}

#[cfg(test)]
mod tests {
	use rstest::rstest;
	use serde_json::json;

	use super::*;
	use crate::strategicpatch::schema::BuiltinSchemaLookup;

	fn deployment_schema() -> BuiltinSchemaLookup {
		BuiltinSchemaLookup::new("apps/v1", "Deployment")
	}

	fn podspec_schema() -> BuiltinSchemaLookup {
		BuiltinSchemaLookup::for_type("io.k8s.api.core.v1.PodSpec")
	}

	fn container_schema() -> BuiltinSchemaLookup {
		BuiltinSchemaLookup::for_type("io.k8s.api.core.v1.Container")
	}

	fn service_spec_schema() -> BuiltinSchemaLookup {
		// Use SSA merge type for composite key tests
		BuiltinSchemaLookup::for_type_with_merge_type(
			"io.k8s.api.core.v1.ServiceSpec",
			crate::strategicpatch::MergeType::ServerSideApply,
		)
	}

	#[rstest]
	#[case::scalar_change(
        json!({"name": "old"}),
        json!({"name": "new"}),
        json!({"name": "new"})
    )]
	#[case::no_change(
        json!({"name": "same"}),
        json!({"name": "same"}),
        json!({})
    )]
	#[case::addition(
        json!({"a": 1}),
        json!({"a": 1, "b": 2}),
        json!({"b": 2})
    )]
	#[case::deletion(
        json!({"a": 1, "b": 2}),
        json!({"a": 1}),
        json!({"b": null})
    )]
	#[case::nested_object(
        json!({"spec": {"replicas": 1}}),
        json!({"spec": {"replicas": 3}}),
        json!({"spec": {"replicas": 3}})
    )]
	fn test_diff_values_basic(
		#[case] from: serde_json::Value,
		#[case] to: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &deployment_schema(), "", &options).unwrap();
		assert_eq!(diff, expected);
	}

	#[test]
	fn test_diff_ignore_deletions() {
		let from = json!({"a": 1, "b": 2});
		let to = json!({"a": 1});
		let options = DiffOptions {
			ignore_deletions: true,
			..Default::default()
		};
		let diff = diff_values(&from, &to, &deployment_schema(), "", &options).unwrap();
		assert_eq!(diff, json!({}));
	}

	#[rstest]
	#[case::container_image_update(
        json!({"containers": [{"name": "app", "image": "nginx:1.0"}]}),
        json!({"containers": [{"name": "app", "image": "nginx:2.0"}]}),
        json!({"containers": [{"name": "app", "image": "nginx:2.0"}]})
    )]
	#[case::container_deletion(
        json!({"containers": [{"name": "app", "image": "nginx"}, {"name": "sidecar", "image": "proxy"}]}),
        json!({"containers": [{"name": "app", "image": "nginx"}]}),
        json!({"containers": [{"name": "sidecar", "$patch": "delete"}]})
    )]
	fn test_diff_podspec_containers(
		#[case] from: serde_json::Value,
		#[case] to: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &podspec_schema(), "", &options).unwrap();
		assert_eq!(diff, expected);
	}

	#[rstest]
	#[case::basic_deployment_update(
        None,
        json!({
            "spec": {
                "replicas": 3,
                "template": {"spec": {"containers": [{"name": "nginx", "image": "nginx:1.25"}]}}
            }
        }),
        json!({
            "spec": {
                "replicas": 1,
                "template": {"spec": {"containers": [{"name": "nginx", "image": "nginx:1.24"}]}}
            }
        }),
        json!({
            "spec": {
                "replicas": 3,
                "template": {
                    "spec": {
                        "containers": [{"name": "nginx", "image": "nginx:1.25"}],
                        "$setElementOrder/containers": [{"name": "nginx"}]
                    }
                }
            }
        })
    )]
	#[case::add_new_container(
        None,
        json!({
            "spec": {"template": {"spec": {"containers": [
                {"name": "app", "image": "nginx"},
                {"name": "sidecar", "image": "proxy:1.0"}
            ]}}}
        }),
        json!({
            "spec": {"template": {"spec": {"containers": [{"name": "app", "image": "nginx"}]}}}
        }),
        json!({
            "spec": {"template": {"spec": {
                "containers": [{"name": "sidecar", "image": "proxy:1.0"}],
                "$setElementOrder/containers": [{"name": "app"}, {"name": "sidecar"}]
            }}}
        })
    )]
	#[case::multiple_container_changes(
        None,
        json!({
            "spec": {"template": {"spec": {"containers": [
                {"name": "app", "image": "nginx:2.0", "resources": {"limits": {"cpu": "200m"}}},
                {"name": "logger", "image": "fluentd:2.0"}
            ]}}}
        }),
        json!({
            "spec": {"template": {"spec": {"containers": [
                {"name": "app", "image": "nginx:1.0", "resources": {"limits": {"cpu": "100m"}}},
                {"name": "logger", "image": "fluentd:1.0"}
            ]}}}
        }),
        json!({
            "spec": {"template": {"spec": {
                "containers": [
                    {"name": "app", "image": "nginx:2.0", "resources": {"limits": {"cpu": "200m"}}},
                    {"name": "logger", "image": "fluentd:2.0"}
                ],
                "$setElementOrder/containers": [{"name": "app"}, {"name": "logger"}]
            }}}
        })
    )]
	#[case::preserve_server_added_container(
        Some(json!({"spec": {"template": {"spec": {"containers": [{"name": "app", "image": "nginx:1.0"}]}}}})),
        json!({"spec": {"template": {"spec": {"containers": [{"name": "app", "image": "nginx:2.0"}]}}}}),
        json!({"spec": {"template": {"spec": {"containers": [
            {"name": "app", "image": "nginx:1.0"},
            {"name": "istio-proxy", "image": "istio/proxyv2:1.19"}
        ]}}}}),
        // Server-injected container (istio-proxy) is included in $setElementOrder
        // to preserve its relative position after app
        json!({
            "spec": {"template": {"spec": {
                "containers": [{"name": "app", "image": "nginx:2.0"}],
                "$setElementOrder/containers": [{"name": "app"}, {"name": "istio-proxy"}]
            }}}
        })
    )]
	fn test_three_way_merge(
		#[case] original: Option<serde_json::Value>,
		#[case] modified: serde_json::Value,
		#[case] current: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		let patch = create_three_way_merge_patch(
			original.as_ref(),
			&modified,
			&current,
			&deployment_schema(),
		)
		.unwrap();
		assert_eq!(patch, expected);
	}

	#[rstest]
	#[case::delete_container_from_original(
        json!({"spec": {"template": {"spec": {"containers": [
            {"name": "app", "image": "nginx"},
            {"name": "sidecar", "image": "proxy"}
        ]}}}}),
        json!({"spec": {"template": {"spec": {"containers": [{"name": "app", "image": "nginx:latest"}]}}}}),
        json!({"spec": {"template": {"spec": {"containers": [
            {"name": "app", "image": "nginx"},
            {"name": "sidecar", "image": "proxy"}
        ]}}}}),
        json!({
            "spec": {"template": {"spec": {
                "containers": [
                    {"name": "app", "image": "nginx:latest"},
                    {"name": "sidecar", "$patch": "delete"}
                ],
                "$setElementOrder/containers": [{"name": "app"}]
            }}}
        })
    )]
	#[case::env_var_merge_with_server_injected(
        json!({"spec": {"template": {"spec": {"containers": [{
            "name": "app",
            "env": [{"name": "LOG_LEVEL", "value": "info"}]
        }]}}}}),
        json!({"spec": {"template": {"spec": {"containers": [{
            "name": "app",
            "env": [
                {"name": "LOG_LEVEL", "value": "debug"},
                {"name": "NEW_VAR", "value": "test"}
            ]
        }]}}}}),
        json!({"spec": {"template": {"spec": {"containers": [{
            "name": "app",
            "env": [
                {"name": "LOG_LEVEL", "value": "info"},
                {"name": "SERVER_INJECTED", "value": "true"}
            ]
        }]}}}}),
        json!({
            "spec": {"template": {"spec": {
                "containers": [{
                    "name": "app",
                    "env": [
                        {"name": "LOG_LEVEL", "value": "debug"},
                        {"name": "NEW_VAR", "value": "test"}
                    ],
                    "$setElementOrder/env": [{"name": "LOG_LEVEL"}, {"name": "NEW_VAR"}]
                }],
                "$setElementOrder/containers": [{"name": "app"}]
            }}}
        })
    )]
	#[case::volume_merge_with_server_injected(
        json!({"spec": {"template": {"spec": {"volumes": [
            {"name": "config", "configMap": {"name": "app-config"}}
        ]}}}}),
        json!({"spec": {"template": {"spec": {"volumes": [
            {"name": "config", "configMap": {"name": "app-config-v2"}},
            {"name": "data", "emptyDir": {}}
        ]}}}}),
        json!({"spec": {"template": {"spec": {"volumes": [
            {"name": "config", "configMap": {"name": "app-config"}},
            {"name": "secret-vol", "secret": {"secretName": "injected"}}
        ]}}}}),
        // Server-injected volume (secret-vol) is included in $setElementOrder
        // after config (where it appears in current) to preserve its position
        json!({
            "spec": {"template": {"spec": {
                "volumes": [
                    {"name": "config", "configMap": {"name": "app-config-v2"}},
                    {"name": "data", "emptyDir": {}}
                ],
                "$setElementOrder/volumes": [{"name": "config"}, {"name": "secret-vol"}, {"name": "data"}]
            }}}
        })
    )]
	fn test_three_way_merge_with_original(
		#[case] original: serde_json::Value,
		#[case] modified: serde_json::Value,
		#[case] current: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		let patch = create_three_way_merge_patch(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
		)
		.unwrap();
		assert_eq!(patch, expected);
	}

	#[rstest]
	#[case::volume_mounts_by_path(
        json!({"volumeMounts": [
            {"name": "config", "mountPath": "/etc/config"},
            {"name": "data", "mountPath": "/data"}
        ]}),
        json!({"volumeMounts": [
            {"name": "config", "mountPath": "/etc/config", "readOnly": true},
            {"name": "data", "mountPath": "/data"}
        ]}),
        json!({"volumeMounts": [{"mountPath": "/etc/config", "readOnly": true}]})
    )]
	fn test_container_merge_keys(
		#[case] from: serde_json::Value,
		#[case] to: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &container_schema(), "", &options).unwrap();
		assert_eq!(diff, expected);
	}

	#[test]
	fn test_no_change_same_content() {
		let content = json!({
			"spec": {
				"replicas": 3,
				"template": {"spec": {"containers": [{"name": "app", "image": "nginx:1.0"}]}}
			}
		});

		let patch =
			create_three_way_merge_patch(None, &content, &content, &deployment_schema()).unwrap();
		assert_eq!(patch, json!({}));
	}

	#[test]
	fn test_three_way_no_original_no_deletions() {
		// Without original (no last-applied-configuration), we can't determine
		// what was intentionally deleted. Extra containers in current should
		// NOT generate delete directives.
		let modified = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:2.0"}
			]}}}
		});
		let current = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"},
				{"name": "server-injected", "image": "sidecar:1.0"},
				{"name": "another-injected", "image": "proxy:1.0"}
			]}}}
		});

		let patch =
			create_three_way_merge_patch(None, &modified, &current, &deployment_schema()).unwrap();

		// Should only update the app container, NO delete directives for server-injected containers
		assert_eq!(
			patch,
			json!({
				"spec": {"template": {"spec": {
					"containers": [{"name": "app", "image": "nginx:2.0"}],
					"$setElementOrder/containers": [{"name": "app"}]
				}}}
			})
		);
	}

	#[test]
	fn test_three_way_with_original_has_deletions() {
		// WITH original, we CAN determine what was intentionally deleted
		let original = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"},
				{"name": "user-added-sidecar", "image": "sidecar:1.0"}
			]}}}
		});
		let modified = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:2.0"}
			]}}}
		});
		let current = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"},
				{"name": "user-added-sidecar", "image": "sidecar:1.0"}
			]}}}
		});

		let patch = create_three_way_merge_patch(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
		)
		.unwrap();

		// Should have delete directive because user-added-sidecar was in original
		// but removed in modified (intentional deletion)
		assert_eq!(
			patch,
			json!({
				"spec": {"template": {"spec": {
					"containers": [
						{"name": "app", "image": "nginx:2.0"},
						{"name": "user-added-sidecar", "$patch": "delete"}
					],
					"$setElementOrder/containers": [{"name": "app"}]
				}}}
			})
		);
	}

	#[test]
	fn test_conflict_detection_detects_conflict() {
		// Original: replicas: 1
		// Modified: replicas: 3 (user wants 3)
		// Current: replicas: 2 (server changed to 2)
		// Should conflict because both changed the same field differently
		let original = json!({"spec": {"replicas": 1}});
		let modified = json!({"spec": {"replicas": 3}});
		let current = json!({"spec": {"replicas": 2}});

		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			false, // Don't overwrite - detect conflicts
			&[],
		);

		assert!(matches!(result, Err(DiffError::Conflict { .. })));
		if let Err(DiffError::Conflict {
			path,
			modified,
			current,
		}) = result
		{
			assert_eq!(path, "spec.replicas");
			assert_eq!(modified, "3");
			assert_eq!(current, "2");
		}
	}

	#[test]
	fn test_conflict_detection_no_conflict_when_same_value() {
		// Original: replicas: 1
		// Modified: replicas: 3
		// Current: replicas: 3 (server already set to same value user wants)
		// No conflict - they agree
		let original = json!({"spec": {"replicas": 1}});
		let modified = json!({"spec": {"replicas": 3}});
		let current = json!({"spec": {"replicas": 3}});

		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			false,
			&[],
		);

		// Current already matches modified for replicas, so no patch needed
		assert_eq!(result.unwrap(), json!({}));
	}

	#[test]
	fn test_conflict_detection_no_conflict_when_different_fields() {
		// Original: replicas: 1
		// Modified: replicas: 3
		// Current: replicas: 1, image updated (server changed different field)
		// No conflict - different fields
		let original = json!({"spec": {"replicas": 1}});
		let modified = json!({"spec": {"replicas": 3}});
		let current = json!({"spec": {"replicas": 1, "paused": true}});

		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			false,
			&[],
		);

		// Patch should change replicas from 1 to 3
		assert_eq!(result.unwrap(), json!({"spec": {"replicas": 3}}));
	}

	#[test]
	fn test_conflict_detection_overwrite_ignores_conflicts() {
		// Same scenario as test_conflict_detection_detects_conflict
		// but with overwrite=true, so no conflict error
		let original = json!({"spec": {"replicas": 1}});
		let modified = json!({"spec": {"replicas": 3}});
		let current = json!({"spec": {"replicas": 2}});

		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			true, // Overwrite - ignore conflicts
			&[],
		);

		assert!(result.is_ok());
		assert_eq!(result.unwrap(), json!({"spec": {"replicas": 3}}));
	}

	#[test]
	fn test_conflict_detection_array_element_conflict() {
		// User and server both modify the same container's image to different values
		let original = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"}
			]}}}
		});
		let modified = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:2.0"}
			]}}}
		});
		let current = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.5"}
			]}}}
		});

		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			false,
			&[],
		);

		assert!(matches!(result, Err(DiffError::Conflict { .. })));
	}

	#[test]
	fn test_conflict_detection_array_no_conflict_different_elements() {
		// User modifies container "app", server modifies container "sidecar"
		// No conflict - different elements
		let original = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"},
				{"name": "sidecar", "image": "proxy:1.0"}
			]}}}
		});
		let modified = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:2.0"},
				{"name": "sidecar", "image": "proxy:1.0"}
			]}}}
		});
		let current = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"},
				{"name": "sidecar", "image": "proxy:2.0"}
			]}}}
		});

		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			false,
			&[],
		);

		// Patch should update app's image (user's change) and revert sidecar's image
		// (to match user's desired state). Includes $setElementOrder.
		assert_eq!(
			result.unwrap(),
			json!({
				"spec": {
					"template": {
						"spec": {
							"$setElementOrder/containers": [
								{"name": "app"},
								{"name": "sidecar"}
							],
							"containers": [
								{"image": "nginx:2.0", "name": "app"},
								{"image": "proxy:1.0", "name": "sidecar"}
							]
						}
					}
				}
			})
		);
	}

	#[test]
	fn test_precondition_fails_when_key_changed() {
		// Precondition requires that "spec" is not changed
		// But our patch changes spec.replicas, so precondition should fail
		let original = json!({"spec": {"replicas": 1}});
		let modified = json!({"spec": {"replicas": 3}});
		let current = json!({"spec": {"replicas": 1}});

		let preconditions = [require_key_unchanged("spec")];
		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			true,
			&preconditions,
		);

		assert!(matches!(
			result,
			Err(DiffError::PreconditionFailed { key }) if key == "spec"
		));
	}

	#[test]
	fn test_precondition_passes_when_key_unchanged() {
		// Precondition requires that "metadata" is not changed
		// Our patch only changes spec, so precondition should pass
		let original = json!({"spec": {"replicas": 1}});
		let modified = json!({"spec": {"replicas": 3}});
		let current = json!({"spec": {"replicas": 1}});

		let preconditions = [require_key_unchanged("metadata")];
		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			true,
			&preconditions,
		);

		// Patch should change replicas from 1 to 3
		assert_eq!(result.unwrap(), json!({"spec": {"replicas": 3}}));
	}

	#[test]
	fn test_precondition_metadata_key() {
		// Test require_metadata_key_unchanged
		let original = json!({"metadata": {"name": "foo"}, "spec": {"replicas": 1}});
		let modified = json!({"metadata": {"name": "bar"}, "spec": {"replicas": 1}});
		let current = json!({"metadata": {"name": "foo"}, "spec": {"replicas": 1}});

		let preconditions = [require_metadata_key_unchanged("name")];
		let result = create_three_way_merge_patch_with_options(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
			true,
			&preconditions,
		);

		assert!(matches!(
			result,
			Err(DiffError::PreconditionFailed { key }) if key == "metadata.name"
		));
	}

	#[test]
	fn test_server_injected_element_order_preserved() {
		// When user adds/reorders elements while server has injected elements,
		// the $setElementOrder should include server-injected elements at their
		// relative positions to preserve ordering.
		//
		// - Original: [app]
		// - Modified: [sidecar, app] (user adds sidecar first)
		// - Current: [app, istio-proxy] (server injected istio after app)
		// Expected: $setElementOrder includes istio-proxy after app
		let original = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"}
			]}}}
		});
		let modified = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "sidecar", "image": "proxy:1.0"},
				{"name": "app", "image": "nginx:1.0"}
			]}}}
		});
		let current = json!({
			"spec": {"template": {"spec": {"containers": [
				{"name": "app", "image": "nginx:1.0"},
				{"name": "istio-proxy", "image": "istio/proxyv2:1.20"}
			]}}}
		});

		let patch = create_three_way_merge_patch(
			Some(&original),
			&modified,
			&current,
			&deployment_schema(),
		)
		.unwrap();

		// Server-injected istio-proxy should be in $setElementOrder after app
		// to preserve its relative position
		assert_eq!(
			patch,
			json!({
				"spec": {"template": {"spec": {
					"containers": [{"name": "sidecar", "image": "proxy:1.0"}],
					"$setElementOrder/containers": [
						{"name": "sidecar"},
						{"name": "app"},
						{"name": "istio-proxy"}
					]
				}}}
			})
		);
	}

	#[test]
	fn test_diff_delete_all_from_merging_list_generates_replace() {
		// When ALL items are deleted from a merging list, generate $patch: replace
		// instead of individual delete directives
		let from = json!({
			"containers": [
				{"name": "app", "image": "nginx"},
				{"name": "sidecar", "image": "proxy"}
			]
		});
		let to = json!({
			"containers": []
		});
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &podspec_schema(), "", &options).unwrap();

		assert_eq!(
			diff,
			json!({
				"containers": [{"$patch": "replace"}]
			})
		);
	}

	#[test]
	fn test_diff_primitive_array_deletion() {
		// When items are removed from a primitive array, generate $deleteFromPrimitiveList
		let from = json!({
			"finalizers": ["kubernetes.io/pv-protection", "custom-finalizer"]
		});
		let to = json!({
			"finalizers": ["kubernetes.io/pv-protection"]
		});
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &deployment_schema(), "", &options).unwrap();

		// Should generate the $deleteFromPrimitiveList directive
		assert_eq!(
			diff,
			json!({
				"$deleteFromPrimitiveList/finalizers": ["custom-finalizer"]
			})
		);
	}

	#[test]
	fn test_diff_primitive_array_no_change() {
		// Identical primitive arrays should produce no diff
		let from = json!({
			"finalizers": ["kubernetes.io/pv-protection", "custom-finalizer"]
		});
		let to = json!({
			"finalizers": ["kubernetes.io/pv-protection", "custom-finalizer"]
		});
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &deployment_schema(), "", &options).unwrap();

		assert_eq!(diff, json!({}));
	}

	#[test]
	fn test_diff_primitive_array_addition() {
		// When items are added to a primitive array, include the new array
		let from = json!({
			"finalizers": ["kubernetes.io/pv-protection"]
		});
		let to = json!({
			"finalizers": ["kubernetes.io/pv-protection", "custom-finalizer"]
		});
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &deployment_schema(), "", &options).unwrap();

		// Should include the full array for additions
		assert_eq!(
			diff,
			json!({
				"finalizers": ["kubernetes.io/pv-protection", "custom-finalizer"]
			})
		);
	}

	#[test]
	fn test_diff_primitive_array_both_add_and_delete() {
		// When both additions and deletions occur
		let from = json!({
			"args": ["--verbose", "--old-flag"]
		});
		let to = json!({
			"args": ["--verbose", "--new-flag"]
		});
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &deployment_schema(), "", &options).unwrap();

		// Should include both the new array and the deletion directive
		assert_eq!(
			diff,
			json!({
				"args": ["--verbose", "--new-flag"],
				"$deleteFromPrimitiveList/args": ["--old-flag"]
			})
		);
	}

	#[test]
	fn test_diff_retain_keys_volume_source_change() {
		// When changing volume source type in a retainKeys field,
		// should generate $retainKeys instead of null for removed keys
		let from = json!({
			"name": "my-vol",
			"configMap": {"name": "my-config"}
		});
		let to = json!({
			"name": "my-vol",
			"secret": {"secretName": "my-secret"}
		});

		let options = DiffOptions::default();
		// Use a path that indicates this is inside a volumes array (which has retainKeys)
		let diff = diff_values(&from, &to, &podspec_schema(), "volumes.my-vol", &options).unwrap();

		assert_eq!(
			diff,
			json!({
				"secret": {"secretName": "my-secret"},
				"$retainKeys": ["name", "secret"]
			})
		);
	}

	#[test]
	fn test_diff_retain_keys_nested() {
		// Deletions within nested objects of a retainKeys field should still
		// use $retainKeys (not null). This tests that retainKeys is inherited
		// by nested paths, not just immediate children.
		let from = json!({
			"name": "my-vol",
			"configMap": {
				"name": "my-config",
				"defaultMode": 420,
				"items": [{"key": "config.yaml", "path": "config.yaml"}]
			}
		});
		let to = json!({
			"name": "my-vol",
			"configMap": {
				"name": "my-config-v2"
				// defaultMode and items removed
			}
		});

		let options = DiffOptions::default();
		// Path indicates we're inside a volumes array element
		let diff = diff_values(&from, &to, &podspec_schema(), "volumes.my-vol", &options).unwrap();

		// Deletions within the configMap nested object should use $retainKeys
		// because volumes has retainKeys strategy
		assert_eq!(
			diff,
			json!({
				"configMap": {
					"name": "my-config-v2",
					"$retainKeys": ["name"]
				}
			})
		);
	}

	#[test]
	fn test_diff_no_retain_keys_for_non_retainkeys_field() {
		// Regular fields should still use null for deletions
		let from = json!({
			"name": "test",
			"image": "nginx",
			"command": ["/bin/sh"]
		});
		let to = json!({
			"name": "test",
			"image": "nginx"
		});

		let options = DiffOptions::default();
		// containers don't have retainKeys strategy
		let diff = diff_values(&from, &to, &podspec_schema(), "containers.test", &options).unwrap();

		assert_eq!(
			diff,
			json!({
				"command": null
			})
		);
	}

	#[rstest]
	#[case::update_one_protocol(
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]}),
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 9090},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]}),
		json!({"ports": [{"port": 80, "protocol": "TCP", "targetPort": 9090}]})
	)]
	#[case::delete_one_protocol(
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]}),
		json!({"ports": [{"port": 80, "protocol": "TCP", "targetPort": 8080}]}),
		json!({"ports": [{"port": 80, "protocol": "UDP", "$patch": "delete"}]})
	)]
	#[case::add_protocol(
		json!({"ports": [{"port": 80, "protocol": "TCP", "targetPort": 8080}]}),
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 80, "protocol": "UDP", "targetPort": 8080}
		]}),
		json!({"ports": [{"port": 80, "protocol": "UDP", "targetPort": 8080}]})
	)]
	#[case::no_change(
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 443, "protocol": "TCP", "targetPort": 8443}
		]}),
		json!({"ports": [
			{"port": 80, "protocol": "TCP", "targetPort": 8080},
			{"port": 443, "protocol": "TCP", "targetPort": 8443}
		]}),
		json!({})
	)]
	fn test_composite_key_service_ports(
		#[case] from: serde_json::Value,
		#[case] to: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		// Default options don't include $setElementOrder
		let options = DiffOptions::default();
		let diff = diff_values(&from, &to, &service_spec_schema(), "", &options).unwrap();
		assert_eq!(diff, expected);
	}

	#[rstest]
	#[case::single_key(
		json!({"name": "app", "image": "nginx"}),
		vec!["name"],
		Some("app")
	)]
	#[case::multi_key(
		json!({"port": 80, "protocol": "TCP", "targetPort": 8080}),
		vec!["port", "protocol"],
		Some("80::TCP")
	)]
	#[case::missing_one_key(
		json!({"port": 80}),
		vec!["port", "protocol"],
		Some("80::")
	)]
	#[case::all_keys_missing(
		json!({"other": "value"}),
		vec!["port", "protocol"],
		None
	)]
	fn test_get_composite_key_value(
		#[case] item: serde_json::Value,
		#[case] keys: Vec<&str>,
		#[case] expected: Option<&str>,
	) {
		let keys: Vec<String> = keys.into_iter().map(|s| s.to_string()).collect();
		let merge_keys = MergeKeys::Borrowed(&keys);
		assert_eq!(
			get_composite_key_value(&item, &merge_keys),
			expected.map(|s| s.to_string())
		);
	}
}
