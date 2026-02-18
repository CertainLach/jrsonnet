//! Strategic merge patch directives.
//!
//! These directives control how patches are applied to Kubernetes resources:
//! - `$patch: delete` - mark an array element for deletion
//! - `$patch: replace` - replace the entire object/array
//! - `$setElementOrder/fieldName` - specify array element ordering
//! - `$deleteFromPrimitiveList/fieldName` - delete items from primitive arrays
//! - `$retainKeys` - keep only specified keys in the result

/// Directive key for patch operations on objects/arrays.
pub const DIRECTIVE_PATCH: &str = "$patch";

/// Directive key prefix for element ordering in arrays.
pub const DIRECTIVE_SET_ELEMENT_ORDER_PREFIX: &str = "$setElementOrder/";

/// Directive key prefix for deleting from primitive lists.
pub const DIRECTIVE_DELETE_FROM_PRIMITIVE_LIST_PREFIX: &str = "$deleteFromPrimitiveList/";

/// Directive key for retaining only specific keys.
pub const DIRECTIVE_RETAIN_KEYS: &str = "$retainKeys";

/// Patch directive values.
pub mod patch_value {
	/// Delete the element (for arrays) or field (for objects).
	pub const DELETE: &str = "delete";
	/// Replace the entire object/array.
	pub const REPLACE: &str = "replace";
}

/// Check if a key is a strategic merge directive.
pub fn is_directive(key: &str) -> bool {
	key.starts_with('$')
}

/// Check if a key is a $setElementOrder directive.
pub fn is_set_element_order(key: &str) -> bool {
	key.starts_with(DIRECTIVE_SET_ELEMENT_ORDER_PREFIX)
}

/// Extract the field name from a $setElementOrder directive key.
///
/// Returns `Some(field_name)` if the key is a valid $setElementOrder directive,
/// or `None` otherwise.
pub fn extract_set_element_order_field(key: &str) -> Option<&str> {
	key.strip_prefix(DIRECTIVE_SET_ELEMENT_ORDER_PREFIX)
}

/// Check if a key is a $deleteFromPrimitiveList directive.
pub fn is_delete_from_primitive_list(key: &str) -> bool {
	key.starts_with(DIRECTIVE_DELETE_FROM_PRIMITIVE_LIST_PREFIX)
}

/// Extract the field name from a $deleteFromPrimitiveList directive key.
pub fn extract_delete_from_primitive_list_field(key: &str) -> Option<&str> {
	key.strip_prefix(DIRECTIVE_DELETE_FROM_PRIMITIVE_LIST_PREFIX)
}

/// Build a $setElementOrder directive key for a field.
pub fn set_element_order_key(field: &str) -> String {
	format!("{}{}", DIRECTIVE_SET_ELEMENT_ORDER_PREFIX, field)
}

/// Build a $deleteFromPrimitiveList directive key for a field.
pub fn delete_from_primitive_list_key(field: &str) -> String {
	format!("{}{}", DIRECTIVE_DELETE_FROM_PRIMITIVE_LIST_PREFIX, field)
}

/// Check if a patch value indicates deletion.
pub fn is_delete_directive(value: &serde_json::Value) -> bool {
	if let Some(obj) = value.as_object() {
		if let Some(serde_json::Value::String(s)) = obj.get(DIRECTIVE_PATCH) {
			return s == patch_value::DELETE;
		}
	}
	false
}

/// Check if a patch value indicates replacement.
pub fn is_replace_directive(value: &serde_json::Value) -> bool {
	if let Some(obj) = value.as_object() {
		if let Some(serde_json::Value::String(s)) = obj.get(DIRECTIVE_PATCH) {
			return s == patch_value::REPLACE;
		}
	}
	false
}

/// Create a delete directive for an array element with the given merge key.
pub fn create_delete_directive(
	merge_key: &str,
	key_value: &serde_json::Value,
) -> serde_json::Value {
	serde_json::json!({
		merge_key: key_value,
		DIRECTIVE_PATCH: patch_value::DELETE
	})
}

/// Strip all directives from a JSON value.
///
/// This removes all keys starting with `$` from objects recursively.
pub fn strip_directives(value: serde_json::Value) -> serde_json::Value {
	match value {
		serde_json::Value::Object(map) => {
			let cleaned: serde_json::Map<String, serde_json::Value> = map
				.into_iter()
				.filter(|(key, _)| !is_directive(key))
				.map(|(key, val)| (key, strip_directives(val)))
				.collect();
			serde_json::Value::Object(cleaned)
		}
		serde_json::Value::Array(arr) => {
			serde_json::Value::Array(arr.into_iter().map(strip_directives).collect())
		}
		other => other,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_is_directive() {
		assert!(is_directive("$patch"));
		assert!(is_directive("$setElementOrder/containers"));
		assert!(is_directive("$deleteFromPrimitiveList/args"));
		assert!(is_directive("$retainKeys"));
		assert!(!is_directive("containers"));
		assert!(!is_directive("name"));
	}

	#[test]
	fn test_extract_set_element_order_field() {
		assert_eq!(
			extract_set_element_order_field("$setElementOrder/containers"),
			Some("containers")
		);
		assert_eq!(
			extract_set_element_order_field("$setElementOrder/env"),
			Some("env")
		);
		assert_eq!(extract_set_element_order_field("$patch"), None);
		assert_eq!(extract_set_element_order_field("containers"), None);
	}

	#[test]
	fn test_is_delete_directive() {
		let delete = serde_json::json!({"name": "sidecar", "$patch": "delete"});
		assert!(is_delete_directive(&delete));

		let not_delete = serde_json::json!({"name": "sidecar"});
		assert!(!is_delete_directive(&not_delete));

		let replace = serde_json::json!({"name": "sidecar", "$patch": "replace"});
		assert!(!is_delete_directive(&replace));
	}

	#[test]
	fn test_create_delete_directive() {
		let directive = create_delete_directive("name", &serde_json::json!("my-container"));
		assert_eq!(
			directive,
			serde_json::json!({
				"name": "my-container",
				"$patch": "delete"
			})
		);
	}

	#[test]
	fn test_strip_directives() {
		let value = serde_json::json!({
			"containers": [
				{
					"name": "app",
					"image": "nginx"
				}
			],
			"$setElementOrder/containers": [{"name": "app"}],
			"$patch": "replace"
		});

		let stripped = strip_directives(value);
		assert_eq!(
			stripped,
			serde_json::json!({
				"containers": [
					{
						"name": "app",
						"image": "nginx"
					}
				]
			})
		);
	}
}
