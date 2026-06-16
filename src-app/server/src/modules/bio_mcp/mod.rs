//! Built-in MCP server for biomedical data (BioMCP, vendor-and-wrap).
//!
//! Registers `bio.ziee.internal` as an **admin-configurable** row in
//! `mcp_servers` (`is_built_in=true`, `transport_type='http'`) pointing at
//! a loopback proxy route on the same axum app (`/api/bio/mcp`). Unlike the
//! zero-config built-ins (files/memory), the bio row stays editable so
//! admins set the upstream API keys as secret entries in its Headers.
//!
//! The proxy supervises a single long-lived `biomcp serve-http` sidecar
//! (see `supervisor`), injecting the configured keys into its process env,
//! and byte-pipes MCP streamable-HTTP through to it. Connected-only: the
//! sidecar queries live upstream APIs. When the build staged a stub binary
//! (no network / unsupported triple) or the operator left `bio_mcp.enabled`
//! false, the module self-disables with a clear log.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod chat_extension;
pub mod embedded;
pub mod handlers;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod supervisor;

pub use repository::BioMcpRepository;

/// Deterministic UUID for the built-in BioMCP server row. Stable across
/// deployments (mirrors `memory_mcp_server_id` / `code_sandbox_server_id`).
pub fn bio_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"bio.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static BIO_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "bio_mcp",
    // After mcp (65) so the mcp_servers table is initialized, and after the
    // other *_mcp built-ins (memory_mcp 85 … elicitation_mcp 89) for a
    // stable boot order. 91 to avoid the tie with `app` (90).
    order: 91,
    description: "Built-in MCP server exposing biomedical database tools (BioMCP)",
    constructor: || Box::new(BioMcpModule::new()),
};

pub struct BioMcpModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl BioMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for BioMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for BioMcpModule {
    fn name(&self) -> &'static str {
        "bio_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing biomedical database tools (BioMCP)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Deploy-level kill switch — ON by default (an absent `bio_mcp:`
        // config section means enabled). Operators opt OUT with
        // `bio_mcp: { enabled: false }`. The module still self-disables
        // below when the embedded binary is a build stub.
        let enabled = ctx
            .config
            .bio_mcp
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(true);
        if !enabled {
            tracing::info!("bio_mcp: disabled in config; skipping registration");
            return Ok(());
        }

        // The build may have staged a zero-byte stub (no network / missing
        // asset / checksum mismatch) — self-disable rather than try to
        // spawn a 0-byte "binary".
        if !embedded::biomcp_available() {
            tracing::warn!(
                "bio_mcp: embedded biomcp binary unavailable (build staged a stub); \
                 not registering. Rebuild with network access to enable."
            );
            return Ok(());
        }

        // Defense-in-depth: pin the built-in MCP URL to loopback regardless
        // of `server.host` (same helper code_sandbox/memory_mcp use), so a
        // misconfigured host can't make the JWT-bearing client dial elsewhere.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/bio/mcp",
            port = ctx.config.server.port,
        );

        let server_id = bio_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::BioMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "bio_mcp: registered built-in server {server_id} at {upsert_url} \
                     (sidecar starts on first query)"
                ),
                Err(e) => tracing::error!("bio_mcp: upsert_builtin_server failed: {e:?}"),
            }
        });

        // Evict the managed sidecar when idle / reap it if it dies.
        supervisor::spawn_idle_reaper();

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::bio_mcp_router())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bio_mcp_server_id_is_deterministic_and_distinct() {
        let a = bio_mcp_server_id();
        let b = bio_mcp_server_id();
        assert_eq!(a, b, "server id must be stable across calls");
        // Must NOT collide with the zero-config edit deny-list ids
        // (files/memory/elicitation) — that's what keeps bio's Headers
        // editable while still being approval-bypassed.
        assert_ne!(a, crate::modules::files_mcp::files_mcp_server_id());
        assert_ne!(a, crate::modules::memory_mcp::memory_mcp_server_id());
        assert_ne!(
            a,
            crate::modules::elicitation_mcp::elicitation_mcp_server_id()
        );
        assert_ne!(a, crate::modules::code_sandbox::code_sandbox_server_id());
    }
}
