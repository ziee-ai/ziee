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

/// MCP configuration for chat requests
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpConfig {
    /// List of MCP servers with optional tool filtering
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Request fields for MCP extension
/// These fields are auto-merged into SendMessageRequest by the macro system
/// Note: Not directly constructed - used by compose_send_message_request macro
#[allow(dead_code)]
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    /// Enable MCP tool calling for this request
    #[serde(default)]
    pub enable_mcp: bool,

    /// MCP configuration specifying which servers and tools to enable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_config: Option<crate::modules::chat::extensions::mcp::McpConfig>,

    /// Tool approval decisions for resuming after approval workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_approvals: Option<Vec<crate::modules::chat::extensions::mcp::ToolApprovalDecision>>,
}

/// Factory function to create the extension instance
/// Called by the auto-registration system
pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::mcp::McpChatExtension::new(pool))
}

/// Register this extension with the distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static MCP_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};

// ============================================================================
// SSE Event Data Structs
// ============================================================================

/// Event data for MCP tool execution start
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamMcpToolStartData {
    /// Unique identifier for this tool use
    pub tool_use_id: String,
    /// Name of the tool being executed
    pub tool_name: String,
    /// MCP server executing the tool
    pub server: String,
}

/// Event data for MCP tool execution completion
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamMcpToolCompleteData {
    /// Unique identifier for this tool use
    pub tool_use_id: String,
    /// Name of the completed tool
    pub tool_name: String,
    /// MCP server that executed the tool
    pub server: String,
    /// Whether the tool execution resulted in an error
    pub is_error: bool,
}

/// Event data for MCP tool approval required
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamMcpApprovalRequiredData {
    /// Unique identifier for this tool use requiring approval
    pub tool_use_id: String,
    /// Name of the tool requiring approval
    pub tool_name: String,
    /// MCP server requesting tool execution (display name)
    pub server: String,
    /// MCP server ID (UUID)
    pub server_id: String,
    /// Tool input parameters
    pub input: serde_json::Value,
}

// ============================================================================
// SSE Event Variants
// ============================================================================

/// MCP extension's SSE event variants
/// These are merged into SSEChatStreamEvent by the compose_chat_stream_events macro
/// Note: Not directly constructed - variants merged by macro into main SSEChatStreamEvent enum
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SSEChatStreamEventVariants {
    /// Tool execution started
    McpToolStart(SSEChatStreamMcpToolStartData),
    /// Tool execution completed
    McpToolComplete(SSEChatStreamMcpToolCompleteData),
    /// Tool requires approval before execution
    McpApprovalRequired(SSEChatStreamMcpApprovalRequiredData),
}

// ============================================================================
// Content Block Delta Variants
// ============================================================================

/// MCP extension's content block delta variants
/// These are merged into ContentBlockDelta by the compose_content_block_delta_variants macro
/// Note: Not directly constructed - variants merged by macro into main ContentBlockDelta enum
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDeltaVariants {
    /// Tool use delta (incremental JSON)
    ToolUseDelta {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_delta: Option<String>,
    },
}

/// MessageContentData variants contributed by MCP extension
/// These will be auto-merged into MessageContentData by the composition macro
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum MessageContentDataVariants {
    /// Tool use request (AI wants to call a tool)
    ToolUse {
        id: String,
        /// Tool name (without server_id prefix)
        name: String,
        /// Server ID (UUID)
        server_id: String,
        input: serde_json::Value,
    },

    /// Tool result (response from tool execution)
    ToolResult {
        tool_use_id: String,
        /// Function/tool name (required for some providers like Gemini)
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}
