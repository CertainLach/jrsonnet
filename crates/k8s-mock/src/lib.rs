//! Mock Kubernetes API server for testing.
//!
//! Provides an HTTP server that can be used with kubeconfig-based connections.

pub mod discovery;
mod helpers;
pub mod http;

pub use discovery::{DiscoveryMode, MockApiResource, MockDiscovery};
pub use http::{HttpMockK8sServer, RunningHttpMockK8sServer};
