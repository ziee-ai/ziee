//! MCP approval workflow models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Approval mode for conversation MCP settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// MCP is disabled for this conversation
    Disabled,
    /// All tools are auto-approved
    AutoApprove,
    /// Manual approval required for each tool use
    ManualApprove,
}

impl Default for ApprovalMode {
    fn default() -> Self {
        Self::ManualApprove
    }
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

/// Auto-approved tool format (supports 3 formats)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutoApprovedTool {
    /// Legacy string format: "server_name::tool_name"
    String(String),
    /// Object with server_id: {"server_id": "uuid", "tool_name": "name"}
    WithServerId {
        server_id: Uuid,
        tool_name: String,
    },
    /// Object with server_name: {"server_name": "name", "tool_name": "name"}
    WithServerName {
        server_name: String,
        tool_name: String,
    },
}

impl AutoApprovedTool {
    /// Normalize to canonical string format "server_name::tool_name"
    /// For server_id format, you must provide server_name_map to lookup the name
    pub fn to_canonical_string(&self, server_name_map: Option<&std::collections::HashMap<Uuid, String>>) -> Option<String> {
        match self {
            AutoApprovedTool::String(s) => Some(s.clone()),
            AutoApprovedTool::WithServerId { server_id, tool_name } => {
                server_name_map
                    .and_then(|map| map.get(server_id))
                    .map(|server_name| format!("{}::{}", server_name, tool_name))
            }
            AutoApprovedTool::WithServerName { server_name, tool_name } => {
                Some(format!("{}::{}", server_name, tool_name))
            }
        }
    }
}

/// Conversation MCP settings
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, schemars::JsonSchema)]
pub struct ConversationMcpSettings {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,

    /// Approval mode
    #[serde(skip)]
    pub approval_mode: String, // Stored as VARCHAR, converted to/from ApprovalMode

    /// Auto-approved tools (JSON array of tool names)
    pub auto_approved_tools: serde_json::Value,

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

    /// Get auto-approved tools as Vec<String>
    pub fn get_auto_approved_tools(&self) -> Vec<String> {
        serde_json::from_value(self.auto_approved_tools.clone()).unwrap_or_default()
    }
}

/// Tool use approval status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Cancelled,
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::Pending => write!(f, "pending"),
            ApprovalStatus::Approved => write!(f, "approved"),
            ApprovalStatus::Denied => write!(f, "denied"),
            ApprovalStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for ApprovalStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ApprovalStatus::Pending),
            "approved" => Ok(ApprovalStatus::Approved),
            "denied" => Ok(ApprovalStatus::Denied),
            "cancelled" => Ok(ApprovalStatus::Cancelled),
            _ => Err(format!("Invalid approval status: {}", s)),
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
    pub tool_input: serde_json::Value,

    /// Server identification (hybrid approach)
    pub server_id: Option<Uuid>,
    pub server_name: String,

    #[serde(skip)]
    pub status: String, // Stored as VARCHAR, converted to/from ApprovalStatus

    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approval_note: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ToolUseApproval {
    /// Get approval status as enum
    pub fn get_status(&self) -> ApprovalStatus {
        self.status.parse().unwrap_or(ApprovalStatus::Pending)
    }
}

/// Request to create MCP settings
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpsertMcpSettingsRequest {
    /// Approval mode
    pub approval_mode: ApprovalMode,

    /// Auto-approved tools (supports 3 formats):
    ///   1. String: "server_name::tool_name"
    ///   2. Object with ID: {"server_id": "uuid", "tool_name": "name"}
    ///   3. Object with name: {"server_name": "name", "tool_name": "name"}
    #[serde(default)]
    pub auto_approved_tools: serde_json::Value,
}

/// Request to approve/deny tool uses
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ApproveToolUsesRequest {
    /// List of approval decisions
    pub approvals: Vec<ToolApprovalDecision>,
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
