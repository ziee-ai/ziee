//! Built-in MCP server for skill body access.
//!
//! Registers `skill.ziee.internal` in `mcp_servers` (`is_built_in=true`,
//! `transport_type='http'`, loopback url) and serves JSON-RPC at
//! `/api/skills/mcp`. The MCP client at `mcp/client/manager.rs`
//! injects a short-lived JWT + `x-conversation-id` for built-in
//! servers, so the handler authenticates the user AND scopes
//! per-conversation hide checks.
//!
//! Two tools — both read-only — that complement the chat extension's
//! Path-B progressive disclosure (listings only in the system prompt;
//! the LLM loads bodies on demand):
//! - `load_skill(name)` — returns the SKILL.md body (frontmatter stripped).
//! - `read_skill_file(name, path)` — returns a supporting file under the
//!   bundle's `<extracted_path>`. Path-safety re-checked at read time on
//!   top of extract-time guarantees.
//!
//! Cache: per-process file-content LRU (64 MiB / 5-min TTL) shared by
//! both tools; invalidated by `SkillUpdated` events.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod file_cache;
pub mod handlers;
pub mod repository;
pub mod routes;
pub mod tools;


/// Deterministic UUID for the built-in skill MCP server row. Stable
/// across deployments. Mirrors `memory_mcp_server_id` /
/// `files_mcp_server_id`.
pub fn skill_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"skill.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static SKILL_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "skill_mcp",
    // After skill (81 — owns the skills table) and mcp (65 — owns the
    // mcp_servers table + client). Same band as memory_mcp (85) /
    // files_mcp (86).
    order: 87,
    description: "Built-in MCP server exposing skill load + read tools",
    constructor: || Box::new(SkillMcpModule::new()),
};

pub struct SkillMcpModule {
    pool: Option<Arc<PgPool>>,
}

impl SkillMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for SkillMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for SkillMcpModule {
    fn name(&self) -> &'static str {
        "skill_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing skill load + read tools"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback (defense in depth — JWTs the MCP client signs
        // for built-in servers MUST NOT leave the host).
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/skills/mcp",
            port = ctx.config.server.port,
        );

        let server_id = skill_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::SkillMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "skill_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => tracing::error!(
                    "skill_mcp: upsert_builtin_server failed: {e:?}"
                ),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::skill_mcp_router())
    }
}
