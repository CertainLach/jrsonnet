use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Environment represents a Tanka environment (tanka.dev/v1alpha1)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub api_version: String,
    pub kind: String,
    pub metadata: Metadata,
    pub spec: Spec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_names: Option<Vec<String>>,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inject_labels: Option<bool>,
}

impl Environment {
    /// Create a new default environment
    pub fn new() -> Self {
        Self {
            api_version: "tanka.dev/v1alpha1".to_string(),
            kind: "Environment".to_string(),
            metadata: Metadata {
                name: None,
                namespace: None,
                labels: Some(HashMap::new()),
            },
            spec: Spec {
                api_server: None,
                context_names: None,
                namespace: "default".to_string(),
                diff_strategy: None,
                apply_strategy: None,
                inject_labels: None,
            },
            data: None,
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}
