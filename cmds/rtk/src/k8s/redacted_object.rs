//! Type-safe redaction for Kubernetes objects.
//!
//! This module provides types that enforce secret masking at the type level,
//! following kubectl's approach to preventing accidental exposure of sensitive data.
//!
//! The pattern uses two types:
//! - [`UnredactedObject`]: Cannot be displayed - forces conversion to redacted form
//! - [`RedactedObject`]: Safe for display, with sensitive data masked
//!
//! This design makes it impossible to accidentally print unredacted secrets.

use std::collections::HashSet;

use kube::core::GroupVersionKind;
use serde_json::Value;

/// Constants matching kubectl's masking behavior.
pub mod mask {
	/// Default mask for secret values (when comparing equal values or single object).
	pub const DEFAULT: &str = "***";
	/// Mask for "before" value when secrets differ.
	pub const BEFORE: &str = "*** (before)";
	/// Mask for "after" value when secrets differ.
	pub const AFTER: &str = "*** (after)";
}

/// An unredacted Kubernetes object.
///
/// **This type cannot be displayed.** There is intentionally no `Display` trait
/// implementation, and `Debug` only shows metadata, not the actual content.
///
/// To display the object, convert it to a [`RedactedObject`] using:
/// - [`RedactedObject::from()`] for single objects
/// - [`RedactedObject::redact_pair()`] for diff comparisons
pub struct UnredactedObject {
	value: Value,
	gvk: GroupVersionKind,
}

impl std::fmt::Debug for UnredactedObject {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UnredactedObject")
			.field("gvk", &self.gvk)
			.field(
				"value",
				&"[REDACTED - convert to RedactedObject to display]",
			)
			.finish()
	}
}

impl UnredactedObject {
	/// Create a new unredacted object.
	pub fn new(value: Value, gvk: GroupVersionKind) -> Self {
		Self { value, gvk }
	}

	/// Construct an empty object for a resource kind.
	///
	/// This represents kubectl's typed-nil behavior in diff masking paths.
	pub fn empty(gvk: GroupVersionKind) -> Self {
		Self {
			value: Value::Null,
			gvk,
		}
	}

	/// Check if this is a v1.Secret.
	fn is_secret(&self) -> bool {
		self.gvk.group.is_empty() && self.gvk.version == "v1" && self.gvk.kind == "Secret"
	}

	/// Get the data field keys if this is a Secret.
	fn secret_data_keys(&self) -> Option<HashSet<String>> {
		if !self.is_secret() {
			return None;
		}

		self.value
			.get("data")
			.and_then(|d| d.as_object())
			.map(|obj| obj.keys().cloned().collect())
	}
}

/// A redacted Kubernetes object, safe for display.
///
/// Sensitive data (like Secret `.data` values) has been masked.
/// This type can be safely converted to YAML for display.
pub struct RedactedObject {
	value: Value,
	#[allow(dead_code)]
	gvk: GroupVersionKind,
}

impl std::fmt::Debug for RedactedObject {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RedactedObject")
			.field("gvk", &self.gvk)
			.field("value", &self.value)
			.finish()
	}
}

impl RedactedObject {
	/// Convert this object to a YAML string.
	pub fn to_yaml(&self) -> Result<String, serde_saphyr::ser_error::Error> {
		crate::yaml::to_yaml(&self.value)
	}

	/// Redact a pair of objects for diff comparison.
	///
	/// This handles the special case where we need to show whether secret values
	/// changed between versions:
	/// - Equal values: both masked as `***`
	/// - Different values: `*** (before)` and `*** (after)`
	/// - Key only in one: masked as `***`
	///
	/// This matches kubectl's `Masker` behavior.
	pub fn redact_pair(current: UnredactedObject, desired: UnredactedObject) -> (Self, Self) {
		// Only mask if both are Secrets (or either is a Secret being created/deleted)
		let current_is_secret = current.is_secret();
		let desired_is_secret = desired.is_secret();

		if !current_is_secret && !desired_is_secret {
			// Neither is a secret, no masking needed
			return (
				Self {
					value: current.value,
					gvk: current.gvk,
				},
				Self {
					value: desired.value,
					gvk: desired.gvk,
				},
			);
		}

		let current_keys = current.secret_data_keys();
		let desired_keys = desired.secret_data_keys();

		let mut current_value = current.value;
		let mut desired_value = desired.value;

		// Collect all keys from both secrets
		let all_keys: HashSet<String> = current_keys
			.iter()
			.flatten()
			.chain(desired_keys.iter().flatten())
			.cloned()
			.collect();

		for key in all_keys {
			let current_has = current_value
				.get("data")
				.and_then(|d| d.get(&key))
				.is_some();
			let desired_has = desired_value
				.get("data")
				.and_then(|d| d.get(&key))
				.is_some();

			let current_val = current_value
				.get("data")
				.and_then(|d| d.get(&key))
				.and_then(|v| v.as_str());
			let desired_val = desired_value
				.get("data")
				.and_then(|d| d.get(&key))
				.and_then(|v| v.as_str());

			match (current_has, desired_has) {
				(true, true) => {
					// Both have the key - check if values are equal
					let values_equal = current_val == desired_val;
					if values_equal {
						// Same value - mask both with default
						set_data_key(&mut current_value, &key, mask::DEFAULT);
						set_data_key(&mut desired_value, &key, mask::DEFAULT);
					} else {
						// Different values - use before/after masks
						set_data_key(&mut current_value, &key, mask::BEFORE);
						set_data_key(&mut desired_value, &key, mask::AFTER);
					}
				}
				(true, false) => {
					// Key only in current (being deleted)
					set_data_key(&mut current_value, &key, mask::DEFAULT);
				}
				(false, true) => {
					// Key only in desired (being added)
					set_data_key(&mut desired_value, &key, mask::DEFAULT);
				}
				(false, false) => unreachable!(),
			}
		}

		(
			Self {
				value: current_value,
				gvk: current.gvk,
			},
			Self {
				value: desired_value,
				gvk: desired.gvk,
			},
		)
	}
}

impl From<UnredactedObject> for RedactedObject {
	/// Convert an unredacted object to a redacted one.
	///
	/// For Secrets, all `.data` values are masked with `***`.
	/// For other resources, the value is passed through unchanged.
	fn from(obj: UnredactedObject) -> Self {
		if !obj.is_secret() {
			return Self {
				value: obj.value,
				gvk: obj.gvk,
			};
		}

		let mut value = obj.value;

		// Mask all data keys
		if let Some(data) = value.get("data").and_then(|d| d.as_object()) {
			let keys: Vec<String> = data.keys().cloned().collect();
			for key in keys {
				set_data_key(&mut value, &key, mask::DEFAULT);
			}
		}

		Self {
			value,
			gvk: obj.gvk,
		}
	}
}

/// Helper to set a value in the .data field of a Secret.
fn set_data_key(value: &mut Value, key: &str, mask_value: &str) {
	if let Some(data) = value.get_mut("data").and_then(|d| d.as_object_mut()) {
		data.insert(key.to_string(), Value::String(mask_value.to_string()));
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn secret_gvk() -> GroupVersionKind {
		GroupVersionKind {
			group: String::new(),
			version: "v1".to_string(),
			kind: "Secret".to_string(),
		}
	}

	fn configmap_gvk() -> GroupVersionKind {
		GroupVersionKind {
			group: String::new(),
			version: "v1".to_string(),
			kind: "ConfigMap".to_string(),
		}
	}

	#[test]
	fn test_single_secret_redaction() {
		let secret = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"metadata": {"name": "test", "namespace": "default"},
			"data": {
				"password": "cGFzc3dvcmQ=",
				"username": "YWRtaW4="
			}
		});

		let unredacted = UnredactedObject::new(secret, secret_gvk());
		let redacted = RedactedObject::from(unredacted);

		assert_eq!(
			redacted.value["data"]["password"],
			Value::String("***".to_string())
		);
		assert_eq!(
			redacted.value["data"]["username"],
			Value::String("***".to_string())
		);
	}

	#[test]
	fn test_configmap_not_redacted() {
		let configmap = json!({
			"apiVersion": "v1",
			"kind": "ConfigMap",
			"metadata": {"name": "test", "namespace": "default"},
			"data": {
				"config": "some-value"
			}
		});

		let original_data = configmap["data"]["config"].clone();
		let unredacted = UnredactedObject::new(configmap, configmap_gvk());
		let redacted = RedactedObject::from(unredacted);

		assert_eq!(redacted.value["data"]["config"], original_data);
	}

	#[test]
	fn test_pair_redaction_equal_values() {
		let secret1 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "cGFzc3dvcmQ="}
		});
		let secret2 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "cGFzc3dvcmQ="}
		});

		let (current, desired) = RedactedObject::redact_pair(
			UnredactedObject::new(secret1, secret_gvk()),
			UnredactedObject::new(secret2, secret_gvk()),
		);

		// Equal values should both be masked with default
		assert_eq!(
			current.value["data"]["password"],
			Value::String("***".to_string())
		);
		assert_eq!(
			desired.value["data"]["password"],
			Value::String("***".to_string())
		);
	}

	#[test]
	fn test_pair_redaction_different_values() {
		let secret1 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "b2xk"}
		});
		let secret2 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "bmV3"}
		});

		let (current, desired) = RedactedObject::redact_pair(
			UnredactedObject::new(secret1, secret_gvk()),
			UnredactedObject::new(secret2, secret_gvk()),
		);

		// Different values should use before/after masks
		assert_eq!(
			current.value["data"]["password"],
			Value::String("*** (before)".to_string())
		);
		assert_eq!(
			desired.value["data"]["password"],
			Value::String("*** (after)".to_string())
		);
	}

	#[test]
	fn test_pair_redaction_key_added() {
		let secret1 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {}
		});
		let secret2 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "bmV3"}
		});

		let (current, desired) = RedactedObject::redact_pair(
			UnredactedObject::new(secret1, secret_gvk()),
			UnredactedObject::new(secret2, secret_gvk()),
		);

		// New key should be masked with default
		assert!(current.value["data"].get("password").is_none());
		assert_eq!(
			desired.value["data"]["password"],
			Value::String("***".to_string())
		);
	}

	#[test]
	fn test_pair_redaction_key_removed() {
		let secret1 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "b2xk"}
		});
		let secret2 = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {}
		});

		let (current, desired) = RedactedObject::redact_pair(
			UnredactedObject::new(secret1, secret_gvk()),
			UnredactedObject::new(secret2, secret_gvk()),
		);

		// Removed key should be masked with default in current
		assert_eq!(
			current.value["data"]["password"],
			Value::String("***".to_string())
		);
		assert!(desired.value["data"].get("password").is_none());
	}

	#[test]
	fn test_unredacted_debug_does_not_leak() {
		let secret = json!({
			"apiVersion": "v1",
			"kind": "Secret",
			"data": {"password": "super-secret-value"}
		});

		let unredacted = UnredactedObject::new(secret, secret_gvk());
		let debug_output = format!("{:?}", unredacted);

		// Debug output should NOT contain the secret value
		assert!(!debug_output.contains("super-secret-value"));
		assert!(debug_output.contains("REDACTED"));
	}
}
