//! Memory module data types — DTOs + DB row types.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A persisted user-level memory row.
///
/// `embedding` is intentionally omitted from the public response shape —
/// embeddings are internal retrieval machinery, never user-facing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct UserMemory {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub embedding_model: Option<String>,
    pub source: String,
    pub source_message_id: Option<Uuid>,
    pub importance: i16,
    pub confidence: i16,
    pub kind: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_recalled_at: Option<DateTime<Utc>>,
    pub recall_count: i32,
}

/// Paginated response shape for the per-user memory list — matches
/// the convention used by `McpServerListResponse` /
/// `LlmRepositoryListResponse`, so the UI can drive a standard
/// antd `<Pagination>` from the response (current_page, total).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryListResponse {
    pub items: Vec<UserMemory>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Per-user memory preferences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct UserMemorySettings {
    pub user_id: Uuid,
    pub extraction_enabled: bool,
    pub retrieval_enabled: bool,
    pub max_memories: i32,
    pub retention_days: Option<i32>,
    pub extraction_model_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Append-only audit entry for memory operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct MemoryAuditEntry {
    pub id: i64,
    pub user_id: Uuid,
    pub memory_id: Option<Uuid>,
    pub op: String,
    pub source: String,
    pub content_snapshot: Option<String>,
    pub actor_kind: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Deployment-wide memory admin settings (single row, id=1).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct MemoryAdminSettings {
    pub id: i16,
    pub embedding_model_id: Option<Uuid>,
    pub embedding_dimensions: i32,
    pub default_extraction_model_id: Option<Uuid>,
    pub default_top_k: i16,
    pub cosine_threshold: f32,
    pub enabled: bool,
    /// Reaper hard-delete grace period for soft-deleted memories (days).
    pub soft_delete_grace_days: i32,
    /// Per-user/day extraction quota (rows created via extraction).
    pub daily_extraction_quota: i32,

    // Summarizer fields (`summarize_after_tokens`,
    // `summarizer_keep_recent_tokens`, `full_summary_prompt`,
    // `incremental_summary_prompt`) moved to
    // `summarization_admin_settings` in migration 91.

    // ─── FTS (migration 89) ────────────────────────────────────────────
    /// Postgres dictionary used by `websearch_to_tsquery` at retrieval
    /// time AND in the GENERATED expression on `user_memories.content_tsv`.
    /// `simple` = no stemming, language-agnostic. `english`/`spanish`/etc.
    /// = Porter stemmer for that language only. Changing this requires
    /// the explicit rebuild flow (`POST /memory/admin/fts/rebuild`); the
    /// PUT settings handler returns 409 on a dictionary change.
    pub fts_dictionary: String,
    /// FTS arm kill switch. When `false`, the retriever skips FTS even
    /// in the no-embedding-model path (then it bails entirely).
    pub fts_enabled: bool,
    /// Semantic (vector) arm kill switch. When `false`, the retriever
    /// skips the vector arm regardless of whether an embedding model is
    /// configured. Effective vector recall requires
    /// `semantic_enabled AND embedding_model_id IS NOT NULL`.
    pub semantic_enabled: bool,
    /// Reciprocal Rank Fusion constant for hybrid (vector ⊕ FTS) recall.
    /// Higher k = more egalitarian; lower = lopsided toward each arm's
    /// top-ranked. Default 60 matches the RRF paper.
    pub fts_rrf_k: i32,
    /// Hybrid retrieval pulls top-K × this many candidates from each arm
    /// before RRF fusion. Higher = more recall, more DB load.
    pub fts_candidate_multiplier: i32,
    /// `ts_rank_cd` cutoff. 0.0 = no filter (default). Increase to drop
    /// weak lexical matches at query time.
    pub fts_min_rank: f32,
    /// Set when an FTS rebuild starts; cleared when it completes (or
    /// stays NULL if no rebuild has ever run on this deployment).
    pub fts_rebuild_started_at: Option<DateTime<Utc>>,
    /// Set when the most recent FTS rebuild finished successfully.
    pub fts_rebuild_completed_at: Option<DateTime<Utc>>,

    pub updated_at: DateTime<Utc>,
}

/// Postgres dictionaries accepted by `to_tsvector` and `websearch_to_tsquery`.
/// Source of truth for the CHECK constraint on `memory_admin_settings.fts_dictionary`
/// AND the const the rebuild endpoint interpolates from when forming DDL
/// (since `tsvector` dictionaries can't be bound as a query parameter in
/// `ALTER TABLE ... GENERATED AS ...`). NEVER interpolate dictionary names
/// directly from a request body — only from this list, after `is_valid_fts_dictionary`.
pub const VALID_FTS_DICTIONARIES: &[&str] = &[
    "simple", "english", "french", "german", "spanish", "italian", "portuguese",
    "russian", "dutch", "norwegian", "swedish", "danish", "finnish", "hungarian",
    "turkish",
];

/// Whether `name` is in the FTS-dictionary allowlist. Used as defense in
/// depth before the CHECK constraint fires, and as the gate the rebuild
/// DDL interpolation passes through.
pub fn is_valid_fts_dictionary(name: &str) -> bool {
    VALID_FTS_DICTIONARIES.contains(&name)
}

/// Request body for `POST /api/memories` — manual user-driven memory add.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateMemoryRequest {
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: i16,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn default_importance() -> i16 {
    50
}

fn default_kind() -> String {
    "fact".to_string()
}

/// Request body for `PATCH /api/memories/{id}` — partial edit.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateMemoryRequest {
    pub content: Option<String>,
    pub importance: Option<i16>,
    pub kind: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Per-user settings update body.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateUserMemorySettingsRequest {
    pub extraction_enabled: Option<bool>,
    pub retrieval_enabled: Option<bool>,
    pub max_memories: Option<i32>,
    // Tri-state: absent = leave unchanged, null = clear to NULL, value = set.
    // Without deserialize_nullable_field the outer Option swallows an explicit
    // null into None, making "clear to NULL" impossible (matches the admin
    // settings struct + project/summarization).
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub retention_days: Option<Option<i32>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub extraction_model_id: Option<Option<Uuid>>,
}

/// Admin settings update body.
///
/// The `Option<Option<T>>` pattern on nullable columns means: outer
/// `None` = leave the field unchanged, `Some(None)` = clear to NULL
/// (use the compiled-in default for prompts, or "no default" for
/// embedding/extraction models), `Some(Some(x))` = set to `x`.
///
/// Serde's default `Option<T>` deserialization collapses `null` and
/// "absent" to the same `None`, so the discriminating "Some(None)"
/// state requires a custom deserializer — see
/// `deserialize_nullable_field` below.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateMemoryAdminSettingsRequest {
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub embedding_model_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub default_extraction_model_id: Option<Option<Uuid>>,
    pub default_top_k: Option<i16>,
    pub cosine_threshold: Option<f32>,
    pub enabled: Option<bool>,
    pub soft_delete_grace_days: Option<i32>,
    pub daily_extraction_quota: Option<i32>,

    // Summarizer fields moved to `summarization` module (migration 91).

    // ─── FTS (migration 89) — runtime-tunable retrieval knobs ──────────
    // Dictionary changes are validated and trigger a 409 here; the actual
    // swap goes through `POST /memory/admin/fts/rebuild` so the GENERATED
    // column on `user_memories.content_tsv` can be rewritten atomically.
    pub fts_dictionary: Option<String>,
    pub fts_enabled: Option<bool>,
    pub fts_rrf_k: Option<i32>,
    pub fts_candidate_multiplier: Option<i32>,
    pub fts_min_rank: Option<f32>,

    // ─── Semantic (vector) arm kill switch (migration 90) ────────────
    pub semantic_enabled: Option<bool>,
}

/// Request body for `POST /api/memory/admin/fts/rebuild`. Drops + re-creates
/// `user_memories.content_tsv` with the new dictionary baked into the
/// GENERATED ALWAYS expression. Long-running; the caller polls
/// `GET /api/memory/admin/fts/rebuild/status` to see when it finishes.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FtsRebuildRequest {
    pub dictionary: String,
}

/// Response body for `GET /api/memory/admin/fts/rebuild/status`. All
/// three fields are derived from `memory_admin_settings.fts_rebuild_*`.
/// `in_progress` is `started_at IS NOT NULL AND completed_at IS NULL`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FtsRebuildStatus {
    pub in_progress: bool,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Distinguish JSON `null` from absent field for `Option<Option<T>>`.
///   absent       → outer None  ("leave unchanged")
///   "field": null → Some(None) ("clear to NULL")
///   "field": v    → Some(Some(v))
/// Mirrors `chat::core::types::deserialize_nullable_field`.
fn deserialize_nullable_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

/// Max length of a single memory `content` row. Shared between
/// `memory/handlers.rs` (REST POST/PATCH) and `memory_mcp/handlers.rs`
/// (MCP remember tool) so both surfaces enforce the same cap.
/// Audit R7-#8 — was previously duplicated as a private const in both
/// handler files; now a single source of truth.
pub const MAX_MEMORY_CONTENT_LEN: usize = 4_000;

/// Allowed values for `kind`.
pub const VALID_KINDS: &[&str] = &["preference", "fact", "goal", "relationship", "other"];

pub fn is_valid_kind(s: &str) -> bool {
    VALID_KINDS.contains(&s)
}
