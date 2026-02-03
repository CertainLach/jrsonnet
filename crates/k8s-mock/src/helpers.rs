//! Helper functions for mock Kubernetes testing.

/// Deep merge two JSON values (patch into base).
pub fn merge_json(base: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
	match (base, patch) {
		(serde_json::Value::Object(mut base_map), serde_json::Value::Object(patch_map)) => {
			for (key, patch_value) in patch_map {
				let base_value = base_map.remove(&key).unwrap_or(serde_json::Value::Null);
				base_map.insert(key, merge_json(base_value, patch_value));
			}
			serde_json::Value::Object(base_map)
		}
		(_, patch) => patch,
	}
}

/// Strip strategic merge patch directives from a JSON value.
///
/// Strategic merge patch uses special keys like `$setElementOrder/xxx`, `$patch`,
/// and `$retainKeys` to control merge behavior. These are instructions for the
/// server, not actual resource content, so they should not appear in the response.
pub fn strip_strategic_merge_directives(value: serde_json::Value) -> serde_json::Value {
	match value {
		serde_json::Value::Object(map) => {
			let cleaned: serde_json::Map<String, serde_json::Value> = map
				.into_iter()
				.filter(|(key, _)| !is_strategic_directive(key))
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

/// Check if a key is a strategic merge patch directive.
///
/// All K8s strategic merge patch directives start with `$`.
fn is_strategic_directive(key: &str) -> bool {
	key.starts_with('$')
}
