// Assistant API types - request/response structures
// Separated from models.rs for cleaner architecture

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::models::{Assistant, ModelParameters};

/// Request structure for creating a new assistant
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateAssistantRequest {
    /// Unique name for the assistant (within user scope for user assistants, globally for templates)
    #[serde(default)]
    #[schemars(length(min = 1, max = 255))]
    pub name: String,

    /// Brief description of the assistant purpose. Closes
    /// 10-assistant F-02 (Medium) — unbounded description was an
    /// LLM-token-cost amplification vector.
    #[schemars(length(max = 4096))]
    pub description: Option<String>,

    /// System instructions for the AI assistant. Bounded at 64 KiB
    /// per 10-assistant F-02 (Medium); legitimate prompts fit
    /// comfortably below 8 KiB.
    #[schemars(length(max = 65_536))]
    pub instructions: Option<String>,

    /// Model parameters (temperature, max_tokens, etc.)
    pub parameters: Option<ModelParameters>,

    /// Whether this is a system-wide template
    /// - true: Requires assistant_templates::create permission, created_by will be NULL
    /// - false/omitted: Requires assistants::create permission, created_by will be set to current user
    /// This field is IMMUTABLE after creation
    pub is_template: Option<bool>,

    /// Whether this is the default assistant
    /// Setting to true will unset other defaults in the same context
    pub is_default: Option<bool>,

    /// Whether this assistant is enabled
    /// Defaults to true if not specified
    pub enabled: Option<bool>,
}

/// Request structure for updating an existing assistant
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateAssistantRequest {
    /// Update assistant name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 255))]
    pub name: Option<String>,

    /// Update description (max 4 KiB per 10-assistant F-02)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 4096))]
    pub description: Option<String>,

    /// Update instructions (max 64 KiB per 10-assistant F-02)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 65_536))]
    pub instructions: Option<String>,

    /// Update model parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ModelParameters>,

    /// Update default status
    /// Setting to true will unset other defaults in the same context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,

    /// Update enabled status (soft delete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    // Note: is_template is NOT included here - it's immutable after creation
}

/// Response structure for listing assistants
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssistantListResponse {
    pub assistants: Vec<Assistant>,
    pub total: i64,
}

impl CreateAssistantRequest {
    /// Convert parameters to JSON value for database storage
    pub fn parameters_to_json(&self) -> serde_json::Value {
        match &self.parameters {
            Some(params) => serde_json::to_value(params).unwrap_or_else(|_| serde_json::json!({})),
            None => serde_json::json!({}),
        }
    }
}

impl UpdateAssistantRequest {
    /// Convert parameters to JSON value for database storage
    pub fn parameters_to_json(&self) -> Option<serde_json::Value> {
        self.parameters
            .as_ref()
            .map(|params| serde_json::to_value(params).unwrap_or_else(|_| serde_json::json!({})))
    }

    /// Check if this update request has any fields set
    pub fn has_updates(&self) -> bool {
        self.name.is_some()
            || self.description.is_some()
            || self.instructions.is_some()
            || self.parameters.is_some()
            || self.is_default.is_some()
            || self.enabled.is_some()
    }
}
