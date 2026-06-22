use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use super::traits::{McpClient, Prompt, PromptResult, Resource, Tool, ToolResult};
use super::stdio::StdioMcpClient;
use super::http::HttpMcpClient;
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::sampling::SamplingHandler;
use crate::modules::mcp::tool_calls::models::McpCallContext;

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
    server_id: Uuid,
    client: Box<dyn McpClient>,
    #[allow(dead_code)] // Used by age() method for monitoring (Phase 3)
    created_at: Instant,
    last_used: Instant,
    /// Who/where/how this session's tool calls are recorded. Stamped by the
    /// manager (`get_or_create_with_context`) or the sampling sites; empty
    /// (`user_id: None`) for pooled non-tool-call sessions, in which case
    /// `call_tool` records nothing. See `tool_calls/`.
    call_ctx: McpCallContext,
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
            call_ctx: McpCallContext::default(),
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
            call_ctx: McpCallContext::default(),
        })
    }

    #[allow(dead_code)] // For future monitoring/debugging features
    pub fn server_id(&self) -> Uuid {
        self.server_id
    }

    /// Stamp the recording context for this session's tool calls. Called by
    /// `McpSessionManager::get_or_create_with_context` (which knows user /
    /// conversation / message / server) and by the chat sampling sites (which
    /// build sessions directly via `new_with_sampling`).
    pub fn set_call_context(&mut self, ctx: McpCallContext) {
        self.call_ctx = ctx;
    }

    /// Stamp the workflow run that owns this session's tool calls (E4). Called
    /// by the workflow `ToolDispatcher` after `get_or_create_with_context`, so
    /// the recorded `mcp_tool_calls` row links back to the run — without adding
    /// a param to the manager method every other caller would pass `None` for.
    pub fn set_workflow_run(&mut self, run_id: Uuid) {
        self.call_ctx.workflow_run_id = Some(run_id);
    }

    /// Record a finished tool call to the `mcp_tool_calls` history, then emit
    /// an owner-scoped sync event. Fire-and-forget: a DB hiccup must NEVER
    /// fail the tool call, so we `tokio::spawn` and only log on error. Skips
    /// silently when the session carries no owner (an unstamped session).
    fn record_call(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        outcome: &Result<ToolResult, AppError>,
        started_at: time::OffsetDateTime,
        elapsed_ms: i64,
    ) {
        use crate::modules::mcp::tool_calls::record::build_record;

        let Some(create) = build_record(
            self.server_id,
            &self.call_ctx,
            tool_name,
            arguments,
            outcome,
            started_at,
            elapsed_ms,
        ) else {
            return; // unstamped session (no owner) — nothing to record
        };

        let owner = create.user_id;
        tokio::spawn(async move {
            // Defensive: in pre-init scaffolding (no RepositoryFactory) `Repos`
            // would panic. Recording is fire-and-forget — skip, don't crash.
            if !crate::core::is_repos_initialized() {
                return;
            }
            match crate::core::Repos.mcp.record_tool_call(create).await {
                Ok(row) => {
                    use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish};
                    // Detached background task → no request connection, so
                    // origin = None (the originating device also refetches).
                    publish(
                        SyncEntity::McpToolCall,
                        SyncAction::Create,
                        row.id,
                        Audience::owner(owner),
                        None,
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mcp: failed to record tool call");
                }
            }
        });
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
        let started_at = time::OffsetDateTime::now_utc();
        let t0 = Instant::now();
        let result = self
            .client
            .call_tool(name, arguments.clone(), message_id, sse_tx, elicit_notify_tx)
            .await;
        let elapsed_ms = t0.elapsed().as_millis() as i64;
        // Record every invocation (chat / rest / always / sampling / approval,
        // incl. built-ins). Fire-and-forget; cannot affect the call's result.
        self.record_call(name, &arguments, &result, started_at, elapsed_ms);
        result
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
