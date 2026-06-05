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

    /// Template-only: when true, delete the existing template install
    /// for this `hub_id` before creating the new one. Used by the
    /// `/hub/updates` Re-install action to refresh an outdated
    /// template; without this the duplicate-prevention guard in
    /// `Hub.createAssistantTemplateFromHub` would 409. Ignored on the
    /// user-scoped install path (per-user installs aren't dedup'd).
    #[serde(default)]
    pub replace_existing: bool,
}

/// Request to create MCP server from hub catalog.
///
/// Used by BOTH `Hub.createMcpServerFromHub` (per-user install) and
/// `Hub.createSystemMcpServerFromHub` (system-wide install). The
/// scope is conveyed by endpoint identity, not by a request field —
/// `RequirePermissions<(...)>` gates each path at the extractor.
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

    /// System-only: when true, delete the existing system install
    /// for this `hub_id` before creating the new one. Used by the
    /// `/hub/updates` Re-install action to refresh an outdated
    /// system MCP server; without this the duplicate-prevention
    /// guard in `Hub.createSystemMcpServerFromHub` would 409.
    /// Rejected with 400 on the user-scoped install path (per-user
    /// installs aren't dedup'd). Mirrors `replace_existing` on
    /// `CreateAssistantFromHubRequest`.
    #[serde(default)]
    pub replace_existing: bool,
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

// =====================================================
// UNIFIED CATALOG TYPES (new in Phase 1)
// =====================================================

/// Per-category counts inside the unified catalog. Surfaced from
/// `GET /api/hub/version` so the UI can show "X models, Y assistants,
/// Z MCP servers" without re-reading the index.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubCatalogCounts {
    pub models: usize,
    pub assistants: usize,
    pub mcp_servers: usize,
}

/// Response for `GET /api/hub/version` — the catalog's current
/// hub_version, the server's own version (so the UI can compute
/// compat client-side), counts per category, where the active catalog
/// came from (`seed` vs `github`), and when it was installed.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubCatalogVersionResponse {
    pub hub_version: String,
    pub server_version: String,
    pub counts: HubCatalogCounts,
    /// "seed" (embedded boot fallback) or "github" (verified fetch).
    pub source: super::hub_manager::CatalogProvenance,
    /// ISO 8601 install time of the active catalog (None if unreadable).
    pub last_refreshed: Option<String>,
}

/// Response for `POST /api/hub/refresh` — what changed.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubCatalogRefreshResponse {
    pub updated: bool,
    pub previous_version: Option<String>,
    pub new_version: String,
    pub cosign_verified: bool,
}

/// Single row in `GET /api/hub/updates` — one installed hub entity
/// that's behind the current catalog (either NULL hub_version or
/// a hub_version different from the catalog).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubUpdateRow {
    pub hub_id: String,
    pub hub_category: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub installed_version: Option<String>,
    pub current_version: String,
    /// True when the install was a system-wide ASSISTANT TEMPLATE
    /// (`created_by IS NULL && entity_type = 'assistant'`). The
    /// Updates UI uses this to route the Re-install action to the
    /// template endpoint instead of creating a user assistant from
    /// a template-origin row.
    pub is_template_install: bool,

    /// True when the install was a system-wide MCP SERVER
    /// (`created_by IS NULL && entity_type = 'mcp_server'`). The
    /// Updates UI uses this to route the Re-install action through
    /// `Hub.createSystemMcpServerFromHub` (with `replace_existing:
    /// true`) instead of the user-scoped endpoint, which would
    /// silently demote an outdated system server to a personal
    /// one. Mirrors `is_template_install` for assistants.
    pub is_system_mcp_install: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubUpdatesResponse {
    pub catalog_version: String,
    pub updates: Vec<HubUpdateRow>,
}

/// Query parameters for `GET /api/hub/manifest/:id`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HubManifestQuery {
    pub category: super::models::HubCategory,
}

/// Response for `GET /api/hub/releases` — the versions published on
/// GitHub, plus which is currently installed (`active_version`) and the
/// admin's pin (`pinned_version`, None = tracking latest).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubReleasesResponse {
    pub active_version: Option<String>,
    pub pinned_version: Option<String>,
    pub releases: Vec<super::hub_manager::HubReleaseInfo>,
}

/// Request body for `POST /api/hub/activate`. `version: null` clears the
/// pin (track latest); otherwise pins + activates that exact version
/// (semver without the leading `v`).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ActivateHubVersionRequest {
    #[serde(default)]
    pub version: Option<String>,
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
