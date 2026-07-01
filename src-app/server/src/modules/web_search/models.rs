//! Request / response DTOs for the web_search admin REST surface.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::providers::ConfigField;

/// Deployment-wide web search settings (singleton row). Returned by GET.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WebSearchSettings {
    pub enabled: bool,
    /// Ordered fallback chain of provider registry keys.
    pub provider_chain: Vec<String>,
    pub max_results: i32,
    pub fetch_max_bytes: i64,
    pub fetch_max_chars: i32,
    pub request_timeout_secs: i32,
    pub updated_at: DateTime<Utc>,
}

/// PUT body for the global settings. Every field optional → absent = leave.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateWebSearchSettingsRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub provider_chain: Option<Vec<String>>,
    #[serde(default)]
    pub max_results: Option<i32>,
    #[serde(default)]
    pub fetch_max_bytes: Option<i64>,
    #[serde(default)]
    pub fetch_max_chars: Option<i32>,
    #[serde(default)]
    pub request_timeout_secs: Option<i32>,
}

/// One entry in the provider catalog (descriptor + current configured state).
/// The API key is NEVER returned — only `api_key_set`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProviderCatalogEntry {
    pub key: String,
    pub display_name: String,
    pub needs_api_key: bool,
    pub config_fields: Vec<ConfigField>,
    /// True when required config + (if needed) API key are present.
    pub configured: bool,
    /// True when an API key is stored (the value itself is never exposed).
    pub api_key_set: bool,
    /// Non-secret stored config for this provider.
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProviderCatalogResponse {
    pub providers: Vec<ProviderCatalogEntry>,
}

/// PUT body for one provider's config/key.
/// `api_key`: absent = leave; `""` = clear; non-empty = set.
/// `config`: absent = leave; present = replace.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateProviderRequest {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
}

// ── Per-user provider keys (user-facing surface) ─────────────────────────────

/// A user's stored key for one provider, in MASKED form only. The raw key is
/// never serialized — `masked_key` is `first-4 + ***`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserProviderKeyEntry {
    pub provider: String,
    pub masked_key: String,
}

/// One row in the user-facing key catalog: a key-accepting provider joined with
/// the calling user's own key state + whether a deployment (shared) key exists.
/// Neither the user key nor the deployment key value is ever exposed.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserProviderKeyCatalogEntry {
    pub provider: String,
    pub display_name: String,
    pub needs_api_key: bool,
    /// True when the deployment/admin has a shared key for this provider — the
    /// fallback used when the user sets none. Boolean only, never the value.
    pub system_key_set: bool,
    /// The user's own key in masked form, or `null` when they've set none.
    pub user_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserProviderKeyCatalogResponse {
    pub providers: Vec<UserProviderKeyCatalogEntry>,
}

/// PUT body to set the calling user's own key for a provider.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SaveUserProviderKeyRequest {
    pub api_key: String,
}
