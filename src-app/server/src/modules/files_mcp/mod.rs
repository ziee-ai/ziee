//! Built-in MCP server for agentic file access.
//!
//! Registers `files.ziee.internal` as a row in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`, loopback url) and serves
//! JSON-RPC at `/api/files/mcp`. The MCP client at `mcp/client/manager.rs`
//! injects a short-lived JWT + `x-conversation-id` for built-in servers, so the
//! handler authenticates the user AND scopes reads to the conversation.
//!
//! Three read-only tools: `list_files` / `read_file` / `grep_files`, served over
//! the shared `file::available_files` resolver (project files + conversation
//! attachments). Mirrors `memory_mcp` for the boot/registration shape and
//! `code_sandbox` for the conversation-ownership shape.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod edits;
pub mod handlers;
pub mod repository;
pub mod routes;
pub mod tools;

pub use repository::FilesMcpRepository;

/// Deterministic UUID for the built-in files MCP server row. Stable across
/// deployments. Mirrors `memory_mcp_server_id`.
pub fn files_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"files.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static FILES_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "files_mcp",
    // After file (order 31, so storage is initialized), mcp (65, so the
    // mcp_servers table + client exist) and project, so all of the
    // available-files resolver's dependencies are live before we upsert the
    // built-in server row.
    order: 86,
    description: "Built-in MCP server exposing agentic file tools (list/read/grep)",
    constructor: || Box::new(FilesMcpModule::new()),
};

pub struct FilesMcpModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl FilesMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for FilesMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for FilesMcpModule {
    fn name(&self) -> &'static str {
        "files_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing agentic file tools (list/read/grep)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback (same defense as memory_mcp) so the JWT-bearing MCP
        // client never ships tokens off-host.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/files/mcp",
            port = ctx.config.server.port,
        );

        let server_id = files_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::FilesMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "files_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => tracing::error!("files_mcp: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::files_mcp_router())
    }
}
