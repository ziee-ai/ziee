//! Built-in MCP server exposing `get_tool_result` — exact, read-only recall of a
//! prior tool result by `tool_use_id` within the current conversation.
//!
//! Motivation: `clear_old_tool_results` (chat streaming) trims old/oversized tool
//! results from what's SENT to the model to bound context; stored history is
//! untouched. This tool lets the model recover the EXACT prior result by id —
//! deterministic, no re-execution (unlike re-calling a live tool, which for a
//! live search returns different results). It also returns the persisted
//! `structuredContent`, so the model can read the full structured detail (e.g.
//! full abstracts of an earlier `literature_search`) on demand. Benefits every
//! built-in / third-party tool, not just literature search.
//!
//! Always-attached for tool-capable conversations (like `elicitation_mcp`),
//! approval-bypassed (read-only), gated on `profile::read` (every authenticated
//! user — the recall only returns the caller's own, already-seen conversation
//! data, scoped to a conversation the caller owns).

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod handlers;
pub mod routes;
pub mod tools;

/// Deterministic UUID for the built-in tool_result MCP server row.
/// Stable across deployments (mirrors `memory_mcp_server_id`).
pub fn tool_result_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"tool_result.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static TOOL_RESULT_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "tool_result_mcp",
    // After mcp (65) so mcp_servers exists; after the other built-ins.
    order: 102,
    description: "Built-in MCP server exposing get_tool_result (exact recall of a prior tool result by id)",
    constructor: || Box::new(ToolResultMcpModule::new()),
};

pub struct ToolResultMcpModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl ToolResultMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for ToolResultMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for ToolResultMcpModule {
    fn name(&self) -> &'static str {
        "tool_result_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing get_tool_result"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback regardless of the configured server host (same helper the
        // other built-in MCP servers use) so the URL can't be redirected.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/tool-result/mcp",
            port = ctx.config.server.port,
        );

        let server_id = tool_result_mcp_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            match upsert_builtin_server(&pool, server_id, &loopback_url).await {
                Ok(()) => tracing::info!(
                    "tool_result_mcp: built-in server {server_id} registered at {loopback_url}"
                ),
                Err(e) => tracing::error!("tool_result_mcp: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::tool_result_mcp_router())
    }
}

/// Idempotent upsert of the built-in tool_result MCP server row. On conflict only
/// re-asserts the identity columns (the loopback `url` carries the live port).
async fn upsert_builtin_server(
    pool: &PgPool,
    server_id: Uuid,
    loopback_url: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO mcp_servers (
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in,
            transport_type, url, headers,
            timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        ) VALUES (
            $1, NULL, 'tool_result', 'Tool Result Recall',
            'Built-in recall of a prior tool result by id (get_tool_result)',
            true, true, true,
            'http', $2, '{}'::jsonb,
            30, false, 'auto', 4,
            NOW(), NOW()
        )
        ON CONFLICT (id) DO UPDATE SET
            is_system = EXCLUDED.is_system,
            is_built_in = EXCLUDED.is_built_in,
            transport_type = EXCLUDED.transport_type,
            url = EXCLUDED.url,
            updated_at = NOW()
        "#,
        server_id,
        loopback_url
    )
    .execute(pool)
    .await?;
    Ok(())
}
