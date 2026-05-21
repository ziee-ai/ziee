// Hub types
#![allow(dead_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{HubAssistant, HubEntity, HubMCPServer, HubModel};

/// Query parameters for hub endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HubQuery {
    /// Locale code (e.g., "en", "es", "fr")
    #[serde(default = "default_locale")]
    pub lang: String,
}

fn default_locale() -> String {
    "en".to_string()
}

/// Version response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubVersionResponse {
    pub version: String,
    pub last_updated: Option<String>,
}

/// Refresh response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubRefreshResponse {
    pub updated: bool,
    pub version: String,
}

/// Response types (for OpenAPI)
pub type HubModelsResponse = Vec<HubModel>;
pub type HubAssistantsResponse = Vec<HubAssistant>;
pub type HubMCPServersResponse = Vec<HubMCPServer>;

// =====================================================
// HUB CREATION REQUESTS
// =====================================================

/// Request to create assistant from hub catalog
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateAssistantFromHubRequest {
    /// Hub assistant ID
    pub hub_id: String,

    /// Optional: Override name (defaults to hub assistant name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional: Override description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional: Override instructions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Optional: Override parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,

    /// Whether this should be the default assistant
    #[serde(default)]
    pub is_default: bool,

    /// Whether this assistant is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Request to create MCP server from hub catalog
/// Note: Hub interface always creates user MCP servers, not system servers
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMcpServerFromHubRequest {
    /// Hub MCP server ID
    pub hub_id: String,

    /// Optional: Override name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional: Override display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Optional: Override enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Request to create LLM model from hub catalog (triggers download)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateModelFromHubRequest {
    /// Hub model ID
    pub hub_id: String,

    /// Provider ID to associate model with
    pub provider_id: Uuid,

    /// Optional: Override display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Optional: Select quantization option (defaults to main file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization_name: Option<String>,

    /// Whether this model is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

// =====================================================
// HUB CREATION RESPONSES
// =====================================================

/// Response for assistant created from hub
#[derive(Debug, Serialize, JsonSchema)]
pub struct AssistantFromHubResponse {
    /// Created assistant
    pub assistant: crate::modules::assistant::models::Assistant,

    /// Hub tracking record
    pub hub_tracking: HubEntity,
}

/// Response for MCP server created from hub
#[derive(Debug, Serialize, JsonSchema)]
pub struct McpServerFromHubResponse {
    /// Created MCP server
    pub server: crate::modules::mcp::McpServer,

    /// Hub tracking record
    pub hub_tracking: HubEntity,
}

/// Response for model download initiated from hub
#[derive(Debug, Serialize, JsonSchema)]
pub struct ModelFromHubResponse {
    /// Created download instance
    pub download: crate::modules::llm_model::models::DownloadInstance,

    /// Hub tracking record
    pub hub_tracking: HubEntity,
}

/// A local LLM provider available as download target
#[derive(Debug, Serialize, JsonSchema)]
pub struct HubLocalProvider {
    pub id: Uuid,
    pub name: String,
}

/// Response listing local providers available for hub model downloads
#[derive(Debug, Serialize, JsonSchema)]
pub struct HubLocalProvidersResponse {
    pub providers: Vec<HubLocalProvider>,
}

fn default_true() -> bool {
    true
}
