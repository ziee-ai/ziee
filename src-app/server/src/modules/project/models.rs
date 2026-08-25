// Project models — database entities.
// API request/response types live in types.rs.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Project entity. One row per personal project.
///
/// MCP defaults previously lived inline (mcp_*); they moved to the
/// unified `mcp_settings` table (migration 78) owned by the mcp module.
/// Clients fetch them via `GET /api/projects/{id}/mcp-settings` (still
/// mounted at the same URL — the route now lives in mcp's
/// project_extension).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub user_id: Uuid,

    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,

    pub default_assistant_id: Option<Uuid>,
    pub default_model_id: Option<Uuid>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

