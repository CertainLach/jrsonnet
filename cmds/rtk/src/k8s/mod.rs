//! Kubernetes client module for diff and apply operations.
//!
//! This module provides native Kubernetes API access using kube-rs,
//! avoiding the need to shell out to kubectl.

pub mod apply;
pub mod client;
pub mod diff;
pub mod discovery;
pub mod output;

/// Kubernetes API resource scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceScope {
	/// Resource is namespaced (e.g., Deployment, ConfigMap).
	Namespaced,

	/// Resource is cluster-wide (e.g., Namespace, ClusterRole).
	ClusterWide,
}
