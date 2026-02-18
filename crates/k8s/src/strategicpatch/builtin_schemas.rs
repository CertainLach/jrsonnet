//! Built-in strategic merge patch schemas for Kubernetes types.
//!
//! This module contains the merge key information extracted from the Kubernetes API
//! type definitions. These are the same values that kubectl uses for strategic merge
//! patch operations.
//!
//! The data is organized as compile-time nested maps: group -> version -> kind -> field -> info.
//! This allows O(1) lookups at every level and implicit coverage checking.
//!
//! Source: https://github.com/kubernetes/api struct tags and comments

use phf::{phf_map, Map};

use super::types::{FieldSchema, PatchMeta, PatchStrategy};

/// The type of merge operation, which determines which merge keys to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MergeType {
	/// Server-Side Apply: uses `+listMapKey` comments (can be composite keys).
	/// Used when Content-Type is `application/apply-patch+yaml`.
	ServerSideApply,
	/// Strategic Merge Patch: uses `patchMergeKey` struct tag (typically single key).
	/// Used when Content-Type is `application/strategic-merge-patch+json`.
	/// This is the default for compatibility with kubectl.
	#[default]
	StrategicMergePatch,
}

/// Merge key and strategy information for a field.
pub struct FieldInfo {
	/// Merge keys for Server-Side Apply (+listMapKey comments).
	/// Can be composite (multiple keys) for some fields.
	pub ssa_merge_keys: &'static [&'static str],
	/// Merge keys for Strategic Merge Patch (patchMergeKey struct tag).
	/// Typically a single key, used by kubectl for client-side merge.
	pub smp_merge_keys: &'static [&'static str],
	pub strategies: &'static [PatchStrategy],
}

impl FieldInfo {
	/// Get the merge keys for the specified merge type.
	pub fn merge_keys(&self, merge_type: MergeType) -> &'static [&'static str] {
		match merge_type {
			MergeType::ServerSideApply => self.ssa_merge_keys,
			MergeType::StrategicMergePatch => self.smp_merge_keys,
		}
	}
}

/// Merge strategy constants.
const MERGE: &[PatchStrategy] = &[PatchStrategy::Merge];
const MERGE_RETAIN_KEYS: &[PatchStrategy] = &[PatchStrategy::Merge, PatchStrategy::RetainKeys];

/// Type aliases for the nested schema map structure.
/// Maps field name -> field info.
pub type FieldMap = Map<&'static str, FieldInfo>;
/// Maps kind name -> field map.
pub type KindMap = Map<&'static str, FieldMap>;
/// Maps version -> kind map.
pub type VersionMap = Map<&'static str, KindMap>;
/// Maps group -> version map. This is the top-level schema map.
pub type BuiltinSchemaMap = Map<&'static str, VersionMap>;

/// Built-in schemas organized as: group -> version -> kind -> field -> info.
///
/// Group names use the Kubernetes API group naming:
/// - "core" for the core API group (apiVersion: v1)
/// - Full group name for others (e.g., "apps", "batch", "networking")
///
/// Note: Groups with dots in the name (e.g., "networking.k8s.io") use underscores
/// in the OpenAPI type name but the original group name here.
///
pub static BUILTIN_SCHEMAS: BuiltinSchemaMap = phf_map! {
	"core" => phf_map! {
		"v1" => phf_map! {
			"PersistentVolumeClaimStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"Container" => phf_map! {
				"ports" => FieldInfo { ssa_merge_keys: &["containerPort", "protocol"], smp_merge_keys: &["containerPort"], strategies: MERGE },
				"env" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"volumeMounts" => FieldInfo { ssa_merge_keys: &["mountPath"], smp_merge_keys: &["mountPath"], strategies: MERGE },
				"volumeDevices" => FieldInfo { ssa_merge_keys: &["devicePath"], smp_merge_keys: &["devicePath"], strategies: MERGE },
			},
			"EphemeralContainer" => phf_map! {
				"ports" => FieldInfo { ssa_merge_keys: &["containerPort", "protocol"], smp_merge_keys: &["containerPort"], strategies: MERGE },
				"env" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"volumeMounts" => FieldInfo { ssa_merge_keys: &["mountPath"], smp_merge_keys: &["mountPath"], strategies: MERGE },
				"volumeDevices" => FieldInfo { ssa_merge_keys: &["devicePath"], smp_merge_keys: &["devicePath"], strategies: MERGE },
			},
			"ContainerStatus" => phf_map! {
				"volumeMounts" => FieldInfo { ssa_merge_keys: &["mountPath"], smp_merge_keys: &["mountPath"], strategies: MERGE },
				"allocatedResourcesStatus" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"PodSpec" => phf_map! {
				"volumes" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE_RETAIN_KEYS },
				"initContainers" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"containers" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"ephemeralContainers" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"imagePullSecrets" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"hostAliases" => FieldInfo { ssa_merge_keys: &["ip"], smp_merge_keys: &["ip"], strategies: MERGE },
				"topologySpreadConstraints" => FieldInfo { ssa_merge_keys: &["topologyKey", "whenUnsatisfiable"], smp_merge_keys: &["topologyKey"], strategies: MERGE },
				"schedulingGates" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"resourceClaims" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE_RETAIN_KEYS },
			},
			"PodStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
				"hostIPs" => FieldInfo { ssa_merge_keys: &["ip"], smp_merge_keys: &["ip"], strategies: MERGE },
				"podIPs" => FieldInfo { ssa_merge_keys: &["ip"], smp_merge_keys: &["ip"], strategies: MERGE },
				"resourceClaimStatuses" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE_RETAIN_KEYS },
			},
			"ReplicationControllerStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ResourceQuotaStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ServiceSpec" => phf_map! {
				"ports" => FieldInfo { ssa_merge_keys: &["port", "protocol"], smp_merge_keys: &["port"], strategies: MERGE },
			},
			"ServiceStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ServiceAccount" => phf_map! {
				"secrets" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"ResourceRequirements" => phf_map! {
				"claims" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"NodeStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
				"addresses" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"NamespaceStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ComponentStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"apps" => phf_map! {
		"v1" => phf_map! {
			"StatefulSetStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"DeploymentStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"DaemonSetStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ReplicaSetStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"batch" => phf_map! {
		"v1" => phf_map! {
			"JobStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"autoscaling" => phf_map! {
		"v2" => phf_map! {
			"HorizontalPodAutoscalerStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"networking" => phf_map! {
		"v1" => phf_map! {
			"IngressLoadBalancerStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ServiceCIDRStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"policy" => phf_map! {
		"v1" => phf_map! {
			"PodDisruptionBudgetStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"admissionregistration" => phf_map! {
		"v1" => phf_map! {
			"ValidatingAdmissionPolicySpec" => phf_map! {
				"matchConditions" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
				"variables" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"ValidatingAdmissionPolicyStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"ValidatingWebhookConfiguration" => phf_map! {
				"webhooks" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"MutatingWebhookConfiguration" => phf_map! {
				"webhooks" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"ValidatingWebhook" => phf_map! {
				"matchConditions" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
			"MutatingWebhook" => phf_map! {
				"matchConditions" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
		},
	},
	"storage" => phf_map! {
		"v1" => phf_map! {
			"CSINodeSpec" => phf_map! {
				"drivers" => FieldInfo { ssa_merge_keys: &["name"], smp_merge_keys: &["name"], strategies: MERGE },
			},
		},
	},
	"flowcontrol" => phf_map! {
		"v1" => phf_map! {
			"FlowSchemaStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
			"PriorityLevelConfigurationStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"certificates" => phf_map! {
		"v1" => phf_map! {
			"CertificateSigningRequestStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
		"v1beta1" => phf_map! {
			"ClusterTrustBundleStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
	"resource" => phf_map! {
		"v1" => phf_map! {
			"ResourceClaimStatus" => phf_map! {
				"reservedFor" => FieldInfo { ssa_merge_keys: &["uid"], smp_merge_keys: &["uid"], strategies: MERGE },
			},
		},
	},
	"storagemigration" => phf_map! {
		"v1beta1" => phf_map! {
			"StorageVersionMigrationStatus" => phf_map! {
				"conditions" => FieldInfo { ssa_merge_keys: &["type"], smp_merge_keys: &["type"], strategies: MERGE },
			},
		},
	},
};

/// Check if we have builtin schema coverage for a given API group and version.
///
/// This is used to skip OpenAPI fetching for standard Kubernetes types.
pub fn has_builtin_coverage(group: &str, version: &str) -> bool {
	BUILTIN_SCHEMAS
		.get(group)
		.is_some_and(|versions| versions.get(version).is_some())
}

/// Parse an OpenAPI schema type name into (group, version, kind).
///
/// Type names follow the pattern: `io.k8s.api.{group}.{version}.{Kind}`
/// Returns None if the format doesn't match.
fn parse_schema_type(schema_type: &str) -> Option<(&str, &str, &str)> {
	let parts: Vec<&str> = schema_type.split('.').collect();
	if parts.len() >= 6 && parts[0] == "io" && parts[1] == "k8s" && parts[2] == "api" {
		// Handle groups with underscores (e.g., "networking_k8s_io" -> "networking")
		// For now we just use the first part before any underscore
		let group = parts[3].split('_').next().unwrap_or(parts[3]);
		let version = parts[4];
		let kind = parts[5];
		Some((group, version, kind))
	} else {
		None
	}
}

/// Get the merge keys for a field in a given OpenAPI schema type.
///
/// # Arguments
/// * `schema_type` - The OpenAPI schema type name (e.g., "io.k8s.api.core.v1.PodSpec")
/// * `field_name` - The JSON field name (e.g., "containers")
/// * `merge_type` - Whether to use SSA or SMP merge keys
///
/// # Returns
/// The merge keys if the field uses strategic merge by key, or `None` otherwise.
pub fn get_merge_keys(
	schema_type: &str,
	field_name: &str,
	merge_type: MergeType,
) -> Option<&'static [&'static str]> {
	let (group, version, kind) = parse_schema_type(schema_type)?;
	BUILTIN_SCHEMAS
		.get(group)?
		.get(version)?
		.get(kind)?
		.get(field_name)
		.map(|info| info.merge_keys(merge_type))
}

/// Get the patch strategies for a field in a given OpenAPI schema type.
pub fn get_patch_strategies(
	schema_type: &str,
	field_name: &str,
) -> Option<&'static [PatchStrategy]> {
	let (group, version, kind) = parse_schema_type(schema_type)?;
	BUILTIN_SCHEMAS
		.get(group)?
		.get(version)?
		.get(kind)?
		.get(field_name)
		.map(|info| info.strategies)
}

/// Get the full field schema for a field in a given OpenAPI schema type.
pub fn get_field_schema(
	schema_type: &str,
	field_name: &str,
	merge_type: MergeType,
) -> Option<FieldSchema> {
	let (group, version, kind) = parse_schema_type(schema_type)?;
	let info = BUILTIN_SCHEMAS
		.get(group)?
		.get(version)?
		.get(kind)?
		.get(field_name)?;

	Some(FieldSchema::new(PatchMeta {
		strategies: info.strategies.to_vec(),
		merge_keys: Some(
			info.merge_keys(merge_type)
				.iter()
				.map(|s| (*s).to_string())
				.collect(),
		),
	}))
}

/// Map from Kubernetes GVK to OpenAPI schema type name.
///
/// This handles the common pattern of mapping apiVersion/kind to schema names.
pub fn gvk_to_schema_type(api_version: &str, kind: &str) -> String {
	// Parse apiVersion into group and version
	let (group, version) = if let Some(slash_pos) = api_version.find('/') {
		let group = &api_version[..slash_pos];
		let version = &api_version[slash_pos + 1..];
		(group, version)
	} else {
		// Core API group (e.g., "v1")
		("core", api_version)
	};

	// Convert group to package path format
	let group_path = match group {
		"core" => "core".to_string(),
		g => g.replace(['.', '-'], "_"),
	};

	format!("io.k8s.api.{}.{}.{}", group_path, version, kind)
}

/// Schema type aliases for common embedded types.
///
/// When we encounter a field that contains an embedded type (like PodTemplateSpec
/// containing PodSpec), we need to know the schema type of the embedded content.
pub static EMBEDDED_SCHEMAS: Map<&'static str, &'static str> = phf_map! {
	// PodTemplateSpec.spec -> PodSpec
	"io.k8s.api.core.v1.PodTemplateSpec.spec" => "io.k8s.api.core.v1.PodSpec",
};

/// Get the schema type for an embedded field.
pub fn get_embedded_schema(parent_type: &str, field_name: &str) -> Option<&'static str> {
	let key = format!("{}.{}", parent_type, field_name);
	EMBEDDED_SCHEMAS.get(&key).copied()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_schema_type() {
		assert_eq!(
			parse_schema_type("io.k8s.api.core.v1.PodSpec"),
			Some(("core", "v1", "PodSpec"))
		);
		assert_eq!(
			parse_schema_type("io.k8s.api.apps.v1.Deployment"),
			Some(("apps", "v1", "Deployment"))
		);
		assert_eq!(
			parse_schema_type("io.k8s.api.networking_k8s_io.v1.Ingress"),
			Some(("networking", "v1", "Ingress"))
		);
		assert_eq!(parse_schema_type("invalid"), None);
	}

	#[test]
	fn test_has_builtin_coverage() {
		assert!(has_builtin_coverage("core", "v1"));
		assert!(has_builtin_coverage("apps", "v1"));
		assert!(has_builtin_coverage("batch", "v1"));
		assert!(!has_builtin_coverage("custom.example.com", "v1"));
		assert!(!has_builtin_coverage("core", "v2")); // Version we don't have
	}

	#[test]
	fn test_get_merge_keys_unknown_field() {
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.PodSpec",
				"unknownField",
				MergeType::StrategicMergePatch
			),
			None
		);
	}

	#[test]
	fn test_get_merge_keys_unknown_type() {
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.unknown.v1.UnknownType",
				"containers",
				MergeType::StrategicMergePatch
			),
			None
		);
	}

	#[test]
	fn test_get_merge_keys_smp_vs_ssa() {
		// ServiceSpec.ports has different keys for SMP vs SSA
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.ServiceSpec",
				"ports",
				MergeType::StrategicMergePatch
			),
			Some(&["port"][..])
		);
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.ServiceSpec",
				"ports",
				MergeType::ServerSideApply
			),
			Some(&["port", "protocol"][..])
		);

		// Container.ports has different keys for SMP vs SSA
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.Container",
				"ports",
				MergeType::StrategicMergePatch
			),
			Some(&["containerPort"][..])
		);
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.Container",
				"ports",
				MergeType::ServerSideApply
			),
			Some(&["containerPort", "protocol"][..])
		);

		// PodSpec.topologySpreadConstraints has different keys
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.PodSpec",
				"topologySpreadConstraints",
				MergeType::StrategicMergePatch
			),
			Some(&["topologyKey"][..])
		);
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.PodSpec",
				"topologySpreadConstraints",
				MergeType::ServerSideApply
			),
			Some(&["topologyKey", "whenUnsatisfiable"][..])
		);

		// PodSpec.containers has same keys for both
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.PodSpec",
				"containers",
				MergeType::StrategicMergePatch
			),
			Some(&["name"][..])
		);
		assert_eq!(
			get_merge_keys(
				"io.k8s.api.core.v1.PodSpec",
				"containers",
				MergeType::ServerSideApply
			),
			Some(&["name"][..])
		);
	}

	#[test]
	fn test_gvk_to_schema_type_core() {
		assert_eq!(gvk_to_schema_type("v1", "Pod"), "io.k8s.api.core.v1.Pod");
	}

	#[test]
	fn test_gvk_to_schema_type_apps() {
		assert_eq!(
			gvk_to_schema_type("apps/v1", "Deployment"),
			"io.k8s.api.apps.v1.Deployment"
		);
	}

	#[test]
	fn test_gvk_to_schema_type_networking() {
		assert_eq!(
			gvk_to_schema_type("networking.k8s.io/v1", "Ingress"),
			"io.k8s.api.networking_k8s_io.v1.Ingress"
		);
	}
}
