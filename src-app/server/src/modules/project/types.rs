// Project API request/response types.
// Separated from models.rs so the DB entity stays clean.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::Project;

/// Request to create a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateProjectRequest {
    #[serde(default)]
    #[schemars(length(min = 1, max = 255))]
    pub name: String,

    /// Brief description. Capped at 4 KiB (same as assistant) to avoid
    /// per-turn token-cost amplification when project context is
    /// injected.
    #[schemars(length(max = 4096))]
    pub description: Option<String>,

    /// System instructions injected into every conversation under this
    /// project. Capped at 64 KiB (same as assistant).
    #[schemars(length(max = 65_536))]
    pub instructions: Option<String>,

    pub default_assistant_id: Option<Uuid>,
    pub default_model_id: Option<Uuid>,
}

/// Request to update an existing project. All fields optional.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateProjectRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 255))]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 4096))]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 65_536))]
    pub instructions: Option<String>,

    /// Tri-state on FKs (missing = no change; null = clear; uuid = set).
    /// The frontend uses the existing deserialize_nullable_field helper
    /// from the chat module for symmetry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_assistant_id: Option<Option<Uuid>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model_id: Option<Option<Uuid>>,
}

/// List response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
    pub total: i64,
}

// `AttachFileRequest` + `ProjectFileListResponse` moved to
// `modules/file/project_extension/models.rs` as part of the project↔file
// inversion. The four `/api/projects/{id}/files*` routes that consume
// them are now owned by the file module.
