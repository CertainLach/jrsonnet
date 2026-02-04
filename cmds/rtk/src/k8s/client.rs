//! Kubernetes cluster connection management.

use std::time::Duration;

use k8s_openapi::apimachinery::pkg::version::Info;
use kube::{
	config::{KubeConfigOptions, Kubeconfig, KubeconfigError},
	Client, Config,
};
use thiserror::Error;
use tracing::instrument;

use crate::spec::Spec;

/// Default timeout for Kubernetes API requests.
const DEFAULT_API_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors that can occur when connecting to a Kubernetes cluster.
#[derive(Debug, Error)]
pub enum ConnectionError {
	#[error(
		"your Environment's spec.json seems incomplete:\n  \
		 * spec.apiServer|spec.contextNames: No Kubernetes cluster endpoint or context names specified. \
		 Please specify only one.\n\n\
		 Please see https://tanka.dev/config for reference"
	)]
	IncompleteSpec,

	#[error("contextNames is empty")]
	EmptyContextNames,

	#[error(
		"no cluster that matches the apiServer `{0}` was found. Please check your $KUBECONFIG"
	)]
	ClusterNotFound(String),

	#[error("no context using cluster `{0}` was found. Please check your $KUBECONFIG")]
	ContextNotFoundForCluster(String),

	#[error("no context named `{0:?}` was found. Please check your $KUBECONFIG")]
	ContextNotFound(Vec<String>),

	#[error(transparent)]
	Kubeconfig(#[from] KubeconfigError),

	#[error(transparent)]
	Kube(#[from] kube::Error),
}

/// Represents a connection to a Kubernetes cluster.
///
/// This type encapsulates the kube client and server metadata,
/// providing a high-level interface for cluster operations.
#[derive(Clone)]
pub struct ClusterConnection {
	client: Client,
	server_version: Info,
	/// Human-readable identifier for the cluster (context name or API server URL).
	cluster_identifier: String,
}

impl std::fmt::Debug for ClusterConnection {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClusterConnection")
			.field("cluster_identifier", &self.cluster_identifier)
			.field("server_version", &self.server_version)
			.finish_non_exhaustive()
	}
}

impl ClusterConnection {
	/// Connect to a cluster using the environment spec.
	///
	/// Exactly one of `spec.apiServer` or `spec.contextNames` must be set:
	/// - `spec.apiServer`: searches kubeconfig for a cluster with matching server URL,
	///   then finds and uses a context that references that cluster
	/// - `spec.contextNames`: uses the first matching context name from kubeconfig
	#[instrument(skip_all)]
	pub async fn from_spec(spec: &Spec) -> Result<Self, ConnectionError> {
		let kubeconfig = Kubeconfig::read()?;
		Self::from_spec_with_kubeconfig(spec, kubeconfig).await
	}

	/// Connect to a cluster using the environment spec and a provided kubeconfig.
	#[instrument(skip_all)]
	pub async fn from_spec_with_kubeconfig(
		spec: &Spec,
		kubeconfig: Kubeconfig,
	) -> Result<Self, ConnectionError> {
		let (mut config, cluster_identifier) = if let Some(api_server) = &spec.api_server {
			// Search kubeconfig for a cluster whose server matches api_server,
			// then find a context that uses that cluster
			let context_name = find_context_for_api_server(&kubeconfig, api_server)?;

			tracing::debug!(
				context = %context_name,
				api_server = %api_server,
				"found context for apiServer"
			);

			let config = Config::from_custom_kubeconfig(
				kubeconfig,
				&KubeConfigOptions {
					context: Some(context_name.clone()),
					..Default::default()
				},
			)
			.await?;

			(
				config,
				format!("{}  (context:{})", api_server, context_name),
			)
		} else if let Some(context_names) = &spec.context_names {
			// Use the first matching context name from kubeconfig
			if context_names.is_empty() {
				return Err(ConnectionError::EmptyContextNames);
			}

			let context_name = find_first_matching_context(&kubeconfig, context_names)?;

			tracing::debug!(context = %context_name, "using context from contextNames");

			let config = Config::from_custom_kubeconfig(
				kubeconfig,
				&KubeConfigOptions {
					context: Some(context_name.clone()),
					..Default::default()
				},
			)
			.await?;

			(config, format!("context:{}", context_name))
		} else {
			// Neither apiServer nor contextNames specified
			return Err(ConnectionError::IncompleteSpec);
		};

		config.read_timeout = Some(DEFAULT_API_TIMEOUT);
		let client = Client::try_from(config)?;

		// Fetch server version for strategy selection
		let server_version = client.apiserver_version().await?;

		Ok(Self {
			client,
			server_version,
			cluster_identifier,
		})
	}

	/// Get a reference to the underlying kube client.
	pub fn client(&self) -> &Client {
		&self.client
	}

	/// Get the server version.
	pub fn server_version(&self) -> &Info {
		&self.server_version
	}

	/// Get the default namespace from the current context.
	pub fn default_namespace(&self) -> &str {
		self.client.default_namespace()
	}

	/// Get the cluster identifier (context name or API server URL).
	pub fn cluster_identifier(&self) -> &str {
		&self.cluster_identifier
	}
}

/// Find a kubeconfig context that uses a cluster with the given API server URL.
///
/// Searches for a cluster whose `server` field matches the apiServer,
/// then finds a context that references that cluster.
fn find_context_for_api_server(
	kubeconfig: &Kubeconfig,
	api_server: &str,
) -> Result<String, ConnectionError> {
	// Find a cluster whose server matches the api_server
	let matching_cluster = kubeconfig
		.clusters
		.iter()
		.find(|c| {
			c.cluster
				.as_ref()
				.is_some_and(|cluster| cluster.server.as_deref() == Some(api_server))
		})
		.ok_or_else(|| ConnectionError::ClusterNotFound(api_server.to_string()))?;

	let cluster_name = &matching_cluster.name;

	// Find a context that uses this cluster
	let matching_context = kubeconfig
		.contexts
		.iter()
		.find(|c| {
			c.context
				.as_ref()
				.is_some_and(|ctx| ctx.cluster.as_str() == cluster_name)
		})
		.ok_or_else(|| ConnectionError::ContextNotFoundForCluster(cluster_name.clone()))?;

	Ok(matching_context.name.clone())
}

/// Find the first context from the list that exists in kubeconfig.
fn find_first_matching_context(
	kubeconfig: &Kubeconfig,
	context_names: &[String],
) -> Result<String, ConnectionError> {
	for name in context_names {
		if kubeconfig.contexts.iter().any(|c| &c.name == name) {
			return Ok(name.clone());
		}
	}

	Err(ConnectionError::ContextNotFound(context_names.to_vec()))
}

#[cfg(test)]
mod tests {
	use assert_matches::assert_matches;

	use super::*;

	#[tokio::test]
	async fn test_connect_no_cluster_specified_errors() {
		let spec = Spec::default();
		let kubeconfig = Kubeconfig::default();

		let result = ClusterConnection::from_spec_with_kubeconfig(&spec, kubeconfig).await;
		assert_matches!(result, Err(ConnectionError::IncompleteSpec));
	}

	#[tokio::test]
	async fn test_connect_empty_context_names_errors() {
		let spec = Spec {
			context_names: Some(vec![]),
			..Spec::default()
		};
		let kubeconfig = Kubeconfig::default();

		let result = ClusterConnection::from_spec_with_kubeconfig(&spec, kubeconfig).await;
		assert_matches!(result, Err(ConnectionError::EmptyContextNames));
	}

	#[tokio::test]
	async fn test_connect_context_not_found() {
		let spec = Spec {
			context_names: Some(vec!["nonexistent".to_string()]),
			..Spec::default()
		};
		let kubeconfig = Kubeconfig::default();

		let result = ClusterConnection::from_spec_with_kubeconfig(&spec, kubeconfig).await;
		assert_matches!(
			result,
			Err(ConnectionError::ContextNotFound(contexts)) if contexts == vec!["nonexistent"]
		);
	}

	#[tokio::test]
	async fn test_connect_api_server_not_found() {
		let spec = Spec {
			api_server: Some("https://unknown:6443".to_string()),
			..Spec::default()
		};
		let kubeconfig = Kubeconfig::default();

		let result = ClusterConnection::from_spec_with_kubeconfig(&spec, kubeconfig).await;
		assert_matches!(
			result,
			Err(ConnectionError::ClusterNotFound(server)) if server == "https://unknown:6443"
		);
	}
}
