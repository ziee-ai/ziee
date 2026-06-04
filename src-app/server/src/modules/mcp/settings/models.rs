// Unified MCP settings type — used for both conversation and project scopes.
// Mirrors the `mcp_settings` table from migration 78.
//
// JSONB columns are stored as raw `serde_json::Value` here; the typed
// parse-on-demand views (`AutoApprovedServer`, `DisabledServer`,
// `LoopSettings`, `ApprovalMode`) live in `mcp/chat_extension/approval/models.rs`
// and are shared across scopes — same parse logic regardless of where
// the row came from.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Discriminator for which scope a `McpSettings` row belongs to.
/// Maps directly to which FK column is set on the underlying row
/// (`conversation_id` XOR `project_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope_type", content = "scope_id", rename_all = "snake_case")]
pub enum McpScope {
    Conversation(Uuid),
    Project(Uuid),
}

impl McpScope {
    pub fn conversation_id(&self) -> Option<Uuid> {
        match self {
            McpScope::Conversation(id) => Some(*id),
            McpScope::Project(_) => None,
        }
    }
    pub fn project_id(&self) -> Option<Uuid> {
        match self {
            McpScope::Project(id) => Some(*id),
            McpScope::Conversation(_) => None,
        }
    }
}

/// One row from the `mcp_settings` table.
///
/// The `id` is stable across reads; the scope is normalized into the
/// `scope` enum at read time (the repository constructs it from the
/// nullable FK columns). The JSONB payload columns stay as `Value`
/// here — callers needing the typed views (`Vec<AutoApprovedServer>`,
/// `LoopSettings`, etc.) parse on demand via the existing helpers in
/// `mcp/chat_extension/approval/models.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSettings {
    pub id: Uuid,
    pub scope: McpScope,
    pub user_id: Uuid,
    pub approval_mode: String,
    pub auto_approved_tools: serde_json::Value,
    pub disabled_servers: serde_json::Value,
    pub loop_settings: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payload for `upsert` — no identity fields, no scope (caller passes
/// scope as a separate arg). All four payload fields are optional:
/// `None` means "leave existing value alone on update" (or use default
/// on insert).
#[derive(Debug, Clone, Default)]
pub struct McpSettingsUpdate {
    pub approval_mode: Option<String>,
    pub auto_approved_tools: Option<serde_json::Value>,
    pub disabled_servers: Option<serde_json::Value>,
    pub loop_settings: Option<Option<serde_json::Value>>, // tri-state: missing / null / value
}
