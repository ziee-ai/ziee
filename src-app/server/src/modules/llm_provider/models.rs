// LLM Provider models - Database entities only
// Source: react-test/src-tauri/src/database/models/provider.rs
// API request/response types have been moved to types.rs

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Proxy settings for LLM providers
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    /// Proxy password — write-only.
    ///
    /// Accepted from request bodies (Deserialize) so admins can set the
    /// value, but NEVER serialized into a response. Without this guard
    /// the field round-tripped verbatim on every GET /llm-providers/{id}
    /// and /llm-providers, exposing the proxy credential. Closes
    /// 06-llm-provider F-05 (Medium).
    #[serde(default, skip_serializing)]
    pub password: String,
    #[serde(default)]
    pub no_proxy: String,
    #[serde(default)]
    pub ignore_ssl_certificates: bool,
}

/// Database entity representing an LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProvider {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String, // local, openai, anthropic, groq, gemini, mistral, deepseek, huggingface, custom
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub built_in: bool,
    pub proxy_settings: ProxySettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Default runtime version for local models in this provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_runtime_version_id: Option<Uuid>,
}
