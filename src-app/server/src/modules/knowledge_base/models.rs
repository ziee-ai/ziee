//! DTOs for the knowledge_base module.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A user-owned knowledge base. `document_count` is derived at read (COUNT(*)),
/// never denormalized (an external file delete would drift a stored counter).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KnowledgeBase {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub document_count: i64,
    /// Rollup of per-document index status (from `file_index_state`).
    pub indexing_summary: IndexingSummary,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Per-KB rollup of document index states, so the UI can show
/// "all indexed / M indexing / K failed / P no-text" and gate grounding.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct IndexingSummary {
    pub total: i64,
    pub indexed: i64,
    pub indexing: i64,
    pub failed: i64,
    pub no_text: i64,
    pub pending: i64,
}

/// One document in a KB, with its derived index status.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KnowledgeBaseDocument {
    pub file_id: Uuid,
    pub filename: String,
    pub added_at: DateTime<Utc>,
    /// One of pending|indexing|indexed|failed|no_text (from `file_index_state`;
    /// `pending` when no state row exists yet).
    pub index_status: String,
    pub chunk_count: i64,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateKnowledgeBaseRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateKnowledgeBaseRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Attach already-uploaded files to a KB by id.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AttachDocumentsRequest {
    pub file_ids: Vec<Uuid>,
}

/// Result of an attach operation — how many were newly linked vs skipped as
/// duplicates already in the KB (checksum dedup, DEC-36).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AttachDocumentsResult {
    pub attached: i64,
    pub skipped_duplicates: i64,
}

/// A `search_knowledge` hit — the SemanticHit provenance plus the file name,
/// so the UI can render a citation chip that deep-links to the source page.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KnowledgeSearchHit {
    pub file_id: Uuid,
    pub filename: String,
    pub page_number: i32,
    pub char_start: i32,
    pub char_end: i32,
    pub score: f64,
    pub content: String,
}

/// Max documents per knowledge base (DEC-14). Well above the 500-doc bar.
pub const KB_MAX_DOCUMENTS: i64 = 2000;
