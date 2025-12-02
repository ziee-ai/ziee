//! User MCP defaults models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::modules::chat::extensions::mcp::approval::models::{
    ApprovalMode, AutoApprovedServer, DisabledServer,
};

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
    #[serde(default)]
    pub auto_approved_tools: Vec<AutoApprovedServer>,

    /// Disabled servers/tools (empty = all servers enabled)
    #[serde(default)]
    pub disabled_servers: Vec<DisabledServer>,
}
