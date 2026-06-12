// File models

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// File entity — the **head view** of a versioned file. The per-version columns
/// (`file_size`, `mime_type`, …) reflect the current head version: `files`
/// keeps them as a denormalized mirror that `append_version`/`restore_version`
/// update in lock-step, so every existing reader of `files.*` transparently
/// sees the latest version.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct File {
    pub id: Uuid,
    pub user_id: Uuid,
    pub filename: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub checksum: Option<String>,
    pub has_thumbnail: bool,
    pub preview_page_count: i32,
    pub text_page_count: i32,
    pub processing_metadata: serde_json::Value,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Head version number (1-based).
    pub version: i32,
    /// FK to the head `file_versions` row.
    pub current_version_id: Uuid,
    /// Storage key for the head version's bytes — equals `current_version_id`
    /// for a normal head, or the restored target for a restored head. Internal
    /// resolution detail (callers load blobs by this id).
    pub blob_version_id: Uuid,
}

/// Data for creating a file (creates the parent row + version 1).
#[derive(Debug, Clone)]
pub struct FileCreateData {
    pub id: Uuid,
    pub user_id: Uuid,
    pub filename: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub checksum: Option<String>,
    pub has_thumbnail: bool,
    pub preview_page_count: i32,
    pub text_page_count: i32,
    pub processing_metadata: serde_json::Value,
    /// The chat turn / tool-call that produced v1 (provenance). `None` for
    /// plain user uploads, which have no originating message.
    pub source_message_id: Option<Uuid>,
    pub created_by: String,
}

/// One immutable version of a file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileVersion {
    pub id: Uuid,
    pub file_id: Uuid,
    pub version: i32,
    pub is_head: bool,
    /// Storage key for this version's bytes (= `id`, or the restored target).
    pub blob_version_id: Uuid,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub checksum: Option<String>,
    pub has_thumbnail: bool,
    pub preview_page_count: i32,
    pub text_page_count: i32,
    pub processing_metadata: serde_json::Value,
    /// The chat turn / tool-call that produced this version (provenance).
    pub source_message_id: Option<Uuid>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

/// Data for appending a new version. The caller pre-saves the new blob at the
/// chosen `version_id` (passed separately) before calling `append_version`.
#[derive(Debug, Clone)]
pub struct FileVersionCreateData {
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub checksum: Option<String>,
    pub has_thumbnail: bool,
    pub preview_page_count: i32,
    pub text_page_count: i32,
    pub processing_metadata: serde_json::Value,
    pub source_message_id: Option<Uuid>,
    pub created_by: String,
}

/// Processing metadata structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_text: Option<bool>,
    /// Total number of pages in the source document (PDF / DOCX /
    /// etc), not the number of preview images we rendered. The two
    /// can diverge when `PREVIEW_PAGE_CAP` truncates a long doc —
    /// the frontend uses both fields to render a "showing first N
    /// of M pages" banner. Optional because non-paged formats
    /// (images, spreadsheets, plain text) have no notion of pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
