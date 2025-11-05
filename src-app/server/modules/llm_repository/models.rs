// LLM Repository database models - Database entities only
// Source: react-test/src-tauri/src/database/models/repository.rs
// Note: API request/response types have been moved to types.rs

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =====================================================
// Database Entities
// =====================================================

/// Authentication configuration for LLM repositories
/// This is part of the database entity and stored as JSONB in PostgreSQL
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryAuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_test_api_endpoint: Option<String>,
}

impl Default for RepositoryAuthConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            username: None,
            password: None,
            token: None,
            auth_test_api_endpoint: None,
        }
    }
}

/// LLM Repository database entity
/// Represents a row in the llm_repositories table
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmRepository {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub auth_type: String, // none, api_key, basic_auth, bearer_token
    pub auth_config: RepositoryAuthConfig,
    pub enabled: bool,
    pub built_in: bool, // true for built-in repositories like Hugging Face
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
