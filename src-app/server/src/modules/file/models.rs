// File models

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// File entity
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
}

/// Data for creating a file
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
