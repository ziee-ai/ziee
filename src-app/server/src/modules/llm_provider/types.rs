// LLM Provider API types - request and response structures
// Separated from models.rs for better separation of concerns

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::llm_model::models::LlmModel;

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
#[serde(deny_unknown_fields)]
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

/// Wrapped create response. For local providers, `plaintext_api_key`
/// carries the auto-minted PROXY_TOKEN — shown ONCE on create (and
/// on rotation). After this response, the value is only accessible
/// via the existing "show api_key" admin action. For non-local
/// providers `plaintext_api_key` is always None — admins typed their
/// own key in, which the server just stored.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateLlmProviderResponse {
    #[serde(flatten)]
    pub provider: LlmProvider,
    /// Plaintext PROXY_TOKEN for newly-created local providers. None
    /// for any other provider_type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plaintext_api_key: Option<String>,
}

/// Response from `POST /llm-providers/{id}/rotate-proxy-token`.
/// Carries the new plaintext token in the same shape as create.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RotateProxyTokenResponse {
    pub provider: LlmProvider,
    /// New plaintext token. Caller should copy + store; only this
    /// response carries it.
    pub plaintext_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProviderListResponse {
    pub providers: Vec<LlmProvider>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupProvidersResponse {
    pub providers: Vec<LlmProvider>,
}

// =====================================================
// Group-centric Request Types
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateGroupProvidersRequest {
    pub provider_ids: Vec<Uuid>,
}

// =====================================================
// User-facing LLM Provider Types
// =====================================================

/// Provider with its available models and user-facing key status
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProviderWithModels {
    #[serde(flatten)]
    pub provider: LlmProvider,
    pub llm_models: Vec<LlmModel>,
    /// Whether an API key is configured (either system-level or user-level)
    pub api_key_configured: bool,
}

/// Response for user-accessible LLM providers
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetUserProvidersResponse {
    pub providers: Vec<ProviderWithModels>,
}

/// Masked user API key entry
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserApiKeyEntry {
    pub provider_id: Uuid,
    pub masked_key: String,
}

/// Response listing user API keys
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserApiKeyListResponse {
    pub keys: Vec<UserApiKeyEntry>,
}

/// Request to save a user API key
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveUserApiKeyRequest {
    pub provider_id: Uuid,
    pub api_key: String,
}
