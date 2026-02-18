//! Helper functions for mock Kubernetes testing.

use k8s::strategicpatch::{merge::apply_strategic_patch, schema::BuiltinSchemaLookup, MergeType};

/// Deep merge two JSON values using strategic merge patch semantics with a specific merge type.
///
/// This is a convenience wrapper that extracts apiVersion/kind from the patch
/// to determine the correct merge keys.
pub fn merge_json_with_type(
	base: serde_json::Value,
	patch: serde_json::Value,
	merge_type: MergeType,
) -> serde_json::Value {
	// Try to extract apiVersion and kind from the patch for schema lookup
	let api_version = patch
		.get("apiVersion")
		.and_then(|v| v.as_str())
		.or_else(|| base.get("apiVersion").and_then(|v| v.as_str()))
		.unwrap_or("")
		.to_string();
	let kind = patch
		.get("kind")
		.and_then(|v| v.as_str())
		.or_else(|| base.get("kind").and_then(|v| v.as_str()))
		.unwrap_or("")
		.to_string();

	let schema = BuiltinSchemaLookup::with_merge_type(&api_version, &kind, merge_type);
	apply_strategic_patch(&base, &patch, &schema, "").unwrap_or(patch)
}

/// Strip strategic merge patch directives from a JSON value.
///
/// Strategic merge patch uses special keys like `$setElementOrder/xxx` and
/// `$retainKeys` to control merge behavior. These are instructions for the
/// server, not actual resource content, so they should not appear in the response.
///
/// Note: `$patch: delete` directives are handled by `apply_strategic_patch` and
/// won't appear in the result, so we don't need to filter those here.
pub fn strip_strategic_merge_directives(value: serde_json::Value) -> serde_json::Value {
	match value {
		serde_json::Value::Object(map) => {
			let cleaned: serde_json::Map<String, serde_json::Value> = map
				.into_iter()
				.filter(|(key, _)| !key.starts_with('$'))
				.map(|(key, val)| (key, strip_strategic_merge_directives(val)))
				.collect();
			serde_json::Value::Object(cleaned)
		}
		serde_json::Value::Array(arr) => serde_json::Value::Array(
			arr.into_iter()
				.map(strip_strategic_merge_directives)
				.collect(),
		),
		other => other,
	}
}

/// Strip empty metadata fields from a K8s resource.
///
/// Real Kubernetes API servers don't include empty annotations/labels in responses.
/// This matches that behavior for more realistic mock responses.
pub fn strip_empty_metadata_fields(value: serde_json::Value) -> serde_json::Value {
	let serde_json::Value::Object(mut map) = value else {
		return value;
	};

	let Some(serde_json::Value::Object(mut metadata)) = map.remove("metadata") else {
		return serde_json::Value::Object(map);
	};

	// Remove empty annotations
	if matches!(metadata.get("annotations"), Some(serde_json::Value::Object(m)) if m.is_empty()) {
		metadata.remove("annotations");
	}

	// Remove empty labels
	if matches!(metadata.get("labels"), Some(serde_json::Value::Object(m)) if m.is_empty()) {
		metadata.remove("labels");
	}

	map.insert("metadata".to_string(), serde_json::Value::Object(metadata));
	serde_json::Value::Object(map)
}
