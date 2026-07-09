//! Built-in MCP server + REST module for user-owned KNOWLEDGE BASES.
//!
//! A knowledge base is a named, standalone-reusable SET of the user's files that
//! the agent retrieves from via the built-in `search_knowledge` MCP tool (RAG at
//! scale). Chunks/embeddings live in the shared `file_chunks` table (migration
//! 99), so a KB is pure grouping — retrieval resolves a KB to its file_ids and
//! calls the (reranked) `file_rag::retrieval::semantic_search`.
//!
//! Registers `knowledge_base.ziee.internal` in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`) pointing at loopback
//! `/api/knowledge-base/mcp`. Mirrors the web_search / citations built-ins.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod chat_extension;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod tools;

pub use repository::KnowledgeBaseRepository;

/// Deterministic UUID for the built-in knowledge_base MCP server row.
/// Stable across deployments (mirrors `citations_server_id`).
pub fn knowledge_base_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"knowledge_base.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static KNOWLEDGE_BASE_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "knowledge_base",
    // After mcp (65) so mcp_servers exists; after file_rag (87) whose retrieval
    // this module reuses.
    order: 104,
    description: "Built-in MCP server for user-owned knowledge-base retrieval",
    constructor: || Box::new(KnowledgeBaseModule::new()),
};

pub struct KnowledgeBaseModule {
    pool: Option<Arc<PgPool>>,
}

impl KnowledgeBaseModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for KnowledgeBaseModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for KnowledgeBaseModule {
    fn name(&self) -> &'static str {
        "knowledge_base"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server for user-owned knowledge-base retrieval"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback regardless of the configured host (same helper the other
        // built-ins use) so the MCP URL can never be redirected off-loopback.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/knowledge-base/mcp",
            port = ctx.config.server.port,
        );

        let server_id = knowledge_base_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::KnowledgeBaseRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &loopback_url).await {
                Ok(()) => tracing::info!(
                    "knowledge_base: built-in server {server_id} registered at {loopback_url}"
                ),
                Err(e) => {
                    tracing::error!("knowledge_base: upsert_builtin_server failed: {e:?}")
                }
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::knowledge_base_router())
    }
}

#[cfg(test)]
mod id_tests {
    use super::knowledge_base_server_id;
    use uuid::Uuid;

    // TEST-14 (ITEM-16): the built-in server id is a STABLE deterministic v5
    // UUID (a change to the namespace input would orphan every existing
    // mcp_servers row / conversation attachment).
    #[test]
    fn knowledge_base_server_id_is_stable() {
        let expected = Uuid::parse_str("70577fd2-afe1-52c7-a629-9464c01fb1e5").unwrap();
        assert_eq!(knowledge_base_server_id(), expected);
        // pure + deterministic
        assert_eq!(knowledge_base_server_id(), knowledge_base_server_id());
        // distinct from the URL namespace itself (sanity)
        assert_ne!(knowledge_base_server_id(), Uuid::NAMESPACE_URL);
    }
}
