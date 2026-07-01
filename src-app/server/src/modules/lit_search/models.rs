//! lit_search DTOs — the normalized record, the singleton settings row, and the
//! admin settings/connector API shapes.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One normalized scholarly record. The provenance anchor for external literature
/// is the structured identifier set (DOI/PMID) — there is no span-level provenance.
/// Returned to the model in the digest text + to the UI via `structuredContent`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LitRecord {
    /// Normalized DOI (lowercased, scheme/`doi:` stripped), if known.
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub title: String,
    pub abstract_text: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub venue: Option<String>,
    /// Best landing/OA URL for the record.
    pub url: Option<String>,
    /// Canonical source key after merge (e.g. "europepmc").
    pub source: String,
    /// Per-source id audit trail, e.g. ["europepmc:PMC123", "crossref:10.x"].
    pub source_ids: Vec<String>,
    pub cited_by_count: Option<i64>,
    pub is_preprint: bool,
    /// Heuristic relevance in 0..1, filled by ranking after merge.
    pub relevance: f32,
}

/// Conservative completeness/saturation estimate — a heuristic signal, NEVER a
/// recall percentage (vendor recall claims do not survive verification).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompletenessEstimate {
    /// "low" | "moderate" | "high"
    pub estimate: String,
    pub method: String,
    pub caveat: String,
}

/// The full aggregated search payload — emitted as `structuredContent` and as the
/// REST/`tools/call` result. The model reads a compact digest of this (see the
/// handler); the UI reads the typed array.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AggregateResult {
    pub query: String,
    /// Deduped + relevance-ranked records.
    pub records: Vec<LitRecord>,
    /// Per-source pre-dedup hit counts (PRISMA "identified" + capture-recapture).
    pub identified: std::collections::BTreeMap<String, usize>,
    pub after_dedup: usize,
    /// Connectors that errored or self-skipped (e.g. CORE enabled but unkeyed).
    pub degraded_sources: Vec<String>,
    /// Present only when the deployment enables the estimate.
    pub completeness: Option<CompletenessEstimate>,
}

/// Deployment-wide lit_search settings (singleton row, id=TRUE).
// Serialize + JsonSchema only (matches the web_search peer): `query_as!` builds
// the struct itself (no `FromRow` needed) and the settings row is only ever
// serialized to clients, never deserialized from a request body.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LitSearchSettings {
    pub enabled: bool,
    pub enabled_connectors: Vec<String>,
    pub max_results: i32,
    pub per_source_limit: i32,
    pub request_timeout_secs: i32,
    pub completeness_estimate_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

/// Partial update for `PUT /api/lit-search/settings`. Every field optional →
/// absent = leave (matches the web_search peer's DTO shape).
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateLitSearchSettingsRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub enabled_connectors: Option<Vec<String>>,
    #[serde(default)]
    pub max_results: Option<i32>,
    #[serde(default)]
    pub per_source_limit: Option<i32>,
    #[serde(default)]
    pub request_timeout_secs: Option<i32>,
    #[serde(default)]
    pub completeness_estimate_enabled: Option<bool>,
}

/// One non-secret config field a connector needs — drives the generic admin UI.
///
/// These owned-`String` response DTOs are a flattened MERGE of the code-owned
/// static descriptor (connectors/mod.rs) with each connector's per-deployment
/// runtime state (`enabled` / `configured` / `api_key_set`), assembled at request
/// time in `build_catalog`. (web_search serializes its `ProviderDescriptor`
/// — a `&'static str` struct — more directly; both `&'static str` and owned
/// `String` derive `Serialize`/`JsonSchema` fine, so this is a structural choice,
/// not a derive limitation.)
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConfigFieldInfo {
    pub key: String,
    pub label: String,
    pub required: bool,
    pub placeholder: String,
    pub help: Option<String>,
    pub docs_url: Option<String>,
}

/// The optional/required API key field for a connector — drives the write-only
/// key input + "Get a key" link in the admin UI.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KeyFieldInfo {
    pub required: bool,
    pub label: String,
    pub help: Option<String>,
    pub docs_url: Option<String>,
}

/// One catalog entry returned by `GET /api/lit-search/connectors`: the code-owned
/// descriptor joined with the stored row's configured/api_key state. The key
/// value is NEVER returned — only `api_key_set`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConnectorCatalogEntry {
    pub key: String,
    pub display_name: String,
    pub keyless_note: String,
    pub key_field: Option<KeyFieldInfo>,
    pub config_fields: Vec<ConfigFieldInfo>,
    /// True when this connector is in `enabled_connectors`.
    pub enabled: bool,
    /// True when required config + (if required) key are present.
    pub configured: bool,
    /// True when an api key is stored (never the value).
    pub api_key_set: bool,
    /// The stored NON-secret config (e.g. `{ "mailto": "a@b.c" }`), so the admin
    /// form can pre-fill + round-trip it (the api key is never included here).
    /// Without this the form would re-submit empty config fields and wipe them.
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConnectorCatalogResponse {
    pub connectors: Vec<ConnectorCatalogEntry>,
}

/// Update one connector's config + key. Tri-state key: absent = leave, empty
/// string = clear, value = set.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateConnectorRequest {
    /// Non-secret config (e.g. `{ "mailto": "a@b.c" }`).
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    /// API key: omitted = leave, "" = clear, non-empty = set.
    #[serde(default)]
    pub api_key: Option<String>,
}

// ── Per-user connector keys (user-facing surface) ────────────────────────────

/// A user's stored key for one connector, in MASKED form only. The raw key is
/// never serialized — `masked_key` is `first-4 + ***`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserConnectorKeyEntry {
    pub connector: String,
    pub masked_key: String,
}

/// One row in the user-facing key catalog: a key-accepting connector joined with
/// the calling user's own key state + whether a deployment (shared) key exists.
/// Neither the user key nor the deployment key value is ever exposed.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserConnectorKeyCatalogEntry {
    pub connector: String,
    pub display_name: String,
    /// The connector's key-field descriptor (label / help / docs / required),
    /// so the user form renders the same guidance as the admin surface.
    pub key_field: Option<KeyFieldInfo>,
    /// True when the deployment/admin has a shared key for this connector — the
    /// fallback used when the user sets none. Boolean only, never the value.
    pub system_key_set: bool,
    /// The user's own key in masked form, or `null` when they've set none.
    pub user_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserConnectorKeyCatalogResponse {
    pub connectors: Vec<UserConnectorKeyCatalogEntry>,
}

/// PUT body to set the calling user's own key for a connector.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SaveUserConnectorKeyRequest {
    pub api_key: String,
}
