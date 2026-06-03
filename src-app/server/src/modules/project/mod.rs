// Project module
//
// Chat Projects — a flat, per-user grouping above conversations that owns
// persistent instructions, knowledge files, default assistant, default
// model, and inline default MCP settings. The chat/extensions/project
// extension injects this context into every conversation that lives in a
// project. See Plan 5 in the worktree's plan file for the full design.

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::ModuleContext;
use crate::core::EventHandler;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleEntry};

pub mod chat_extension;
pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;

pub use repository::ProjectRepository;
pub use routes::project_router;

/// Register project module via linkme.
#[distributed_slice(MODULE_ENTRIES)]
static PROJECT_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "project",
    order: 47,
    description: "Chat projects: grouping conversations under shared instructions, knowledge files, and defaults",
    constructor: || Box::new(ProjectModule::new()),
};

pub struct ProjectModule {
    pool: Option<Arc<PgPool>>,
}

impl ProjectModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for ProjectModule {
    fn name(&self) -> &'static str {
        "project"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Chat projects (instructions, knowledge, defaults)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            router.merge(project_router())
        } else {
            tracing::error!("ProjectModule: Pool not initialized during route registration");
            router
        }
    }

    fn register_event_handlers(&self) -> Vec<Arc<dyn EventHandler>> {
        // v1: no event reactions
        vec![]
    }
}

impl Default for ProjectModule {
    fn default() -> Self {
        Self::new()
    }
}
