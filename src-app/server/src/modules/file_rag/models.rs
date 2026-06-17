//! Document-RAG data types — DB row types + admin DTOs.
//!
//! `file_chunks` holds per-file text chunks with a `halfvec` embedding
//! (nullable — FTS works without it) and a GENERATED `content_tsv`. Chunks
//! are keyed by `file_id` and re-indexed (delete + insert) whenever the head
//! version changes; retrieval filters by `file_id = ANY(scope)`.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Deployment-wide Document-RAG admin settings (single row, id=1).
///
/// Mirrors `memory_admin_settings`, but Document-RAG defaults **ON**
/// (`enabled = true`) — FTS works from day one; the vector arm activates
/// once `embedding_model_id` is set. `embedding_dimensions` is capped at
/// 4000 (the HNSW halfvec ceiling) and is derived by probing the chosen
/// model, never typed by the admin.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct FileRagAdminSettings {
    pub id: i16,
    pub enabled: bool,
    pub embedding_model_id: Option<Uuid>,
    pub embedding_dimensions: i32,
    // Chunking
    pub chunk_chars: i32,
    pub chunk_overlap_chars: i32,
    pub max_chunks_per_file: i32,
    // Retrieval tuning (mirror memory_admin_settings)
    pub default_top_k: i16,
    pub cosine_threshold: f32,
    pub semantic_enabled: bool,
    pub fts_enabled: bool,
    /// Postgres dictionary used by `websearch_to_tsquery` at query time.
    /// The GENERATED `content_tsv` is fixed at `'simple'`, so v1 keeps this
    /// at `'simple'` too (not exposed in the update request) to avoid an
    /// index/query stemming mismatch — see the plan's FTS gotcha.
    pub fts_dictionary: String,
    pub fts_rrf_k: i32,
    pub fts_candidate_multiplier: i32,
    pub fts_min_rank: f32,
    pub updated_at: DateTime<Utc>,
}

/// Partial update body for `PUT /api/file-rag/admin-settings`.
///
/// `embedding_model_id` uses the `Option<Option<Uuid>>` pattern: absent =
/// unchanged, `null` = clear, value = set (see `deserialize_nullable_field`).
/// `embedding_dimensions` is intentionally NOT a field — the handler derives
/// it by probe-embedding the chosen model. `fts_dictionary` is intentionally
/// NOT a field in v1 (kept at `'simple'` to match the GENERATED column).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateFileRagAdminSettingsRequest {
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub embedding_model_id: Option<Option<Uuid>>,
    pub chunk_chars: Option<i32>,
    pub chunk_overlap_chars: Option<i32>,
    pub max_chunks_per_file: Option<i32>,
    pub default_top_k: Option<i16>,
    pub cosine_threshold: Option<f32>,
    pub semantic_enabled: Option<bool>,
    pub fts_enabled: Option<bool>,
    pub fts_rrf_k: Option<i32>,
    pub fts_candidate_multiplier: Option<i32>,
    pub fts_min_rank: Option<f32>,
}

/// One chunk produced by the chunker, before persistence. `chunk_index` is a
/// file-global running index (0-based) across pages; `char_start`/`char_end`
/// are char offsets relative to the page's extracted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkDraft {
    pub page_number: i32,
    pub chunk_index: i32,
    pub char_start: i32,
    pub char_end: i32,
    pub content: String,
}

/// A file that has extracted text but no chunks yet — the backfill work-list.
#[derive(Debug, Clone)]
pub struct IndexTarget {
    pub file_id: Uuid,
    pub user_id: Uuid,
    pub blob_version_id: Uuid,
    pub version: i32,
    pub text_page_count: i32,
}

/// One retrieval hit, carrying span-level provenance for the grounding layer.
/// `score` is the RRF score (hybrid) or cosine distance complement (vector).
#[derive(Debug, Clone, Serialize)]
pub struct SemanticHit {
    pub file_id: Uuid,
    pub blob_version_id: Uuid,
    pub version: i32,
    pub page_number: i32,
    pub char_start: i32,
    pub char_end: i32,
    pub content: String,
    pub score: f64,
}

/// Which retrieval arms produced a result set — surfaced in `structuredContent`
/// so the model (and tests) can tell semantic from lexical results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrievalMode {
    Hybrid,
    Vector,
    Fts,
}

impl RetrievalMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RetrievalMode::Hybrid => "hybrid",
            RetrievalMode::Vector => "vector",
            RetrievalMode::Fts => "fts",
        }
    }
}

/// Distinguish JSON `null` from an absent field for `Option<Option<T>>`.
/// Mirrors `memory::models::deserialize_nullable_field`.
fn deserialize_nullable_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}
