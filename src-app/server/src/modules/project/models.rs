// Project models — database entities.
// API request/response types live in types.rs.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Project entity. One row per personal project.
///
/// MCP defaults are inline (mcp_*) — see migration 51 for the rationale
/// (no sibling project_mcp_settings table; 1:1 sync hazard).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub user_id: Uuid,

    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,

    pub default_assistant_id: Option<Uuid>,
    pub default_model_id: Option<Uuid>,

    pub mcp_approval_mode: String,
    /// JSONB: `[{"server_id": "uuid", "tools": ["tool1", ...]}, ...]`
    #[serde(default)]
    pub mcp_auto_approved_tools: serde_json::Value,
    /// JSONB: `[{"server_id": "uuid", "tools": []}, ...]`
    #[serde(default)]
    pub mcp_disabled_servers: serde_json::Value,
    /// JSONB; nullable. Same shape + NULL convention as
    /// `conversation_mcp_settings.loop_settings` (migration 19).
    /// NULL means "not configured — application supplies the default":
    ///   { stop_when_no_tool_calling: true, max_iteration: 10,
    ///     stop_when_tools_called: [], force_final_answer: false,
    ///     per_tool_max_iteration: [] }
    /// Snapshotted into conversation_mcp_settings.loop_settings at
    /// conversation create time.
    pub mcp_loop_settings: Option<serde_json::Value>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// project_files membership row. The `file` field itself is fetched via
/// a JOIN in the file-list endpoint; this struct is mainly internal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, FromRow)]
pub struct ProjectFile {
    pub project_id: Uuid,
    pub file_id: Uuid,
    pub added_at: DateTime<Utc>,
}
