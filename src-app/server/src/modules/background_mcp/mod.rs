//! Built-in MCP server exposing the generalized BACKGROUND-RUN backbone to the
//! chat model (ITEM-17 / DEC-33/36).
//!
//! Registers `background.ziee.internal` in `mcp_servers`
//! (`is_built_in=true`, `is_system=true`, `transport_type='http'`, loopback url)
//! and serves JSON-RPC at `POST/GET /api/background/mcp`. Mirrors
//! `workflow_mcp` / `memory_mcp` / `control_mcp` registration. The MCP client at
//! `mcp/client/manager.rs` injects a short-lived JWT + `x-conversation-id` for
//! built-in servers, so the handler authenticates the user (gated on
//! `background::use`) AND scopes the run to the originating conversation.
//!
//! Three tools — the uniform "not a one-off MCP hack" surface on the
//! `workflow_runs`-backed backbone (a `JobKind` run + the shared runner):
//! - `spawn_background{kind, spec}` — creates a background `workflow_runs` row of
//!   the given [`crate::modules::workflow::models::JobKind`] via
//!   `insert_background_run` + `spawn_background_run` (fire-and-forget), returns an
//!   opaque owner-scoped `run_id`. This is a WRITE — it launches a detached agent
//!   — so it is routed through the reviewer/approval gate (NOT approval-bypassed;
//!   see `background_call_needs_approval` in `tools.rs` + the `is_background` arm
//!   in `mcp/chat_extension/mcp.rs`).
//! - `check_status{run_id}` — cheap owner-scoped read of the run row (state +
//!   progress); a cross-user id yields 404. READ, approval-bypassed.
//! - `collect_result{run_id}` — idempotent, paged owner-scoped read of
//!   `final_output_json`; a cross-user id yields 404. READ, approval-bypassed.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod handlers;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod run_notes;
pub mod tools;

// ── ITEM-26 (inbox): declare the background-run completion notification kind ──
//
// The `notification` inbox already provides the full owner-scoped list /
// mark-read / unread-count REST (in `ziee-notification`); background sub-agent
// completions ALREADY write a durable typed row (`tools.rs`, kind
// `"background_run_result"`, payload `{workflow_run_id, conversation_id}`). The
// only backbone gap for a "unified agent inbox" was that this kind wasn't
// DECLARED in the SDK's per-module kind registry, so `GET /api/notifications/kinds`
// didn't advertise it and the FE couldn't build its agent/background filter +
// renderer from the registry. Declaring it here (additive `#[distributed_slice]`,
// no OpenAPI change) closes that gap. Everything else about the inbox is a
// FRONTEND composition of the existing notification inbox + the existing
// owner-scoped background-run list/SSE (DEC-65).
#[distributed_slice(ziee_notification::registry::NOTIFICATION_KINDS)]
static BACKGROUND_RUN_RESULT_KIND: ziee_notification::registry::NotificationKindDescriptor =
    ziee_notification::registry::NotificationKindDescriptor {
        kind: "background_run_result",
        description: "A detached background sub-agent run finished; its result is ready to collect.",
    };

/// Deterministic UUID for the built-in background MCP server row. Stable across
/// deployments. Mirrors `workflow_mcp_server_id` / `control_mcp_server_id`.
pub fn background_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"background.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static BACKGROUND_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "background_mcp",
    // After workflow (82 — owns workflow_runs + the runner backbone) and mcp
    // (65 — owns mcp_servers + client) so the row upsert + the backbone helpers
    // exist. Same band as workflow_mcp (88) / control_mcp (88).
    order: 88,
    description: "Built-in MCP server exposing the background-run backbone (spawn/check/collect)",
    constructor: || Box::new(BackgroundMcpModule::new()),
};

pub struct BackgroundMcpModule {
    pool: Option<Arc<PgPool>>,
}

impl BackgroundMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for BackgroundMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for BackgroundMcpModule {
    fn name(&self) -> &'static str {
        "background_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing the background-run backbone (spawn/check/collect)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback (defense in depth — the JWTs the MCP client signs for
        // built-in servers MUST NOT leave the host).
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/background/mcp",
            port = ctx.config.server.port,
        );

        let server_id = background_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::BackgroundMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "background_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => {
                    tracing::error!("background_mcp: upsert_builtin_server failed: {e:?}")
                }
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::background_mcp_router())
    }
}
