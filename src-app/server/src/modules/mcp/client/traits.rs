use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use schemars::JsonSchema;

use crate::common::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    #[serde(alias = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// MCP Prompt template metadata (per MCP spec § server/prompts).
/// Returned by `prompts/list`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Prompt {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// Result of a `prompts/get` call — server's rendered prompt messages.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PromptResult {
    pub description: Option<String>,
    pub messages: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolContent {
    #[serde(flatten)]
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default, alias = "isError")]
    pub is_error: bool,
    /// The tool response's MCP `structuredContent` object, if any. Captured here
    /// (via the `structuredContent` alias) so the HTTP client — which deserializes
    /// the whole JSON-RPC `result` into this struct — preserves it; it is then
    /// persisted on the `tool_result` content block for the UI + `get_tool_result`.
    /// `#[serde(default)]` keeps responses without it back-compatible.
    #[serde(default, alias = "structuredContent")]
    pub structured_content: Option<Value>,
}

#[async_trait]
pub trait McpClient: Send + Sync {
    /// Connect to the MCP server
    async fn connect(&mut self) -> Result<(), AppError>;

    /// Disconnect from the MCP server
    async fn disconnect(&mut self) -> Result<(), AppError>;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// List available tools
    async fn list_tools(&mut self) -> Result<Vec<Tool>, AppError>;

    /// Call a tool.
    ///
    /// `sse_tx` — optional Axum browser SSE sender. When provided, the HTTP client
    /// will forward `mcpElicitationRequired` events to the browser if the MCP server
    /// sends an `elicitation/create` request mid-stream. Non-HTTP clients (stdio, SSE)
    /// accept the parameter but ignore it.
    ///
    /// `message_id` — ID of the assistant message driving this tool call. Used by the
    /// HTTP client to key the elicitation registry so that only the message owner can
    /// respond via the REST endpoint. Non-HTTP clients accept and ignore it.
    async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
        message_id: Option<uuid::Uuid>,
        sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
        elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
    ) -> Result<ToolResult, AppError>;

    /// List available resources
    async fn list_resources(&mut self) -> Result<Vec<Resource>, AppError>;

    /// Read a resource
    async fn read_resource(&mut self, uri: &str) -> Result<Value, AppError>;

    /// List available prompt templates (MCP spec § server/prompts).
    /// Returns an empty Vec if the server didn't advertise the `prompts`
    /// capability or doesn't implement this method.
    async fn list_prompts(&mut self) -> Result<Vec<Prompt>, AppError>;

    /// Render a prompt template with the given arguments.
    async fn get_prompt(
        &mut self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<PromptResult, AppError>;

    /// Liveness check (MCP spec § utilities/ping). Returns Ok if the server
    /// responds within the underlying transport's timeout.
    async fn ping(&mut self) -> Result<(), AppError>;

    /// Notify the server that a previously-issued request is being abandoned
    /// (MCP spec § utilities/cancellation — `notifications/cancelled`). This is
    /// a fire-and-forget notification; the server SHOULD stop work tied to
    /// `request_id`. Transports that can't deliver it (stdio's rmcp wrapper)
    /// no-op. `request_id` is the JSON-RPC id the client assigned to the
    /// in-flight request.
    async fn cancel(&mut self, request_id: i64, reason: &str) -> Result<(), AppError>;
}
