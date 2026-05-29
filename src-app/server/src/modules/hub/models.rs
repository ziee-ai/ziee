// Hub models
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Hub model entry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubModel {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub repository_path: String,
    pub main_filename: String,
    pub file_format: FileFormat,
    pub capabilities: Option<ModelCapabilities>,
    pub size_gb: f64,
    #[serde(default)]
    pub tags: Vec<String>,
    pub recommended_parameters: Option<serde_json::Value>,
    pub quantization_options: Option<Vec<HubModelQuantizationOption>>,
    pub popularity_score: f64,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage_url: Option<String>,
    #[serde(default)]
    pub public: bool,
    pub context_length: Option<i32>,
    pub language_support: Option<Vec<String>>,
    pub recommended_engine: Option<String>,
    pub recommended_engine_settings: Option<serde_json::Value>,

    /// Whether authentication is required to download/use this model
    #[serde(default)]
    pub auth_required: bool,

    /// Array of model IDs downloaded by ANYONE from this hub model (system-wide)
    #[serde(default)]
    pub created_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    GGUF,
    SafeTensors,
    PyTorch,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
#[derive(Default)]
pub struct ModelCapabilities {
    pub vision: bool,
    pub audio: bool,
    pub tools: bool,
    pub code_interpreter: bool,
    pub chat: bool,
    pub text_embedding: bool,
    pub image_generator: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubModelQuantizationOption {
    pub name: String,
    pub main_filename: String,
    pub size_gb: Option<f64>,
}

/// Hub assistant entry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubAssistant {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub parameters: serde_json::Value,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub recommended_models: Option<Vec<String>>,
    pub capabilities_required: Option<Vec<String>>,
    pub use_cases: Option<Vec<String>>,
    pub example_prompts: Option<Vec<String>>,
    pub author: Option<String>,
    #[serde(default)]
    pub popularity_score: f64,

    /// Array of entity IDs created by current user from this hub assistant
    #[serde(default)]
    pub created_ids: Vec<Uuid>,
}

/// Hub MCP server entry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubMCPServer {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub transport_type: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    #[serde(alias = "env")]
    pub environment_variables: Option<serde_json::Value>,
    pub url: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub repository_url: Option<String>,
    pub documentation_url: Option<String>,
    pub icon_url: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    #[serde(default)]
    pub popularity_score: f64,
    pub download_count: Option<i32>,
    pub rating: Option<f64>,
    pub requires_desktop: Option<bool>,
    pub platform_support: Option<Vec<String>>,
    pub minimum_version: Option<String>,
    pub tool_count: Option<i32>,
    pub tool_categories: Option<Vec<String>>,
    pub example_tools: Option<Vec<String>>,
    pub use_cases: Option<Vec<String>>,
    pub example_prompts: Option<Vec<String>>,

    /// Whether this MCP server uses MCP sampling (sends sampling/createMessage requests back to the platform)
    pub supports_sampling: Option<bool>,

    /// Array of entity IDs created by current user from this hub server
    #[serde(default)]
    pub created_ids: Vec<Uuid>,
}

// =====================================================
// HUB ENTITY TRACKING
// =====================================================

/// Hub entity tracking record
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct HubEntity {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub hub_id: String,
    pub hub_category: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

/// Entity type enum for type safety
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HubEntityType {
    Assistant,
    McpServer,
    LlmModel,
}

impl HubEntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            HubEntityType::Assistant => "assistant",
            HubEntityType::McpServer => "mcp_server",
            HubEntityType::LlmModel => "llm_model",
        }
    }
}

/// Hub category enum. The JSON wire form uses kebab-case (`"mcp-server"`)
/// to match the on-disk folder names in the catalog (`mcp-servers/`) and
/// the index.json shape published by ziee-ai/hub's `release.yml`. The
/// `as_str()` helper still returns the snake-case form (`"mcp_server"`)
/// because the `hub_entities` DB column was created with that value
/// (migration 8's CHECK constraint) — kept for backward compat with
/// existing rows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum HubCategory {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "mcp-server", alias = "mcp_server")]
    McpServer,
    #[serde(rename = "model")]
    Model,
}

impl HubCategory {
    /// Snake-case form for DB rows in `hub_entities.hub_category` —
    /// matches migration 8's CHECK constraint.
    pub fn as_str(&self) -> &'static str {
        match self {
            HubCategory::Assistant => "assistant",
            HubCategory::McpServer => "mcp_server",
            HubCategory::Model => "model",
        }
    }
}

/// Combined hub data structure (for file storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubData {
    pub version: String,
    pub models: Vec<HubModel>,
    pub assistants: Vec<HubAssistant>,
    pub mcp_servers: Vec<HubMCPServer>,
}
