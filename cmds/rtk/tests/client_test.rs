//! Integration tests for ClusterConnection using HTTP mock server.

use assert_matches::assert_matches;
use k8s_mock::{discovery::DiscoveryMode, http::HttpMockK8sServer};
use k8s_openapi::apimachinery::pkg::version::Info;
use rtk::{
	k8s::client::{ClusterConnection, ConnectionError},
	spec::Spec,
};

async fn test_connect_with_api_server_impl(discovery_mode: DiscoveryMode) {
	let server = HttpMockK8sServer::builder()
		.discovery_mode(discovery_mode)
		.build()
		.start()
		.await;

	let spec = Spec {
		api_server: Some(server.uri()),
		..Spec::default()
	};

	let conn = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
		.await
		.expect("connection should succeed");

	assert_eq!(
		*conn.server_version(),
		Info {
			major: "1".to_string(),
			minor: "28".to_string(),
			git_version: "v1.28.0".to_string(),
			git_commit: "fake".to_string(),
			git_tree_state: "clean".to_string(),
			build_date: "2024-01-01T00:00:00Z".to_string(),
			go_version: "go1.21.0".to_string(),
			compiler: "gc".to_string(),
			platform: "linux/amd64".to_string(),
		}
	);
}

#[tokio::test]
async fn test_connect_with_api_server_aggregated() {
	test_connect_with_api_server_impl(DiscoveryMode::Aggregated).await;
}

#[tokio::test]
async fn test_connect_with_api_server_legacy() {
	test_connect_with_api_server_impl(DiscoveryMode::Legacy).await;
}

#[tokio::test]
async fn test_connect_with_context_names() {
	let server = HttpMockK8sServer::builder().build().start().await;

	let spec = Spec {
		context_names: Some(vec!["mock-context".to_string()]),
		..Spec::default()
	};

	let conn = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig())
		.await
		.expect("connection should succeed");

	assert_eq!(
		*conn.server_version(),
		Info {
			major: "1".to_string(),
			minor: "28".to_string(),
			git_version: "v1.28.0".to_string(),
			git_commit: "fake".to_string(),
			git_tree_state: "clean".to_string(),
			build_date: "2024-01-01T00:00:00Z".to_string(),
			go_version: "go1.21.0".to_string(),
			compiler: "gc".to_string(),
			platform: "linux/amd64".to_string(),
		}
	);
}

#[tokio::test]
async fn test_connect_context_not_found() {
	let server = HttpMockK8sServer::builder().build().start().await;

	let spec = Spec {
		context_names: Some(vec!["nonexistent-context".to_string()]),
		..Spec::default()
	};

	let result = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig()).await;
	assert_matches!(
		result,
		Err(ConnectionError::ContextNotFound(contexts)) if contexts == vec!["nonexistent-context"]
	);
}

#[tokio::test]
async fn test_connect_api_server_not_found() {
	let server = HttpMockK8sServer::builder().build().start().await;

	let spec = Spec {
		api_server: Some("https://not-in-kubeconfig:6443".to_string()),
		..Spec::default()
	};

	let result = ClusterConnection::from_spec_with_kubeconfig(&spec, server.kubeconfig()).await;
	assert_matches!(
		result,
		Err(ConnectionError::ClusterNotFound(server)) if server == "https://not-in-kubeconfig:6443"
	);
}
