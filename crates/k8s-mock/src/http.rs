//! HTTP-based mock Kubernetes server using wiremock.
//!
//! This provides a real HTTP server that can be used with actual kubeconfig-based
//! connections, unlike the tower mock which only works with in-process clients.

use std::{
	collections::HashMap,
	sync::{Arc, RwLock},
};

use bon::Builder;
use kube::config::{
	AuthInfo, Cluster, Context, Kubeconfig, NamedAuthInfo, NamedCluster, NamedContext,
};
use tracing::{debug, trace};
use wiremock::{
	matchers::{header_regex, method, path, path_regex},
	Mock, MockServer, Request, ResponseTemplate,
};

use super::{
	discovery::{DiscoveryMode, MockDiscovery},
	helpers::{merge_json, strip_strategic_merge_directives},
};

/// Type alias for the shared mutable resources map.
pub type SharedResources = Arc<RwLock<HashMap<(String, String), serde_json::Value>>>;

/// A mock Kubernetes server exposed over HTTP.
#[derive(Builder)]
pub struct HttpMockK8sServer {
	#[builder(default)]
	discovery_mode: DiscoveryMode,
	/// Resources to serve as raw manifests. The server derives API paths from
	/// apiVersion/kind using the discovery data.
	#[builder(default)]
	resources: Vec<serde_json::Value>,
}

/// A running HTTP mock server instance.
pub struct RunningHttpMockK8sServer {
	server: MockServer,
	/// Shared mutable resources state.
	#[allow(dead_code)]
	resources: SharedResources,
}

impl HttpMockK8sServer {
	/// Start the mock server with all configured resources.
	pub async fn start(self) -> RunningHttpMockK8sServer {
		let server = MockServer::start().await;
		let discovery = MockDiscovery::default();

		debug!(uri = %server.uri(), "Started mock K8s server");

		// Build resources map with API paths derived from discovery
		let mut resources: HashMap<(String, String), serde_json::Value> = HashMap::new();

		for manifest in self.resources {
			if let Some((api_path, name)) = api_path_for_manifest(&manifest, &discovery) {
				trace!(api_path = %api_path, name = %name, "Registered resource");
				resources.insert((api_path, name), manifest);
			}
		}

		// Add default namespace if not present
		let ns_key = ("/api/v1/namespaces".to_string(), "default".to_string());
		resources.entry(ns_key).or_insert_with(|| {
			serde_json::json!({
				"apiVersion": "v1",
				"kind": "Namespace",
				"metadata": {
					"name": "default"
				}
			})
		});

		// Wrap in Arc<RwLock> for mutable sharing
		let shared_resources = Arc::new(RwLock::new(resources));

		mount_version(&server).await;
		mount_discovery(&server, &discovery, self.discovery_mode).await;
		mount_resources(&server, &shared_resources).await;

		RunningHttpMockK8sServer {
			server,
			resources: shared_resources,
		}
	}
}

/// Derive the API path for a manifest using discovery data.
fn api_path_for_manifest(
	manifest: &serde_json::Value,
	discovery: &MockDiscovery,
) -> Option<(String, String)> {
	let api_version = manifest.get("apiVersion")?.as_str()?;
	let kind = manifest.get("kind")?.as_str()?;
	let name = manifest.get("metadata")?.get("name")?.as_str()?.to_string();
	let namespace = manifest
		.get("metadata")
		.and_then(|m| m.get("namespace"))
		.and_then(|n| n.as_str());

	// Look up the plural resource name from discovery
	let (plural, namespaced) = if api_version.contains('/') {
		// Group resource (e.g., apps/v1)
		let resource = discovery
			.group_resources
			.get(api_version)?
			.iter()
			.find(|r| r.kind == kind)?;
		(&resource.name, resource.namespaced)
	} else {
		// Core resource (e.g., v1)
		let resource = discovery.core_resources.iter().find(|r| r.kind == kind)?;
		(&resource.name, resource.namespaced)
	};

	let path = if api_version.contains('/') {
		if namespaced {
			let ns = namespace.unwrap_or("default");
			format!("/apis/{}/namespaces/{}/{}", api_version, ns, plural)
		} else {
			format!("/apis/{}/{}", api_version, plural)
		}
	} else if namespaced {
		let ns = namespace.unwrap_or("default");
		format!("/api/{}/namespaces/{}/{}", api_version, ns, plural)
	} else {
		format!("/api/{}/{}", api_version, plural)
	};

	Some((path, name))
}

impl RunningHttpMockK8sServer {
	/// Get the server's URI (e.g., "http://127.0.0.1:12345").
	pub fn uri(&self) -> String {
		self.server.uri()
	}

	/// Create a Kubeconfig pointing to this mock server.
	pub fn kubeconfig(&self) -> Kubeconfig {
		self.kubeconfig_with_context("mock-context")
	}

	/// Create a Kubeconfig pointing to this mock server with a custom context name.
	pub fn kubeconfig_with_context(&self, context_name: &str) -> Kubeconfig {
		let cluster_name = "mock-cluster";
		let user_name = "mock-user";

		Kubeconfig {
			clusters: vec![NamedCluster {
				name: cluster_name.to_string(),
				cluster: Some(Cluster {
					server: Some(self.uri()),
					insecure_skip_tls_verify: Some(true),
					..Default::default()
				}),
			}],
			contexts: vec![NamedContext {
				name: context_name.to_string(),
				context: Some(Context {
					cluster: cluster_name.to_string(),
					user: Some(user_name.to_string()),
					namespace: Some("default".to_string()),
					..Default::default()
				}),
			}],
			auth_infos: vec![NamedAuthInfo {
				name: user_name.to_string(),
				auth_info: Some(AuthInfo::default()),
			}],
			current_context: Some(context_name.to_string()),
			..Default::default()
		}
	}
}

async fn mount_version(server: &MockServer) {
	Mock::given(method("GET"))
		.and(path("/version"))
		.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
			"major": "1",
			"minor": "28",
			"gitVersion": "v1.28.0",
			"gitCommit": "fake",
			"gitTreeState": "clean",
			"buildDate": "2024-01-01T00:00:00Z",
			"goVersion": "go1.21.0",
			"compiler": "gc",
			"platform": "linux/amd64"
		})))
		.mount(server)
		.await;
}

async fn mount_discovery(server: &MockServer, discovery: &MockDiscovery, mode: DiscoveryMode) {
	// Build aggregated discovery responses
	let core_aggregated_resources: Vec<_> = discovery
		.core_resources
		.iter()
		.map(|r| {
			serde_json::json!({
				"resource": r.name,
				"responseKind": {
					"group": "",
					"version": "v1",
					"kind": r.kind
				},
				"scope": if r.namespaced { "Namespaced" } else { "Cluster" },
				"verbs": r.verbs,
			})
		})
		.collect();

	let aggregated_core_body = serde_json::json!({
		"kind": "APIGroupDiscoveryList",
		"apiVersion": "apidiscovery.k8s.io/v2",
		"items": [{
			"metadata": {
				"name": ""
			},
			"versions": [{
				"version": "v1",
				"resources": core_aggregated_resources,
				"freshness": "Current"
			}]
		}]
	});

	let aggregated_groups: Vec<_> = discovery
		.group_resources
		.iter()
		.map(|(gv, rs)| {
			let (group, version) = gv.split_once('/').unwrap_or(("", gv));
			let resources: Vec<_> = rs
				.iter()
				.map(|r| {
					serde_json::json!({
						"resource": r.name,
						"responseKind": {
							"group": group,
							"version": version,
							"kind": r.kind
						},
						"scope": if r.namespaced { "Namespaced" } else { "Cluster" },
						"verbs": r.verbs,
					})
				})
				.collect();

			serde_json::json!({
				"metadata": {
					"name": group
				},
				"versions": [{
					"version": version,
					"resources": resources,
					"freshness": "Current"
				}]
			})
		})
		.collect();

	let aggregated_apis_body = serde_json::json!({
		"kind": "APIGroupDiscoveryList",
		"apiVersion": "apidiscovery.k8s.io/v2",
		"items": aggregated_groups
	});

	// Mount aggregated discovery endpoints (higher priority, matched first)
	// These match when Accept header contains "apidiscovery"
	// The Content-Type must indicate aggregated discovery format for kubectl to parse it correctly
	const AGGREGATED_DISCOVERY_CONTENT_TYPE: &str =
		"application/json;g=apidiscovery.k8s.io;v=v2;as=APIGroupDiscoveryList";

	match mode {
		DiscoveryMode::Aggregated => {
			// Return aggregated discovery responses with correct Content-Type
			// We must use set_body_raw since set_body_json overwrites Content-Type
			let core_body = serde_json::to_vec(&aggregated_core_body)
				.expect("serializing discovery JSON should never fail");
			let apis_body = serde_json::to_vec(&aggregated_apis_body)
				.expect("serializing discovery JSON should never fail");

			Mock::given(method("GET"))
				.and(path("/api"))
				.and(header_regex("accept", "apidiscovery"))
				.respond_with(
					ResponseTemplate::new(200)
						.set_body_raw(core_body, AGGREGATED_DISCOVERY_CONTENT_TYPE),
				)
				.mount(server)
				.await;

			Mock::given(method("GET"))
				.and(path("/apis"))
				.and(header_regex("accept", "apidiscovery"))
				.respond_with(
					ResponseTemplate::new(200)
						.set_body_raw(apis_body, AGGREGATED_DISCOVERY_CONTENT_TYPE),
				)
				.mount(server)
				.await;
		}
		DiscoveryMode::Legacy => {
			// Return 406 Not Acceptable for aggregated discovery requests
			Mock::given(method("GET"))
				.and(path("/api"))
				.and(header_regex("accept", "apidiscovery"))
				.respond_with(ResponseTemplate::new(406))
				.mount(server)
				.await;

			Mock::given(method("GET"))
				.and(path("/apis"))
				.and(header_regex("accept", "apidiscovery"))
				.respond_with(ResponseTemplate::new(406))
				.mount(server)
				.await;
		}
	}

	// Legacy discovery endpoints (fallback)
	// Core API versions
	Mock::given(method("GET"))
		.and(path("/api"))
		.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
			"kind": "APIVersions",
			"versions": ["v1"],
			"serverAddressByClientCIDRs": []
		})))
		.mount(server)
		.await;

	// API groups
	let groups: Vec<_> = discovery
		.group_resources
		.keys()
		.map(|gv| {
			let (group, version) = gv.split_once('/').unwrap_or(("", gv));
			serde_json::json!({
				"name": group,
				"versions": [{"groupVersion": gv, "version": version}],
				"preferredVersion": {"groupVersion": gv, "version": version}
			})
		})
		.collect();

	Mock::given(method("GET"))
		.and(path("/apis"))
		.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
			"kind": "APIGroupList",
			"apiVersion": "v1",
			"groups": groups
		})))
		.mount(server)
		.await;

	// Core resources (/api/v1)
	let core_resources: Vec<_> = discovery
		.core_resources
		.iter()
		.map(|r| {
			serde_json::json!({
				"name": r.name,
				"singularName": "",
				"namespaced": r.namespaced,
				"kind": r.kind,
				"verbs": r.verbs,
			})
		})
		.collect();

	Mock::given(method("GET"))
		.and(path("/api/v1"))
		.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
			"kind": "APIResourceList",
			"apiVersion": "v1",
			"groupVersion": "v1",
			"resources": core_resources
		})))
		.mount(server)
		.await;

	// Group resources (e.g., /apis/apps/v1)
	for (gv, rs) in &discovery.group_resources {
		let resources: Vec<_> = rs
			.iter()
			.map(|r| {
				serde_json::json!({
					"name": r.name,
					"singularName": "",
					"namespaced": r.namespaced,
					"kind": r.kind,
					"verbs": r.verbs,
				})
			})
			.collect();

		Mock::given(method("GET"))
			.and(path(format!("/apis/{}", gv)))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"kind": "APIResourceList",
				"apiVersion": "v1",
				"groupVersion": gv,
				"resources": resources
			})))
			.mount(server)
			.await;
	}
}

async fn mount_resources(server: &MockServer, resources: &SharedResources) {
	let patch_resources = Arc::clone(resources);
	let post_resources = Arc::clone(resources);
	let get_resources = Arc::clone(resources);

	// PATCH endpoints - merge request body with existing resource
	// If dry-run is not set, persist the changes
	Mock::given(method("PATCH"))
		.and(path_regex(r"^/api(s)?/.*"))
		.respond_with(move |req: &Request| {
			let path_str = req.url.path();
			let query = req.url.query().unwrap_or("");
			let is_dry_run = query.contains("dryRun");

			let (api_path, name) = parse_resource_path(path_str);

			let patch: serde_json::Value =
				serde_json::from_slice(&req.body).unwrap_or(serde_json::Value::Null);

			let merged = {
				let resources = patch_resources.read().unwrap();
				if let Some(existing) = resources.get(&(api_path.clone(), name.clone())) {
					merge_json(existing.clone(), patch)
				} else {
					patch
				}
			};

			let result = strip_strategic_merge_directives(merged.clone());

			// Persist changes if not dry-run
			if !is_dry_run {
				let mut resources = patch_resources.write().unwrap();
				resources.insert((api_path, name), result.clone());
			}

			ResponseTemplate::new(200).set_body_json(result)
		})
		.mount(server)
		.await;

	// POST for create - echo back the request body and optionally persist
	Mock::given(method("POST"))
		.and(path_regex(r"^/api(s)?/.*"))
		.respond_with(move |req: &Request| {
			let path_str = req.url.path();
			let query = req.url.query().unwrap_or("");
			let is_dry_run = query.contains("dryRun");

			let body: serde_json::Value =
				serde_json::from_slice(&req.body).unwrap_or(serde_json::Value::Null);

			// Extract name from the body
			let name = body
				.pointer("/metadata/name")
				.and_then(|v| v.as_str())
				.unwrap_or("")
				.to_string();

			// Persist if not dry-run
			if !is_dry_run && !name.is_empty() {
				let api_path = path_str.to_string();
				let mut resources = post_resources.write().unwrap();
				resources.insert((api_path, name), body.clone());
			}

			ResponseTemplate::new(200).set_body_json(body)
		})
		.mount(server)
		.await;

	// GET endpoints - handles both single resource and LIST
	// This is a catch-all that determines whether to return a single resource or a list
	Mock::given(method("GET"))
		.and(path_regex(r"^/api(s)?/.*"))
		.respond_with(move |req: &Request| {
			let path_str = req.url.path();
			let resources = get_resources.read().unwrap();

			// First, try to find a single resource
			let (api_path, name) = parse_resource_path(path_str);

			// Check if this looks like a single resource request (has a name component)
			if !name.is_empty() {
				if let Some(resource) = resources.get(&(api_path.clone(), name.clone())) {
					return ResponseTemplate::new(200).set_body_json(resource.clone());
				}
			}

			// Try LIST - match resources under this path
			// First try exact api_path match (namespaced list)
			let items: Vec<_> = resources
				.iter()
				.filter(|((res_api_path, _), _)| res_api_path == path_str)
				.map(|(_, v)| v.clone())
				.collect();

			if !items.is_empty() {
				return ResponseTemplate::new(200).set_body_json(serde_json::json!({
					"kind": "List",
					"apiVersion": "v1",
					"metadata": {"resourceVersion": "1"},
					"items": items
				}));
			}

			// Try cluster-wide list (e.g., /api/v1/configmaps matches /api/v1/namespaces/X/configmaps)
			let cluster_items: Vec<_> = resources
				.iter()
				.filter(|((res_api_path, _), _)| {
					if let Some(cluster_path) = extract_cluster_wide_path(res_api_path) {
						cluster_path == path_str
					} else {
						false
					}
				})
				.map(|(_, v)| v.clone())
				.collect();

			if !cluster_items.is_empty() {
				return ResponseTemplate::new(200).set_body_json(serde_json::json!({
					"kind": "List",
					"apiVersion": "v1",
					"metadata": {"resourceVersion": "1"},
					"items": cluster_items
				}));
			}

			// If name was provided but resource not found, return 404
			if !name.is_empty() {
				return ResponseTemplate::new(404).set_body_json(serde_json::json!({
					"kind": "Status",
					"apiVersion": "v1",
					"metadata": {},
					"status": "Failure",
					"message": "not found",
					"reason": "NotFound",
					"code": 404
				}));
			}

			// Return empty list for paths that didn't match anything
			ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"kind": "List",
				"apiVersion": "v1",
				"metadata": {"resourceVersion": "1"},
				"items": []
			}))
		})
		.mount(server)
		.await;
}

/// Parse a Kubernetes API path into (api_path, resource_name).
///
/// Examples:
/// - `/api/v1/namespaces/default/configmaps/my-config` -> (`/api/v1/namespaces/default/configmaps`, `my-config`)
/// - `/apis/apps/v1/namespaces/default/deployments/my-deploy` -> (`/apis/apps/v1/namespaces/default/deployments`, `my-deploy`)
/// - `/api/v1/namespaces/my-ns` -> (`/api/v1/namespaces`, `my-ns`)
fn parse_resource_path(path: &str) -> (String, String) {
	// Split path and find the last component as the resource name
	let path = path.trim_end_matches('/');
	if let Some(last_slash) = path.rfind('/') {
		let api_path = &path[..last_slash];
		let name = &path[last_slash + 1..];
		(api_path.to_string(), name.to_string())
	} else {
		(path.to_string(), String::new())
	}
}

/// Extract a cluster-wide path from a namespaced API path.
///
/// Examples:
/// - `/api/v1/namespaces/default/configmaps` -> Some(`/api/v1/configmaps`)
/// - `/apis/apps/v1/namespaces/default/deployments` -> Some(`/apis/apps/v1/deployments`)
/// - `/api/v1/namespaces` -> None (already cluster-wide for namespaces)
fn extract_cluster_wide_path(path: &str) -> Option<String> {
	// Find "/namespaces/<ns>/" pattern and extract the resource type
	if let Some(ns_idx) = path.find("/namespaces/") {
		let before_ns = &path[..ns_idx];
		let after_ns = &path[ns_idx + "/namespaces/".len()..];

		// Find the resource type after the namespace name
		if let Some(slash_idx) = after_ns.find('/') {
			let resource_type = &after_ns[slash_idx..];
			return Some(format!("{}{}", before_ns, resource_type));
		}
	}
	None
}
