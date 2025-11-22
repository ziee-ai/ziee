//! Data models for LLM provider file mappings

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

/// LLM provider file mapping
///
/// Maps system files to provider-specific file IDs for caching and reuse.
/// This enables cost optimization by avoiding repeated file uploads.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LlmProviderFile {
    pub id: Uuid,
    pub file_id: Uuid,
    pub provider_id: Uuid,
    pub provider_file_id: Option<String>,
    pub provider_metadata: serde_json::Value,
    pub upload_status: UploadStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Upload status for provider file mappings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UploadStatus {
    /// Queued for upload
    Pending,
    /// Upload in progress
    Uploading,
    /// Successfully uploaded
    Completed,
    /// Upload failed
    Failed,
    /// File expired (Gemini 48h TTL)
    Expired,
}

impl std::fmt::Display for UploadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UploadStatus::Pending => write!(f, "pending"),
            UploadStatus::Uploading => write!(f, "uploading"),
            UploadStatus::Completed => write!(f, "completed"),
            UploadStatus::Failed => write!(f, "failed"),
            UploadStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for UploadStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(UploadStatus::Pending),
            "uploading" => Ok(UploadStatus::Uploading),
            "completed" => Ok(UploadStatus::Completed),
            "failed" => Ok(UploadStatus::Failed),
            "expired" => Ok(UploadStatus::Expired),
            _ => Err(format!("Invalid upload status: {}", s)),
        }
    }
}
