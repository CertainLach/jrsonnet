//! CRD metadata extraction for mock server configuration.
//!
//! Extracts CustomResourceDefinition manifests and generates the
//! corresponding discovery and OpenAPI configurations.

use std::collections::HashMap;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

use crate::{MockApiResource, MockDiscovery};

/// Extract CRD metadata from manifests to configure discovery and OpenAPI endpoints.
///
/// For each CRD found in the manifests:
/// 1. Adds the custom resource type to discovery
/// 2. Converts the CRD's openAPIV3Schema to an OpenAPI v3 document
///
/// # Arguments
/// * `manifests` - Slice of JSON manifests that may include CRDs
///
/// # Returns
/// Tuple of (MockDiscovery with CRD types, HashMap of OpenAPI schemas by API path)
pub fn extract_crd_metadata(
	manifests: &[serde_json::Value],
) -> (MockDiscovery, HashMap<String, serde_json::Value>) {
	let mut discovery = MockDiscovery::default();
	let mut openapi_schemas: HashMap<String, serde_json::Value> = HashMap::new();

	for manifest in manifests {
		// Check if this is a CRD by apiVersion/kind
		let api_version = manifest.get("apiVersion").and_then(|v| v.as_str());
		let kind = manifest.get("kind").and_then(|v| v.as_str());

		if !matches!(
			(api_version, kind),
			(
				Some("apiextensions.k8s.io/v1"),
				Some("CustomResourceDefinition")
			)
		) {
			continue;
		}

		// Parse the CRD using k8s-openapi types
		let crd: CustomResourceDefinition = match serde_json::from_value(manifest.clone()) {
			Ok(crd) => crd,
			Err(_) => continue,
		};

		let group = &crd.spec.group;
		let names = &crd.spec.names;
		let plural = &names.plural;
		let crd_kind = &names.kind;
		let scope = crd.spec.scope.as_str();

		for version_spec in &crd.spec.versions {
			let version = &version_spec.name;

			// Add to discovery
			let api_version = format!("{group}/{version}");
			let resource = if scope == "Namespaced" {
				MockApiResource::namespaced(plural, crd_kind)
			} else {
				MockApiResource::cluster_scoped(plural, crd_kind)
			};
			discovery = discovery.with_group(&api_version, vec![resource]);

			// Extract and convert openAPIV3Schema to OpenAPI document
			if let Some(schema) = version_spec
				.schema
				.as_ref()
				.and_then(|s| s.open_api_v3_schema.as_ref())
			{
				// Convert JSONSchemaProps back to JSON for the OpenAPI document
				let schema_json = match serde_json::to_value(schema) {
					Ok(v) => v,
					Err(_) => continue,
				};

				let type_name = format!("com.{}.{version}.{crd_kind}", group.replace('.', "_"));
				let openapi_doc = serde_json::json!({
					"openapi": "3.0.0",
					"info": { "title": format!("{crd_kind} API"), "version": version },
					"components": {
						"schemas": {
							&type_name: schema_json
						}
					}
				});

				let api_path = format!("apis/{group}/{version}");
				openapi_schemas.insert(api_path, openapi_doc);
			}
		}
	}

	(discovery, openapi_schemas)
}
