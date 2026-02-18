//! Mock Kubernetes API server for testing.
//!
//! Provides an HTTP server that can be used with kubeconfig-based connections.

pub mod crd;
pub mod discovery;
mod helpers;
pub mod http;

pub use crd::extract_crd_metadata;
pub use discovery::{DiscoveryMode, MockApiResource, MockDiscovery};
pub use http::{HttpExchange, HttpMockK8sServer, RunningHttpMockK8sServer};
