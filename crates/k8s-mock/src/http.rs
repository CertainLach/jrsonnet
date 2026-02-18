//! HTTP-based mock Kubernetes server using wiremock.
//!
//! This provides a real HTTP server that can be used with actual kubeconfig-based
//! connections, unlike the tower mock which only works with in-process clients.

use std::{
	collections::HashMap,
	sync::{Arc, RwLock},
};

use bon::Builder;
use k8s::strategicpatch::MergeType;
use kube::config::{
	AuthInfo, Cluster, Context, Kubeconfig, NamedAuthInfo, NamedCluster, NamedContext,
};
use tracing::{debug, trace};
use typed_headers::{
	http::{
		header::{ACCEPT, CONTENT_TYPE},
		HeaderMap, HeaderValue,
	},
	mime::Mime,
	Accept, ContentType, HeaderMapExt, QualityItem,
};
use wiremock::{
	matchers::{method, path, path_regex},
	Mock, MockServer, Request, ResponseTemplate,
};

use super::{
	discovery::{DiscoveryMode, MockDiscovery},
	helpers::{
		merge_json_with_type, strip_empty_metadata_fields, strip_strategic_merge_directives,
	},
};

/// Type alias for the shared mutable resources map.
pub type SharedResources = Arc<RwLock<HashMap<(String, String), serde_json::Value>>>;
type SharedExchanges = Arc<RwLock<Vec<HttpExchange>>>;

/// Captured mock HTTP exchange for debugging request/response differences.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HttpExchange {
	pub method: String,
	pub path: String,
	pub query: Option<String>,
	pub accept: Option<String>,
	pub content_type: Option<String>,
	pub request_body: String,
	pub response_status: u16,
	pub response_body: String,
}

/// A mock Kubernetes server exposed over HTTP.
#[derive(Builder)]
pub struct HttpMockK8sServer {
	#[builder(default)]
	discovery_mode: DiscoveryMode,
	/// Custom discovery configuration. If not set, uses default resources.
	#[builder(default)]
	discovery: MockDiscovery,
	/// Resources to serve as raw manifests. The server derives API paths from
	/// apiVersion/kind using the discovery data.
	#[builder(default)]
	resources: Vec<serde_json::Value>,
	/// OpenAPI v3 schemas to serve for strategic merge patch lookups.
	/// Keys are API paths like "api/v1" or "apis/mygroup.example.com/v1".
	/// Values are OpenAPI schema JSON documents.
	#[builder(default)]
	openapi_schemas: HashMap<String, serde_json::Value>,
}

/// A running HTTP mock server instance.
pub struct RunningHttpMockK8sServer {
	server: MockServer,
	exchanges: SharedExchanges,
}

impl HttpMockK8sServer {
	/// Start the mock server with all configured resources.
	pub async fn start(self) -> RunningHttpMockK8sServer {
		let server = MockServer::builder().start().await;
		let discovery = self.discovery;
		let exchanges = Arc::new(RwLock::new(Vec::new()));

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

		mount_version(&server, &exchanges).await;
		mount_discovery(&server, &discovery, self.discovery_mode, &exchanges).await;
		mount_resources(&server, &shared_resources, &exchanges).await;
		mount_openapi(&server, &self.openapi_schemas, &exchanges).await;

		RunningHttpMockK8sServer { server, exchanges }
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

	/// Return all captured request/response exchanges in arrival order.
	pub fn http_exchanges(&self) -> Vec<HttpExchange> {
		self.exchanges.read().unwrap().clone()
	}
}

async fn mount_version(server: &MockServer, exchanges: &SharedExchanges) {
	let exchanges = Arc::clone(exchanges);
	Mock::given(method("GET"))
		.and(path("/version"))
		.respond_with(move |req: &Request| {
			let body = serde_json::json!({
				"major": "1",
				"minor": "28",
				"gitVersion": "v1.28.0",
				"gitCommit": "fake",
				"gitTreeState": "clean",
				"buildDate": "2024-01-01T00:00:00Z",
				"goVersion": "go1.21.0",
				"compiler": "gc",
				"platform": "linux/amd64"
			});
			record_exchange(&exchanges, req, 200, &body.to_string());
			ResponseTemplate::new(200).set_body_json(body)
		})
		.mount(server)
		.await;
}

async fn mount_discovery(
	server: &MockServer,
	discovery: &MockDiscovery,
	mode: DiscoveryMode,
	exchanges: &SharedExchanges,
) {
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
	let legacy_api_body = serde_json::json!({
		"kind": "APIVersions",
		"versions": ["v1"],
		"serverAddressByClientCIDRs": []
	});

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
	let legacy_apis_body = serde_json::json!({
		"kind": "APIGroupList",
		"apiVersion": "v1",
		"groups": groups
	});

	// Mount aggregated discovery endpoints (higher priority, matched first)
	// These match when Accept header contains "apidiscovery"
	// The Content-Type must indicate aggregated discovery format for kubectl to parse it correctly
	const AGGREGATED_DISCOVERY_CONTENT_TYPE: &str =
		"application/json;g=apidiscovery.k8s.io;v=v2;as=APIGroupDiscoveryList";

	match mode {
		DiscoveryMode::Aggregated => {
			// Return aggregated discovery for clients that request it; otherwise legacy JSON.
			let core_body = serde_json::to_vec(&aggregated_core_body)
				.expect("serializing discovery JSON should never fail");
			let apis_body = serde_json::to_vec(&aggregated_apis_body)
				.expect("serializing discovery JSON should never fail");
			let api_legacy_body = legacy_api_body.clone();
			let apis_legacy_body = legacy_apis_body.clone();
			let core_exchanges = Arc::clone(exchanges);
			let apis_exchanges = Arc::clone(exchanges);

			Mock::given(method("GET"))
				.and(path("/api"))
				.respond_with(move |req: &Request| {
					if accepts_aggregated_discovery(req) {
						record_exchange(
							&core_exchanges,
							req,
							200,
							&String::from_utf8_lossy(&core_body).to_string(),
						);
						return ResponseTemplate::new(200)
							.set_body_raw(core_body.clone(), AGGREGATED_DISCOVERY_CONTENT_TYPE);
					}
					if accepts_legacy_json(req) {
						record_exchange(&core_exchanges, req, 200, &api_legacy_body.to_string());
						return ResponseTemplate::new(200).set_body_json(api_legacy_body.clone());
					}
					record_exchange(&core_exchanges, req, 406, "");
					ResponseTemplate::new(406)
				})
				.mount(server)
				.await;

			Mock::given(method("GET"))
				.and(path("/apis"))
				.respond_with(move |req: &Request| {
					if accepts_aggregated_discovery(req) {
						record_exchange(
							&apis_exchanges,
							req,
							200,
							&String::from_utf8_lossy(&apis_body).to_string(),
						);
						return ResponseTemplate::new(200)
							.set_body_raw(apis_body.clone(), AGGREGATED_DISCOVERY_CONTENT_TYPE);
					}
					if accepts_legacy_json(req) {
						record_exchange(&apis_exchanges, req, 200, &apis_legacy_body.to_string());
						return ResponseTemplate::new(200).set_body_json(apis_legacy_body.clone());
					}
					record_exchange(&apis_exchanges, req, 406, "");
					ResponseTemplate::new(406)
				})
				.mount(server)
				.await;
		}
		DiscoveryMode::Legacy => {
			// Legacy mode only returns 406 if the client cannot accept legacy JSON.
			let api_body = legacy_api_body.clone();
			let api_exchanges = Arc::clone(exchanges);
			Mock::given(method("GET"))
				.and(path("/api"))
				.respond_with(move |req: &Request| {
					if accepts_legacy_json(req) {
						record_exchange(&api_exchanges, req, 200, &api_body.to_string());
						ResponseTemplate::new(200).set_body_json(api_body.clone())
					} else {
						record_exchange(&api_exchanges, req, 406, "");
						ResponseTemplate::new(406)
					}
				})
				.mount(server)
				.await;

			let apis_body = legacy_apis_body.clone();
			let apis_exchanges = Arc::clone(exchanges);
			Mock::given(method("GET"))
				.and(path("/apis"))
				.respond_with(move |req: &Request| {
					if accepts_legacy_json(req) {
						record_exchange(&apis_exchanges, req, 200, &apis_body.to_string());
						ResponseTemplate::new(200).set_body_json(apis_body.clone())
					} else {
						record_exchange(&apis_exchanges, req, 406, "");
						ResponseTemplate::new(406)
					}
				})
				.mount(server)
				.await;
		}
	}

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

	let core_list_body = serde_json::json!({
		"kind": "APIResourceList",
		"apiVersion": "v1",
		"groupVersion": "v1",
		"resources": core_resources
	});
	let core_list_exchanges = Arc::clone(exchanges);
	Mock::given(method("GET"))
		.and(path("/api/v1"))
		.respond_with(move |req: &Request| {
			record_exchange(&core_list_exchanges, req, 200, &core_list_body.to_string());
			ResponseTemplate::new(200).set_body_json(core_list_body.clone())
		})
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

		let group_exchanges = Arc::clone(exchanges);
		let group_body = serde_json::json!({
			"kind": "APIResourceList",
			"apiVersion": "v1",
			"groupVersion": gv,
			"resources": resources
		});

		Mock::given(method("GET"))
			.and(path(format!("/apis/{}", gv)))
			.respond_with(move |req: &Request| {
				record_exchange(&group_exchanges, req, 200, &group_body.to_string());
				ResponseTemplate::new(200).set_body_json(group_body.clone())
			})
			.mount(server)
			.await;
	}
}

fn accepts_legacy_json(req: &Request) -> bool {
	let Some(parsed) = parse_accept(req) else {
		return true;
	};

	parsed.iter().any(media_range_allows_legacy_json)
}

fn accepts_aggregated_discovery(req: &Request) -> bool {
	let Some(parsed) = parse_accept(req) else {
		return false;
	};
	parsed.iter().any(media_range_requests_aggregated_discovery)
}

fn parse_accept(req: &Request) -> Option<Accept> {
	let accept = req.headers.get("accept").and_then(|v| v.to_str().ok())?;
	let accept_header = HeaderValue::from_str(accept).ok()?;
	let mut headers = HeaderMap::new();
	headers.insert(ACCEPT, accept_header);
	headers.typed_get::<Accept>().ok().flatten()
}

fn record_exchange(
	exchanges: &SharedExchanges,
	req: &Request,
	response_status: u16,
	response_body: &str,
) {
	let request_body = String::from_utf8_lossy(&req.body).to_string();
	let accept = req
		.headers
		.get("accept")
		.and_then(|value| value.to_str().ok())
		.map(str::to_string);
	let content_type = req
		.headers
		.get("content-type")
		.and_then(|value| value.to_str().ok())
		.map(str::to_string);
	let exchange = HttpExchange {
		method: req.method.as_str().to_string(),
		path: req.url.path().to_string(),
		query: req.url.query().map(str::to_string),
		accept,
		content_type,
		request_body: preview_body(&request_body),
		response_status,
		response_body: preview_body(response_body),
	};
	exchanges.write().unwrap().push(exchange);
}

fn preview_body(body: &str) -> String {
	const MAX: usize = 4096;
	let mut chars = body.chars();
	let preview: String = chars.by_ref().take(MAX).collect();
	if chars.next().is_some() {
		return format!("{}...[truncated]", preview);
	}
	preview
}

fn media_range_allows_legacy_json(item: &QualityItem<Mime>) -> bool {
	if item.quality.as_u16() == 0 {
		return false;
	}

	let media = &item.item;
	let is_any = media.type_() == typed_headers::mime::STAR;
	let is_application_wildcard = media.type_() == typed_headers::mime::APPLICATION
		&& media.subtype() == typed_headers::mime::STAR;
	let is_application_json = media.type_() == typed_headers::mime::APPLICATION
		&& media.subtype() == typed_headers::mime::JSON;
	if is_any || is_application_wildcard {
		return true;
	}
	if !is_application_json {
		return false;
	}
	true
}

fn media_range_requests_aggregated_discovery(item: &QualityItem<Mime>) -> bool {
	if item.quality.as_u16() == 0 {
		return false;
	}

	item.item.params().any(|(name, value)| {
		name.as_str().eq_ignore_ascii_case("as")
			&& value.as_str().eq_ignore_ascii_case("APIGroupDiscoveryList")
	})
}

fn merge_type_from_content_type(req: &Request) -> MergeType {
	let Some(content_type) = req
		.headers
		.get("content-type")
		.and_then(|v| v.to_str().ok())
	else {
		return MergeType::StrategicMergePatch;
	};
	let Ok(content_type_header) = HeaderValue::from_str(content_type) else {
		return MergeType::StrategicMergePatch;
	};
	let mut headers = HeaderMap::new();
	headers.insert(CONTENT_TYPE, content_type_header);
	let Ok(Some(content_type_typed)) = headers.typed_get::<ContentType>() else {
		return MergeType::StrategicMergePatch;
	};

	if content_type_typed.subtype().as_str() == "apply-patch" {
		MergeType::ServerSideApply
	} else {
		MergeType::StrategicMergePatch
	}
}

async fn mount_resources(
	server: &MockServer,
	resources: &SharedResources,
	exchanges: &SharedExchanges,
) {
	let patch_resources = Arc::clone(resources);
	let post_resources = Arc::clone(resources);
	let delete_resources = Arc::clone(resources);
	let get_resources = Arc::clone(resources);
	let patch_exchanges = Arc::clone(exchanges);
	let post_exchanges = Arc::clone(exchanges);
	let delete_exchanges = Arc::clone(exchanges);
	let get_exchanges = Arc::clone(exchanges);

	// PATCH endpoints - merge request body with existing resource
	// If dry-run is not set, persist the changes
	Mock::given(method("PATCH"))
		.and(path_regex(r"^/api(s)?/.*"))
		.respond_with(move |req: &Request| {
			let path_str = req.url.path();
			let query = req.url.query().unwrap_or("");
			let is_dry_run = query.contains("dryRun");

			let (api_path, name) = parse_resource_path(path_str);
			let name = name.unwrap_or_default();

			// Determine merge type based on Content-Type header
			// - application/strategic-merge-patch+json -> StrategicMergePatch (kubectl)
			// - application/apply-patch+yaml -> ServerSideApply
			let merge_type = merge_type_from_content_type(req);

			let patch: serde_json::Value =
				serde_json::from_slice(&req.body).unwrap_or(serde_json::Value::Null);

			let merged = {
				let resources = patch_resources.read().unwrap();
				if let Some(existing) = resources.get(&(api_path.clone(), name.clone())) {
					merge_json_with_type(existing.clone(), patch, merge_type)
				} else {
					patch
				}
			};

			let result = strip_strategic_merge_directives(merged.clone());

			// Strip empty metadata fields (annotations, labels) to match real K8s API behavior.
			// Real K8s servers don't include empty maps in responses.
			let result = strip_empty_metadata_fields(result);

			// Persist changes if not dry-run
			if !is_dry_run {
				let mut resources = patch_resources.write().unwrap();
				resources.insert((api_path, name), result.clone());
			}

			record_exchange(&patch_exchanges, req, 200, &result.to_string());
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

			record_exchange(&post_exchanges, req, 200, &body.to_string());
			ResponseTemplate::new(200).set_body_json(body)
		})
		.mount(server)
		.await;

	// DELETE endpoints - remove resource from the mock storage
	Mock::given(method("DELETE"))
		.and(path_regex(r"^/api(s)?/.*"))
		.respond_with(move |req: &Request| {
			let path_str = req.url.path();
			let (api_path, name) = parse_resource_path(path_str);
			let name = name.unwrap_or_default();

			if name.is_empty() {
				let body = serde_json::json!({
					"kind": "Status",
					"apiVersion": "v1",
					"metadata": {},
					"status": "Failure",
					"message": "name is required",
					"reason": "BadRequest",
					"code": 400
				});
				record_exchange(&delete_exchanges, req, 400, &body.to_string());
				return ResponseTemplate::new(400).set_body_json(serde_json::json!({
					"kind": "Status",
					"apiVersion": "v1",
					"metadata": {},
					"status": "Failure",
					"message": "name is required",
					"reason": "BadRequest",
					"code": 400
				}));
			}

			let removed = {
				let mut resources = delete_resources.write().unwrap();
				resources.remove(&(api_path.clone(), name.clone()))
			};

			match removed {
				Some(resource) => {
					// Return the deleted resource with a deletion timestamp
					let mut result = resource;
					if let serde_json::Value::Object(ref mut obj) = result {
						if let Some(serde_json::Value::Object(ref mut metadata)) =
							obj.get_mut("metadata")
						{
							metadata.insert(
								"deletionTimestamp".to_string(),
								serde_json::json!("2024-01-01T00:00:00Z"),
							);
						}
					}
					record_exchange(&delete_exchanges, req, 200, &result.to_string());
					ResponseTemplate::new(200).set_body_json(result)
				}
				None => {
					let body = serde_json::json!({
						"kind": "Status",
						"apiVersion": "v1",
						"metadata": {},
						"status": "Failure",
						"message": format!("{} \"{}\" not found", api_path, name),
						"reason": "NotFound",
						"code": 404
					});
					record_exchange(&delete_exchanges, req, 404, &body.to_string());
					ResponseTemplate::new(404).set_body_json(body)
				}
			}
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
			if let Some(ref name) = name {
				if let Some(resource) = resources.get(&(api_path.clone(), name.clone())) {
					record_exchange(&get_exchanges, req, 200, &resource.to_string());
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
				let body = serde_json::json!({
					"kind": "List",
					"apiVersion": "v1",
					"metadata": {"resourceVersion": "1"},
					"items": items
				});
				record_exchange(&get_exchanges, req, 200, &body.to_string());
				return ResponseTemplate::new(200).set_body_json(body);
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
				let body = serde_json::json!({
					"kind": "List",
					"apiVersion": "v1",
					"metadata": {"resourceVersion": "1"},
					"items": cluster_items
				});
				record_exchange(&get_exchanges, req, 200, &body.to_string());
				return ResponseTemplate::new(200).set_body_json(body);
			}

			// If name was provided but resource not found, return 404
			if name.is_some() {
				let body = serde_json::json!({
					"kind": "Status",
					"apiVersion": "v1",
					"metadata": {},
					"status": "Failure",
					"message": "not found",
					"reason": "NotFound",
					"code": 404
				});
				record_exchange(&get_exchanges, req, 404, &body.to_string());
				return ResponseTemplate::new(404).set_body_json(body);
			}

			// Return empty list for paths that didn't match anything
			let body = serde_json::json!({
				"kind": "List",
				"apiVersion": "v1",
				"metadata": {"resourceVersion": "1"},
				"items": []
			});
			record_exchange(&get_exchanges, req, 200, &body.to_string());
			ResponseTemplate::new(200).set_body_json(body)
		})
		.mount(server)
		.await;
}

/// Parse a Kubernetes API path into `(collection_api_path, optional_resource_name)`.
///
/// Examples:
/// - `/api/v1/namespaces/default/configmaps/my-config` -> (`/api/v1/namespaces/default/configmaps`, `Some("my-config")`)
/// - `/apis/apps/v1/namespaces/default/deployments` -> (`/apis/apps/v1/namespaces/default/deployments`, `None`)
/// - `/api/v1/pods` -> (`/api/v1/pods`, `None`)
fn parse_resource_path(path: &str) -> (String, Option<String>) {
	let segments: Vec<&str> = path.trim_end_matches('/').split('/').collect();
	if segments.len() < 3 {
		return (path.trim_end_matches('/').to_string(), None);
	}

	let (collection_len, name_index) = match segments.get(1).copied() {
		Some("api") => {
			// /api/{version}/...
			if segments.get(3) == Some(&"namespaces") && segments.len() >= 6 {
				// /api/{version}/namespaces/{ns}/{resource}[/name]
				(6usize, Some(6usize))
			} else {
				// /api/{version}/{resource}[/name]
				(4usize, Some(4usize))
			}
		}
		Some("apis") => {
			// /apis/{group}/{version}/...
			if segments.get(4) == Some(&"namespaces") && segments.len() >= 7 {
				// /apis/{group}/{version}/namespaces/{ns}/{resource}[/name]
				(7usize, Some(7usize))
			} else {
				// /apis/{group}/{version}/{resource}[/name]
				(5usize, Some(5usize))
			}
		}
		_ => return (path.trim_end_matches('/').to_string(), None),
	};

	if segments.len() < collection_len {
		return (path.trim_end_matches('/').to_string(), None);
	}

	let collection = format!("/{}", segments[1..collection_len].join("/"));
	let name = name_index
		.and_then(|index| segments.get(index))
		.map(|value| (*value).to_string());
	(collection, name)
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

/// Mount OpenAPI v3 endpoints for strategic merge patch schema lookups.
///
/// The schemas map keys are API paths like "api/v1" or "apis/mygroup.example.com/v1".
/// Values are OpenAPI schema JSON documents with x-kubernetes-list-map-keys (preferred,
/// supports composite keys) or x-kubernetes-patch-merge-key, and x-kubernetes-patch-strategy
/// extensions.
async fn mount_openapi(
	server: &MockServer,
	schemas: &HashMap<String, serde_json::Value>,
	exchanges: &SharedExchanges,
) {
	if schemas.is_empty() {
		return;
	}

	// Build the /openapi/v3 index
	let mut paths = serde_json::Map::new();
	for api_path in schemas.keys() {
		paths.insert(
			api_path.clone(),
			serde_json::json!({"serverRelativeURL": format!("/openapi/v3/{}", api_path)}),
		);
	}

	let index_exchanges = Arc::clone(exchanges);
	let index_body = serde_json::json!({
		"paths": paths
	});
	Mock::given(method("GET"))
		.and(path("/openapi/v3"))
		.respond_with(move |req: &Request| {
			record_exchange(&index_exchanges, req, 200, &index_body.to_string());
			ResponseTemplate::new(200).set_body_json(index_body.clone())
		})
		.mount(server)
		.await;

	// Mount each schema at its path
	for (api_path, schema) in schemas {
		let schema_exchanges = Arc::clone(exchanges);
		let schema_body = schema.clone();
		Mock::given(method("GET"))
			.and(path(format!("/openapi/v3/{}", api_path)))
			.respond_with(move |req: &Request| {
				record_exchange(&schema_exchanges, req, 200, &schema_body.to_string());
				ResponseTemplate::new(200).set_body_json(schema_body.clone())
			})
			.mount(server)
			.await;
	}
}

#[cfg(test)]
mod tests {
	use k8s::strategicpatch::MergeType;
	use wiremock::{
		http::{HeaderMap, HeaderValue, Method},
		Request,
	};

	use super::{
		accepts_aggregated_discovery, accepts_legacy_json, merge_type_from_content_type,
		parse_resource_path,
	};

	fn request_with_accept(accept: Option<&str>) -> Request {
		let mut headers = HeaderMap::new();
		if let Some(value) = accept {
			headers.insert("accept", HeaderValue::from_str(value).unwrap());
		}

		Request {
			url: "http://localhost/api".parse().unwrap(),
			method: Method::GET,
			headers,
			body: vec![],
		}
	}

	fn request_with_content_type(content_type: Option<&str>) -> Request {
		let mut headers = HeaderMap::new();
		if let Some(value) = content_type {
			headers.insert("content-type", HeaderValue::from_str(value).unwrap());
		}

		Request {
			url: "http://localhost/apis/apps/v1/namespaces/default/deployments/x"
				.parse()
				.unwrap(),
			method: Method::PATCH,
			headers,
			body: vec![],
		}
	}

	#[test]
	fn accepts_legacy_json_when_header_missing() {
		assert!(accepts_legacy_json(&request_with_accept(None)));
	}

	#[test]
	fn accepts_legacy_json_when_json_fallback_present() {
		let req = request_with_accept(Some(
			"application/json;g=apidiscovery.k8s.io;v=v2;as=APIGroupDiscoveryList,application/json",
		));
		assert!(accepts_legacy_json(&req));
	}

	#[test]
	fn accepts_legacy_json_when_only_aggregated_discovery_is_accepted() {
		let req = request_with_accept(Some(
			"application/json;g=apidiscovery.k8s.io;v=v2;as=APIGroupDiscoveryList",
		));
		assert!(accepts_legacy_json(&req));
	}

	#[test]
	fn detects_aggregated_discovery_request() {
		let req = request_with_accept(Some(
			"application/json;g=apidiscovery.k8s.io;v=v2;as=APIGroupDiscoveryList",
		));
		assert!(accepts_aggregated_discovery(&req));
	}

	#[test]
	fn detects_non_aggregated_request() {
		let req = request_with_accept(Some("application/json"));
		assert!(!accepts_aggregated_discovery(&req));
	}

	#[test]
	fn merge_type_detects_apply_patch() {
		let req = request_with_content_type(Some("application/apply-patch+yaml"));
		assert_eq!(
			merge_type_from_content_type(&req),
			MergeType::ServerSideApply
		);
	}

	#[test]
	fn merge_type_defaults_to_strategic_merge() {
		let req = request_with_content_type(Some("application/strategic-merge-patch+json"));
		assert_eq!(
			merge_type_from_content_type(&req),
			MergeType::StrategicMergePatch
		);
	}

	#[test]
	fn parse_resource_path_classifies_core_list_and_item() {
		assert_eq!(
			parse_resource_path("/api/v1/pods"),
			("/api/v1/pods".to_string(), None)
		);
		assert_eq!(
			parse_resource_path("/api/v1/pods/my-pod"),
			("/api/v1/pods".to_string(), Some("my-pod".to_string()))
		);
	}

	#[test]
	fn parse_resource_path_classifies_namespaced_list_and_item() {
		assert_eq!(
			parse_resource_path("/api/v1/namespaces/default/configmaps"),
			("/api/v1/namespaces/default/configmaps".to_string(), None)
		);
		assert_eq!(
			parse_resource_path("/api/v1/namespaces/default/configmaps/my-config"),
			(
				"/api/v1/namespaces/default/configmaps".to_string(),
				Some("my-config".to_string())
			)
		);
	}

	#[test]
	fn parse_resource_path_keeps_namespace_resource_cluster_scoped() {
		assert_eq!(
			parse_resource_path("/api/v1/namespaces/default"),
			(
				"/api/v1/namespaces".to_string(),
				Some("default".to_string())
			)
		);
	}
}
