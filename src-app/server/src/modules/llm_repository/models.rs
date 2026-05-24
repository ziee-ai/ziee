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
/// This is part of the database entity and stored as JSONB in PostgreSQL.
///
/// SECURITY: api_key / password / token are write-only — accepted from
/// request bodies (Deserialize unchanged) so admins can set values, but
/// NEVER returned in responses. Without these `skip_serializing` guards
/// the credentials round-tripped on every GET /llm-repositories and
/// /llm-repositories/{id}, exposing the secret to every user with
/// `llm_repositories::read` (no per-user scope on llm_repositories).
/// Closes 09-llm-repository F-02 (High). The deeper at-rest encryption
/// (pgcrypto / SecretView) is the follow-up A5-full work.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct RepositoryAuthConfig {
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
    #[serde(default, skip_serializing)]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_test_api_endpoint: Option<String>,
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
