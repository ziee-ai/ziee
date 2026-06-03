//! User MCP defaults models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::modules::mcp::chat_extension::approval::models::{
    ApprovalMode, AutoApprovedServer, DisabledServer,
};

// ===== Loop Settings Types =====

/// Identifies a specific tool on a specific server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct ToolIdentifier {
    /// MCP server ID
    pub server_id: Uuid,
    /// Tool name
    pub tool_name: String,
}

/// Per-tool iteration limit
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PerToolLimit {
    /// MCP server ID
    pub server_id: Uuid,
    /// Tool name
    pub tool_name: String,
    /// Maximum number of times this tool can be called per conversation turn
    pub max_iteration: u32,
}

/// Loop settings for controlling the streaming iteration behavior
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LoopSettings {
    /// Stop when LLM generates a response without any tool calls (default: true)
    #[serde(default = "default_stop_when_no_tool_calling")]
    pub stop_when_no_tool_calling: bool,

    /// Maximum iterations allowed per conversation turn (0 = unlimited, default: 10)
    #[serde(default = "default_max_iteration")]
    pub max_iteration: u32,

    /// Stop when any of these specific tools are called
    #[serde(default)]
    pub stop_when_tools_called: Vec<ToolIdentifier>,

    /// Force a final text answer when limits are reached (disable tools for last iteration)
    #[serde(default)]
    pub force_final_answer: bool,

    /// Per-tool iteration limits
    #[serde(default)]
    pub per_tool_max_iteration: Vec<PerToolLimit>,
}

impl Default for LoopSettings {
    fn default() -> Self {
        Self {
            stop_when_no_tool_calling: true,
            max_iteration: 10,
            stop_when_tools_called: vec![],
            force_final_answer: false,
            per_tool_max_iteration: vec![],
        }
    }
}

fn default_stop_when_no_tool_calling() -> bool {
    true
}

fn default_max_iteration() -> u32 {
    10
}

/// User MCP defaults (database model)
#[derive(Debug, Clone, Deserialize, FromRow)]
pub struct UserMcpDefaults {
    pub id: Uuid,
    pub user_id: Uuid,

    /// Approval mode (stored as VARCHAR in DB)
    pub approval_mode: String,

    /// Auto-approved tools (JSON array stored in DB)
    pub auto_approved_tools: serde_json::Value,

    /// Disabled servers/tools (JSON array stored in DB)
    pub disabled_servers: serde_json::Value,

    /// Loop settings (JSON object stored in DB)
    pub loop_settings: Option<serde_json::Value>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserMcpDefaults {
    /// Get approval mode as enum
    pub fn get_approval_mode(&self) -> ApprovalMode {
        self.approval_mode
            .parse()
            .unwrap_or(ApprovalMode::ManualApprove)
    }

    /// Get auto-approved tools as typed Vec
    pub fn get_auto_approved_tools(&self) -> Vec<AutoApprovedServer> {
        serde_json::from_value(self.auto_approved_tools.clone()).unwrap_or_default()
    }

    /// Get disabled servers as typed Vec
    pub fn get_disabled_servers(&self) -> Vec<DisabledServer> {
        serde_json::from_value(self.disabled_servers.clone()).unwrap_or_default()
    }

    /// Get loop settings
    pub fn get_loop_settings(&self) -> LoopSettings {
        self.loop_settings
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }
}

/// User MCP defaults (API response - properly typed)
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct UserMcpDefaultsResponse {
    pub id: Uuid,
    pub user_id: Uuid,

    /// Approval mode
    pub approval_mode: ApprovalMode,

    /// Auto-approved tools grouped by server
    pub auto_approved_tools: Vec<AutoApprovedServer>,

    /// Disabled servers/tools (empty = all servers enabled)
    pub disabled_servers: Vec<DisabledServer>,

    /// Loop settings for controlling iteration behavior
    pub loop_settings: LoopSettings,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<UserMcpDefaults> for UserMcpDefaultsResponse {
    fn from(defaults: UserMcpDefaults) -> Self {
        Self {
            id: defaults.id,
            user_id: defaults.user_id,
            approval_mode: defaults.get_approval_mode(),
            auto_approved_tools: defaults.get_auto_approved_tools(),
            disabled_servers: defaults.get_disabled_servers(),
            loop_settings: defaults.get_loop_settings(),
            created_at: defaults.created_at,
            updated_at: defaults.updated_at,
        }
    }
}

/// Request to create/update user MCP defaults
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpsertUserMcpDefaultsRequest {
    /// Approval mode
    pub approval_mode: ApprovalMode,

    /// Auto-approved tools grouped by server
    /// None = preserve existing value in DB; Some(vec) = overwrite with this value
    #[serde(default)]
    pub auto_approved_tools: Option<Vec<AutoApprovedServer>>,

    /// Disabled servers/tools (empty = all servers enabled)
    #[serde(default)]
    pub disabled_servers: Vec<DisabledServer>,

    /// Loop settings for controlling iteration behavior
    #[serde(default)]
    pub loop_settings: LoopSettings,
}
