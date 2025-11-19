// MCP extension entry point and request fields

use linkme::distributed_slice;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

/// Extension metadata - defines name and execution order
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "mcp",
    order: 30, // Execute after assistant (10) and file (20) extensions
};

// use super::mcp_extension::McpChatExtension;

/// MCP server configuration for fine-grained tool selection
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerConfig {
    /// MCP server ID
    pub server_id: Uuid,
    /// Specific tools to enable (empty = all tools from this server)
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Request fields for MCP extension
/// These fields are auto-merged into SendMessageRequest by the macro system
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    /// Enable MCP tool calling for this request
    #[serde(default)]
    pub enable_mcp: bool,

    /// MCP configuration (JSON value to be deserialized in extension)
    /// Contains: { "mcp_servers": [...] }
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_config: Option<serde_json::Value>,

    /// Tool approval decisions for resuming after approval workflow
    /// Format: [{"tool_use_id": "...", "decision": "approve", "note": "..."}]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_approvals: Option<serde_json::Value>,
}

/// Factory function to create the extension instance
/// Called by the auto-registration system
pub fn create(pool: PgPool) -> Arc<dyn ChatExtension> {
    Arc::new(super::mcp::McpChatExtension::new(pool))
}

/// Register this extension with the distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static MCP_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
