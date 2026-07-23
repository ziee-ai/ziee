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
    pub mcp_config: Option<crate::modules::mcp::chat_extension::McpConfig>,

    /// Tool approval decisions for resuming after approval workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_approvals: Option<Vec<crate::modules::mcp::chat_extension::ToolApprovalDecision>>,

    /// UNATTENDED run (ITEM-13/DEC-17): set by the scheduler for a headless
    /// firing (no user to approve). When true, a tool that would require
    /// per-call approval AND is not in `unattended_allowed_tools` is DENIED
    /// (synthesized denial tool_result) instead of creating an orphaned pending
    /// approval + truncating the turn. Never set by an interactive client.
    #[serde(default)]
    pub unattended: bool,

    /// Per-task allow-list for an unattended run: servers/tools the task's
    /// creator pre-authorized to run without the approval speed-bump.
    /// Fully-qualified path so the `compose_chat_extensions` macro resolves the
    /// type at its (external) expansion site — mirrors how `mcp_config` above
    /// uses a full `crate::…` path.
    #[serde(default)]
    pub unattended_allowed_tools:
        Vec<crate::modules::mcp::chat_extension::extension::UnattendedToolGrant>,
}

/// One pre-authorized (server, tool?) grant for an unattended run. `tool_name`
/// `None` allow-lists the whole server. Structurally identical to the
/// scheduler's `AllowedTool` (deserialized from the same JSON), kept here so
/// mcp has no scheduler dependency.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub struct UnattendedToolGrant {
    pub server_id: uuid::Uuid,
    #[serde(default)]
    pub tool_name: Option<String>,
}


/// Factory function to create the extension instance
/// Called by the auto-registration system
pub fn create(pool: PgPool, config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::mcp::McpChatExtension::new(pool, config))
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
    /// Tool input parameters (for display in "Show details" panel)
    pub input: serde_json::Value,
}

/// Event data for an artifact file created by a tool (via MCP resource_link)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamArtifactCreatedData {
    /// Tool use id this artifact was produced by. Lets the frontend attach the
    /// artifact's resource_link to the matching tool_result block (positioning),
    /// and disambiguate when tools run in parallel.
    pub tool_use_id: String,
    /// UUID of the file in the files table
    pub file_id: String,
    /// Display filename
    pub filename: String,
    /// MIME type of the file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes
    pub file_size: i64,
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
    /// Tool result text, truncated to 2000 chars (for display in "Show details" panel)
    pub result: Option<String>,
}

/// Event data for MCP elicitation required (server is requesting human input)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamMcpElicitationRequiredData {
    /// Per-elicitation random UUID — POST to /api/mcp/elicitation/{elicitation_id}/respond
    pub elicitation_id: String,
    /// ID of the assistant message that triggered this elicitation (informational)
    pub message_id: Option<String>,
    /// Human-readable prompt from the MCP server
    pub message: String,
    /// JSON Schema describing the requested fields (per SEP-1330)
    pub requested_schema: serde_json::Value,
    /// Display name of the MCP server that sent the request
    pub server: String,
}

/// Event data for MCP tool progress (server sent a `notifications/progress`
/// during a long-running tool call — e.g. a sandbox rootfs download).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamMcpToolProgressData {
    /// ID of the assistant message whose tool call is reporting progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Display name of the MCP server reporting progress.
    pub server: String,
    /// Opaque progress token the server echoed back (the client sets it on
    /// the originating `tools/call`). Lets the UI correlate concurrent calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_token: Option<serde_json::Value>,
    /// Current progress value (monotonically increasing per the MCP spec).
    pub progress: f64,
    /// Total expected units, if known (denominator for a progress bar).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// Human-readable progress message ("Downloading…", "Verifying…").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
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
    /// ITEM-50 (full-disclosure): the EXTERNAL destination host the tool would
    /// send data to (e.g. `api.example.com`), so the human reviews a
    /// *data-egress* decision and not just a verb. `None` for built-in /
    /// loopback / stdio servers, which have no meaningful external destination
    /// (the card treats a `None` here as a local call).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_host: Option<String>,
    /// ITEM-50 (full-disclosure): the tool's FULL, EXACT advertised description
    /// (never summarized/truncated — poisoning hides in truncation). Best-effort
    /// (`None` when the server is unreachable or advertises no description).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Event data for a `run_js` script's per-call tool approval.
///
/// Emitted while a `run_js` script is SUSPENDED in-process awaiting the user's
/// decision on a gated sub-tool call. Unlike `McpApprovalRequired` (a
/// turn-boundary flow resumed by re-sending the message), this is resolved by a
/// SIDE-CHANNEL `POST /api/mcp/elicitation/{elicitation_id}/respond` — the same
/// in-process oneshot mechanism `ask_user` uses — because a live QuickJS call
/// stack cannot survive an HTTP-request boundary. `accept` → the sub-tool runs;
/// `decline`/`cancel` → the host fn throws a catchable error into the script.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SSEChatStreamRunJsApprovalRequiredData {
    /// Per-approval random UUID — POST to /api/mcp/elicitation/{elicitation_id}/respond
    pub elicitation_id: String,
    /// Name of the sub-tool the script wants to call
    pub tool_name: String,
    /// MCP server that owns the sub-tool (display name)
    pub server: String,
    /// Sub-tool input parameters
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
    /// MCP server requires structured human input (elicitation)
    McpElicitationRequired(SSEChatStreamMcpElicitationRequiredData),
    /// MCP server reported progress on a long-running tool call
    McpToolProgress(SSEChatStreamMcpToolProgressData),
    /// A tool created an artifact file (via MCP resource_link) that should be shown as a file card
    ArtifactCreated(SSEChatStreamArtifactCreatedData),
    /// A `run_js` script is suspended awaiting approval of a gated sub-tool call
    RunJsApprovalRequired(SSEChatStreamRunJsApprovalRequiredData),
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
        /// ID of the MCP server that executed this tool (UUID string)
        #[serde(skip_serializing_if = "Option::is_none")]
        server_id: Option<String>,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        /// Inline file attachment returned by a tool (base64-encoded content).
        /// Mirrors `McpContentData::ToolResult.attachment` so the
        /// `to_message_content()` serde roundtrip in content.rs preserves
        /// it through DB persistence. Without this field here, serde
        /// silently drops it when re-deserialising into MessageContentData.
        #[serde(skip_serializing_if = "Option::is_none")]
        attachment: Option<crate::modules::mcp::chat_extension::content::RichFile>,
        /// Inline images returned by a tool (base64). Mirrors
        /// `McpContentData::ToolResult.images`; replayed to the model as image
        /// blocks. Same serde-roundtrip requirement as `attachment` above.
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<crate::modules::mcp::chat_extension::content::RichFile>>,
        /// References to persisted files returned by a tool (MCP
        /// resource_link). MUST be persisted on the assistant message —
        /// the frontend's `MessageFilesView` keys inline file-preview
        /// rendering off this field. Missing-from-schema means missing-
        /// from-storage means no inline preview.
        #[serde(skip_serializing_if = "Option::is_none")]
        resource_links: Option<Vec<crate::modules::mcp::chat_extension::content::ResourceLink>>,
        /// System context for the LLM (e.g. download URLs) — never rendered to users.
        /// Stripped from API responses by strip_hidden_content_serialize in core/models/content.rs.
        #[serde(skip_serializing_if = "Option::is_none")]
        hidden_content: Option<String>,
        /// The MCP tool response's `structuredContent`, persisted verbatim. Mirrors
        /// `McpContentData::ToolResult.structured_content` so the `to_message_content()`
        /// serde roundtrip preserves it through DB persistence (same requirement as
        /// `attachment`/`resource_links` above). UI-only — content-type renderers (e.g.
        /// the literature screening card) read it, and the model recalls it via
        /// `get_tool_result`; it is NOT forwarded to the LLM by `to_content_block`.
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_content: Option<serde_json::Value>,
    },

    /// Elicitation request — MCP server asking the user for structured input.
    /// Persisted to DB immediately when the request arrives so it survives page reloads.
    ElicitationRequest {
        /// Per-elicitation random UUID matching the SSE event
        elicitation_id: String,
        /// Human-readable prompt from the MCP server
        message: String,
        /// JSON Schema (SEP-1330) describing the requested fields
        requested_schema: serde_json::Value,
        /// Display name of the MCP server that sent the request
        server: String,
        /// "pending" | "accepted" | "declined" | "cancelled"
        status: String,
        /// User's submitted field values (only present when status = "accepted")
        #[serde(skip_serializing_if = "Option::is_none")]
        response_content: Option<serde_json::Value>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The artifactCreated event must carry `tool_use_id` so the frontend can
    /// attach the artifact's resource_link to the matching tool_result block
    /// (inline positioning) and disambiguate parallel tool calls.
    #[test]
    fn artifact_created_event_serializes_tool_use_id() {
        let data = SSEChatStreamArtifactCreatedData {
            tool_use_id: "toolu_42".to_string(),
            file_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            filename: "plot.png".to_string(),
            mime_type: Some("image/png".to_string()),
            file_size: 1234,
        };

        let json = serde_json::to_value(&data).expect("serialize");
        assert_eq!(json["tool_use_id"], "toolu_42");
        assert_eq!(json["file_id"], "550e8400-e29b-41d4-a716-446655440000");

        // Round-trips back without losing the association.
        let back: SSEChatStreamArtifactCreatedData =
            serde_json::from_value(json).expect("deserialize");
        assert_eq!(back.tool_use_id, "toolu_42");
    }
}
