// Project API request/response types.
// Separated from models.rs so the DB entity stays clean.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::Project;
use crate::modules::file::models::File as ProjectFileEntity;

/// One entry in the `auto_approved_tools` / `disabled_servers` JSONB
/// arrays. Mirrors the shape used by `conversation_mcp_settings`:
/// `[{"server_id": "<uuid>", "tools": ["tool1", ...]}]`.
///
/// Strict deserialization here (instead of `serde_json::Value`) closes
/// audit B3: without the struct, a client could POST
/// `{"auto_approved_tools": "string"}` and the chat MCP extension
/// would crash trying to interpret it as an array of objects later.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerToolEntry {
    pub server_id: Uuid,
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Validation helper — clamps the per-list and per-tool sizes so the
/// JSONB payload can't grow unbounded.
pub const MCP_MAX_ENTRIES_PER_LIST: usize = 256;
pub const MCP_MAX_TOOLS_PER_ENTRY: usize = 256;
pub const MCP_MAX_TOOL_NAME_LEN: usize = 256;

pub fn validate_mcp_entries(entries: &[McpServerToolEntry], field: &str) -> Result<(), String> {
    if entries.len() > MCP_MAX_ENTRIES_PER_LIST {
        return Err(format!(
            "{} has {} entries; max is {}",
            field,
            entries.len(),
            MCP_MAX_ENTRIES_PER_LIST
        ));
    }
    for (i, e) in entries.iter().enumerate() {
        if e.tools.len() > MCP_MAX_TOOLS_PER_ENTRY {
            return Err(format!(
                "{}[{}].tools has {} entries; max is {}",
                field,
                i,
                e.tools.len(),
                MCP_MAX_TOOLS_PER_ENTRY
            ));
        }
        for (ti, t) in e.tools.iter().enumerate() {
            if t.len() > MCP_MAX_TOOL_NAME_LEN {
                return Err(format!(
                    "{}[{}].tools[{}] exceeds {} bytes",
                    field, i, ti, MCP_MAX_TOOL_NAME_LEN
                ));
            }
        }
    }
    Ok(())
}

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

    /// MCP defaults. If omitted, the project gets the standard defaults
    /// from migration 51 (manual_approve, empty allowlist, empty
    /// blocklist). Conversations created in the project receive a
    /// snapshot of these settings at creation time.
    pub mcp_approval_mode: Option<String>,
    pub mcp_auto_approved_tools: Option<Vec<McpServerToolEntry>>,
    pub mcp_disabled_servers: Option<Vec<McpServerToolEntry>>,
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

/// MCP-only update endpoint payload (sibling to UpdateProjectRequest so
/// the MCP panel can PUT without sending the rest of the project fields).
///
/// `loop_settings` is `Option<Value>` (not a strict struct) for symmetry
/// with the conversation_mcp_settings schema (migration 19) — keeping
/// the project shape JSONB-flexible avoids a separate Rust struct that
/// would need to stay in lockstep with the chat-side type. The chat
/// extension consumes the snapshot opaquely.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateProjectMcpSettingsRequest {
    pub approval_mode: String,
    pub auto_approved_tools: Vec<McpServerToolEntry>,
    pub disabled_servers: Vec<McpServerToolEntry>,
    #[serde(default)]
    pub loop_settings: Option<serde_json::Value>,
}

/// Accepted values for `approval_mode`. Matches the strings the chat
/// MCP extension already recognizes (see
/// `chat/extensions/mcp/Mcp.store.ts`).
pub const MCP_APPROVAL_MODES: &[&str] = &["disabled", "auto_approve", "manual_approve"];

pub fn validate_approval_mode(mode: &str) -> Result<(), String> {
    if MCP_APPROVAL_MODES.contains(&mode) {
        Ok(())
    } else {
        Err(format!(
            "approval_mode must be one of {:?} (got {:?})",
            MCP_APPROVAL_MODES, mode
        ))
    }
}

/// Request body for attach-by-ID.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttachFileRequest {
    pub file_id: Uuid,
}

/// List response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
    pub total: i64,
}

/// File-list response (joined with the `files` table for client
/// convenience — saves a per-file lookup roundtrip).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectFileListResponse {
    pub files: Vec<ProjectFileEntity>,
    pub total: i64,
}
