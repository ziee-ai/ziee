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

/// One document in a KB, with its derived index status plus the file metadata
/// the documents panel's `FileCard` needs (thumbnail + size/type subtitle) —
/// so the KB panel reuses the same `FileCard` row the project knowledge-files
/// panel uses, instead of a hand-rolled list row.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KnowledgeBaseDocument {
    pub file_id: Uuid,
    pub filename: String,
    pub added_at: DateTime<Utc>,
    /// One of pending|indexing|indexed|failed|no_text (from `file_index_state`;
    /// `pending` when no state row exists yet).
    pub index_status: String,
    pub chunk_count: i64,
    /// File metadata (from `files`) for the FileCard thumbnail + subtitle.
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub has_thumbnail: bool,
    pub preview_page_count: i32,
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

/// Deployment-wide retrieval capability (from `file_rag_admin_settings`), so the
/// KB detail page can show a "Retrieval: hybrid + reranker / hybrid /
/// keyword-only" line + whether an embedding / reranker model is configured.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RetrievalInfo {
    /// `hybrid_rerank` | `hybrid` | `keyword_only`.
    pub mode: String,
    pub embedding_configured: bool,
    pub rerank_enabled: bool,
}

/// Request body for the detail-page "test retrieval" search box (REST mirror of
/// the `search_knowledge` MCP tool, scoped to one KB).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct KnowledgeBaseSearchRequest {
    pub query: String,
    #[serde(default)]
    pub top_k: Option<i64>,
}

/// How much of the KB is searchable vs total (background indexing may lag).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IndexingIncomplete {
    pub searchable: i64,
    pub total: i64,
}

/// Result of a direct KB search (detail-page box) — the same hits + mode +
/// indexing signal the chat tool returns, so a user can verify retrieval.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KnowledgeBaseSearchResponse {
    pub hits: Vec<KnowledgeSearchHit>,
    pub mode: String,
    pub indexing_incomplete: IndexingIncomplete,
}

/// One place a KB is attached (a conversation or a project), for the
/// "Used in" card. `label` is the conversation title / project name.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UsageRef {
    pub id: Uuid,
    pub label: String,
}

/// Where a KB is currently attached (owner-scoped), for the "Used in" card.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct KnowledgeBaseUsage {
    pub conversations: Vec<UsageRef>,
    pub projects: Vec<UsageRef>,
}

/// Compiled DEFAULT for the per-KB document cap (DEC-14). The LIVE cap is
/// admin-configurable — `file_rag_admin_settings.kb_max_documents`, whose DB
/// DEFAULT (migration 137) mirrors this value; the attach handler reads the
/// setting, never this const. Well above the 500-doc bar.
pub const KB_MAX_DOCUMENTS_DEFAULT: i64 = 2000;

#[cfg(test)]
mod cap_tests {
    use super::KB_MAX_DOCUMENTS_DEFAULT;

    // TEST-15 (ITEM-18): the per-KB document cap default is 2000 (the live cap is
    // the admin setting kb_max_documents, whose DB default mirrors this).
    // document_count is a live COUNT(*) projection, not a stored column.
    #[test]
    fn kb_max_documents_default_is_2000() {
        assert_eq!(KB_MAX_DOCUMENTS_DEFAULT, 2000);
    }
}
