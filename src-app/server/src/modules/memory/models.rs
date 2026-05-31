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
    /// Summarizer trigger threshold (branch message count).
    pub summarize_after_n_messages: i32,
    /// Summarizer messages kept verbatim alongside the summary.
    pub summarizer_keep_recent: i32,
    /// Override for the full-resummarize LLM prompt. NULL = use the
    /// compiled-in default. Must contain `{transcript}` if set.
    pub full_summary_prompt: Option<String>,
    /// Override for the incremental-refresh LLM prompt. NULL = use the
    /// compiled-in default. Must contain `{previous_summary}` and
    /// `{new_transcript}` if set.
    pub incremental_summary_prompt: Option<String>,
    pub updated_at: DateTime<Utc>,
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
    pub retention_days: Option<Option<i32>>,
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
    pub summarize_after_n_messages: Option<i32>,
    pub summarizer_keep_recent: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub full_summary_prompt: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub incremental_summary_prompt: Option<Option<String>>,
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

/// Allowed values for `source` — guards against arbitrary writes.
pub const VALID_SOURCES: &[&str] = &["extraction", "mcp_tool", "manual"];

/// Allowed values for `kind`.
pub const VALID_KINDS: &[&str] = &["preference", "fact", "goal", "relationship", "other"];

pub fn is_valid_source(s: &str) -> bool {
    VALID_SOURCES.contains(&s)
}

pub fn is_valid_kind(s: &str) -> bool {
    VALID_KINDS.contains(&s)
}
