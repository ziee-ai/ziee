// API request/response types for the project↔file routes mounted at
// `/api/projects/{id}/files*`. Relocated from `modules/project/types.rs`
// as part of the project↔file inversion.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::file::models::File as FileEntity;

/// Request body for attach-by-ID (`POST /api/projects/{id}/files`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttachFileRequest {
    pub file_id: Uuid,
}

/// Response for `GET /api/projects/{id}/files` — joined with the `files`
/// table so the client gets file metadata without a per-file lookup.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectFileListResponse {
    pub files: Vec<FileEntity>,
    pub total: i64,
}
