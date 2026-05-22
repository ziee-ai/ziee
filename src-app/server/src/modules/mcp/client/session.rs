use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use super::traits::{McpClient, Prompt, PromptResult, Resource, Tool, ToolResult};
use super::stdio::StdioMcpClient;
use super::http::HttpMcpClient;
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::sampling::SamplingHandler;

/// Error returned when a server is configured with the deprecated SSE
/// transport (removed in MCP 2025-03-26 in favour of Streamable HTTP).
fn sse_deprecated_error() -> AppError {
    AppError::bad_request(
        "DEPRECATED_TRANSPORT",
        "The SSE (HTTP+SSE) transport was deprecated in MCP 2025-03-26. \
         Reconfigure this server to use the Streamable HTTP transport (\"http\") \
         instead. The new transport uses the same JSON-RPC payloads but a \
         single POST endpoint that may respond with either JSON or SSE."
    )
}

pub struct McpSession {
    #[allow(dead_code)] // Kept for debugging/logging (future use)
    server_id: Uuid,
    client: Box<dyn McpClient>,
    #[allow(dead_code)] // Used by age() method for monitoring (Phase 3)
    created_at: Instant,
    last_used: Instant,
}

impl McpSession {
    /// Build an HTTP client, attaching the server's stored OAuth
    /// client_credentials config when one exists. Built-in servers skip the
    /// lookup (they authenticate with a short-lived internal JWT).
    async fn build_http_client(
        server: &McpServer,
        handler: Option<Arc<dyn SamplingHandler>>,
    ) -> Result<HttpMcpClient, AppError> {
        let oauth = if server.is_built_in {
            None
        } else {
            crate::core::Repos
                .mcp
                .get_oauth_config(server.id)
                .await?
                .map(|c| c.into_client_config())
        };
        HttpMcpClient::new_internal(server.clone(), handler, oauth)
    }

    pub async fn new(server: McpServer) -> Result<Self, AppError> {
        // Create appropriate client based on transport type.
        // SSE is intentionally rejected — see sse_deprecated_error doc.
        let mut client: Box<dyn McpClient> = match server.transport_type {
            TransportType::Stdio => Box::new(StdioMcpClient::new(server.clone())?),
            TransportType::Http => Box::new(Self::build_http_client(&server, None).await?),
            TransportType::Sse => return Err(sse_deprecated_error()),
        };

        client.connect().await?;

        Ok(Self {
            server_id: server.id,
            client,
            created_at: Instant::now(),
            last_used: Instant::now(),
        })
    }

    /// Create a session with a sampling handler attached.
    /// The handler enables the MCP server to request LLM completions inline.
    /// Only HTTP transport supports sampling currently.
    pub async fn new_with_sampling(
        server: McpServer,
        handler: Arc<dyn SamplingHandler>,
    ) -> Result<Self, AppError> {
        let mut client: Box<dyn McpClient> = match server.transport_type {
            TransportType::Http => Box::new(Self::build_http_client(&server, Some(handler)).await?),
            TransportType::Stdio => {
                tracing::warn!(
                    "Sampling is only supported for HTTP transport; server '{}' uses stdio. Falling back to non-sampling session.",
                    server.name
                );
                Box::new(StdioMcpClient::new(server.clone())?)
            }
            TransportType::Sse => return Err(sse_deprecated_error()),
        };

        client.connect().await?;

        Ok(Self {
            server_id: server.id,
            client,
            created_at: Instant::now(),
            last_used: Instant::now(),
        })
    }

    #[allow(dead_code)] // For future monitoring/debugging features
    pub fn server_id(&self) -> Uuid {
        self.server_id
    }

    #[allow(dead_code)] // For future monitoring features (Phase 3)
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    #[allow(dead_code)] // Used by cleanup_idle() for session management (Phase 3)
    pub fn idle_time(&self) -> std::time::Duration {
        self.last_used.elapsed()
    }

    pub async fn list_tools(&mut self) -> Result<Vec<Tool>, AppError> {
        self.last_used = Instant::now();
        self.client.list_tools().await
    }

    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
        message_id: Option<uuid::Uuid>,
        sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
        elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
    ) -> Result<ToolResult, AppError> {
        self.last_used = Instant::now();
        self.client.call_tool(name, arguments, message_id, sse_tx, elicit_notify_tx).await
    }

    pub async fn list_resources(&mut self) -> Result<Vec<Resource>, AppError> {
        self.last_used = Instant::now();
        self.client.list_resources().await
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<serde_json::Value, AppError> {
        self.last_used = Instant::now();
        self.client.read_resource(uri).await
    }

    pub async fn list_prompts(&mut self) -> Result<Vec<Prompt>, AppError> {
        self.last_used = Instant::now();
        self.client.list_prompts().await
    }

    pub async fn get_prompt(
        &mut self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<PromptResult, AppError> {
        self.last_used = Instant::now();
        self.client.get_prompt(name, arguments).await
    }

    pub async fn ping(&mut self) -> Result<(), AppError> {
        self.last_used = Instant::now();
        self.client.ping().await
    }

    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        self.client.disconnect().await
    }
}
