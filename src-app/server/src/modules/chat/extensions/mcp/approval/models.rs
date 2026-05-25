//! MCP approval workflow models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::modules::chat::extensions::mcp::defaults::models::LoopSettings;

/// Approval mode for conversation MCP settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ApprovalMode {
    /// MCP is disabled for this conversation
    Disabled,
    /// All tools are auto-approved
    AutoApprove,
    /// Manual approval required for each tool use
    #[default]
    ManualApprove,
}


impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalMode::Disabled => write!(f, "disabled"),
            ApprovalMode::AutoApprove => write!(f, "auto_approve"),
            ApprovalMode::ManualApprove => write!(f, "manual_approve"),
        }
    }
}

impl std::str::FromStr for ApprovalMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disabled" => Ok(ApprovalMode::Disabled),
            "auto_approve" => Ok(ApprovalMode::AutoApprove),
            "manual_approve" => Ok(ApprovalMode::ManualApprove),
            _ => Err(format!("Invalid approval mode: {}", s)),
        }
    }
}

/// Auto-approved tools grouped by server
/// Format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}, ...]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AutoApprovedServer {
    /// MCP server ID (UUID)
    pub server_id: Uuid,
    /// List of tool names that are auto-approved for this server
    pub tools: Vec<String>,
}

impl AutoApprovedServer {
    /// Check if a specific tool is auto-approved for this server
    pub fn contains_tool(&self, tool_name: &str) -> bool {
        self.tools.iter().any(|t| t == tool_name)
    }
}

/// Disabled servers/tools for a conversation
/// Format: [{"server_id": "uuid", "tools": []}, ...]
/// Empty tools array = entire server disabled
/// Non-empty tools array = only those specific tools disabled
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DisabledServer {
    /// MCP server ID (UUID)
    pub server_id: Uuid,
    /// List of disabled tool names (empty = entire server disabled)
    pub tools: Vec<String>,
}

#[allow(dead_code)]
impl DisabledServer {
    /// Check if entire server is disabled (empty tools = all disabled)
    pub fn is_server_disabled(&self) -> bool {
        self.tools.is_empty()
    }

    /// Check if a specific tool is disabled for this server
    pub fn is_tool_disabled(&self, tool_name: &str) -> bool {
        // If tools is empty, entire server is disabled
        if self.tools.is_empty() {
            return true;
        }
        self.tools.iter().any(|t| t == tool_name)
    }
}

/// Conversation MCP settings (database model)
#[derive(Debug, Clone, Deserialize, FromRow)]
pub struct ConversationMcpSettings {
    pub id: Uuid,
    pub conversation_id: Uuid,
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

impl ConversationMcpSettings {
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

/// Conversation MCP settings (API response - properly typed)
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ConversationMcpSettingsResponse {
    pub id: Uuid,
    pub conversation_id: Uuid,
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

impl From<ConversationMcpSettings> for ConversationMcpSettingsResponse {
    fn from(settings: ConversationMcpSettings) -> Self {
        Self {
            id: settings.id,
            conversation_id: settings.conversation_id,
            user_id: settings.user_id,
            approval_mode: settings.get_approval_mode(),
            auto_approved_tools: settings.get_auto_approved_tools(),
            disabled_servers: settings.get_disabled_servers(),
            loop_settings: settings.get_loop_settings(),
            created_at: settings.created_at,
            updated_at: settings.updated_at,
        }
    }
}

/// Tool use approval record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, schemars::JsonSchema)]
pub struct ToolUseApproval {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,

    pub tool_use_id: String,
    pub tool_name: String,

    /// Tool input (serialized as "input" for API compatibility)
    #[serde(rename = "input")]
    pub tool_input: serde_json::Value,

    /// Server identification (hybrid approach)
    pub server_id: Option<Uuid>,
    pub server_name: String,

    /// Approval status (stored as VARCHAR in DB, serialized as String for API)
    pub status: String, // Stored as VARCHAR, converted to/from ApprovalStatus

    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approval_note: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create/update MCP settings
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpsertMcpSettingsRequest {
    /// Approval mode
    pub approval_mode: ApprovalMode,

    /// Auto-approved tools grouped by server
    /// Format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}, ...]
    /// None = preserve existing value in DB; Some(vec) = overwrite with this value
    #[serde(default)]
    pub auto_approved_tools: Option<Vec<AutoApprovedServer>>,

    /// Disabled servers/tools (empty = all servers enabled)
    /// Format: [{"server_id": "uuid", "tools": []}, ...] (empty tools = entire server disabled)
    #[serde(default)]
    pub disabled_servers: Vec<DisabledServer>,

    /// Loop settings for controlling iteration behavior
    #[serde(default)]
    pub loop_settings: LoopSettings,
}

/// Single tool approval decision
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ToolApprovalDecision {
    /// Tool use ID
    pub tool_use_id: String,

    /// Decision: "approve" | "deny"
    pub decision: String,

    /// Optional note
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
