// Assistant models - copied from ziee-chat-ref and adapted for ziee-chat
// Source: ziee-chat-ref/src-tauri/src/database/models/assistant.rs

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export ModelParameters from llm_model module
pub use crate::modules::llm_model::models::ModelParameters;

/// Assistant entity
/// Defines AI behavior with instructions, parameters, and settings
/// Supports both user-created assistants and system-wide templates
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct Assistant {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    /// Model parameters stored as JSONB
    /// Can be deserialized to ModelParameters when needed
    #[serde(default)]
    pub parameters: serde_json::Value,
    /// User who created this assistant (NULL for system templates)
    pub created_by: Option<Uuid>,
    /// Whether this is a system-wide template (immutable after creation)
    pub is_template: bool,
    /// Whether this is the default assistant for the user/template context
    pub is_default: bool,
    /// Whether this assistant is enabled (false means disabled/soft-deleted)
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request structure for creating a new assistant
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateAssistantRequest {
    /// Unique name for the assistant (within user scope for user assistants, globally for templates)
    #[serde(default)]
    #[schemars(length(min = 1, max = 255))]
    pub name: String,

    /// Brief description of the assistant purpose
    pub description: Option<String>,

    /// System instructions for the AI assistant
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

    /// Update description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Update instructions
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl Assistant {
    /// Parse parameters from JSONB to ModelParameters
    pub fn get_parameters(&self) -> Result<Option<ModelParameters>, serde_json::Error> {
        if self.parameters.is_null() || self.parameters == serde_json::json!({}) {
            Ok(None)
        } else {
            Ok(Some(serde_json::from_value(self.parameters.clone())?))
        }
    }
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
        self.parameters.as_ref().map(|params| {
            serde_json::to_value(params).unwrap_or_else(|_| serde_json::json!({}))
        })
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
