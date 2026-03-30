use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use super::traits::{McpClient, Tool, Resource, ToolResult};
use super::stdio::StdioMcpClient;
use super::http::HttpMcpClient;
use super::sse::SseMcpClient;
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::sampling::SamplingHandler;

pub struct McpSession {
    #[allow(dead_code)] // Kept for debugging/logging (future use)
    server_id: Uuid,
    client: Box<dyn McpClient>,
    #[allow(dead_code)] // Used by age() method for monitoring (Phase 3)
    created_at: Instant,
    last_used: Instant,
}

impl McpSession {
    pub async fn new(server: McpServer) -> Result<Self, AppError> {
        // Create appropriate client based on transport type
        let mut client: Box<dyn McpClient> = match server.transport_type {
            TransportType::Stdio => Box::new(StdioMcpClient::new(server.clone())?),
            TransportType::Http => Box::new(HttpMcpClient::new(server.clone())?),
            TransportType::Sse => Box::new(SseMcpClient::new(server.clone())?),
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
            TransportType::Http => Box::new(HttpMcpClient::new_with_sampling(server.clone(), handler)?),
            TransportType::Stdio | TransportType::Sse => {
                // Fallback to regular session for non-HTTP transports
                tracing::warn!(
                    "Sampling is only supported for HTTP transport; server '{}' uses {:?}. Falling back to non-sampling session.",
                    server.name, server.transport_type
                );
                match server.transport_type {
                    TransportType::Stdio => Box::new(StdioMcpClient::new(server.clone())?),
                    TransportType::Sse => Box::new(SseMcpClient::new(server.clone())?),
                    _ => unreachable!(),
                }
            }
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
    ) -> Result<ToolResult, AppError> {
        self.last_used = Instant::now();
        self.client.call_tool(name, arguments).await
    }

    pub async fn list_resources(&mut self) -> Result<Vec<Resource>, AppError> {
        self.last_used = Instant::now();
        self.client.list_resources().await
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<serde_json::Value, AppError> {
        self.last_used = Instant::now();
        self.client.read_resource(uri).await
    }

    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        self.client.disconnect().await
    }
}
