// Assistant models - Database entities only
// API request/response types moved to types.rs

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
