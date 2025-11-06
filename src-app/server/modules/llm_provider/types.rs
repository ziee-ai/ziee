// LLM Provider API types - request and response structures
// Separated from models.rs for better separation of concerns

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{LlmProvider, ProxySettings};

// =====================================================
// Request Types
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLlmProviderRequest {
    pub name: String,
    pub provider_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_settings: Option<ProxySettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateLlmProviderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_settings: Option<ProxySettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssignProviderToGroupRequest {
    pub group_id: Uuid,
}

// =====================================================
// Response Types
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProviderListResponse {
    pub providers: Vec<LlmProvider>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}
