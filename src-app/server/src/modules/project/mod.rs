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
pub mod core;
pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;

#[path = "extension_registration.rs"]
mod extension_registration;
use extension_registration::auto_register_project_extensions;

pub use core::extension::ProjectExtensionRegistry;
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
    extension_registry: Option<Arc<ProjectExtensionRegistry>>,
}

impl ProjectModule {
    pub fn new() -> Self {
        Self {
            pool: None,
            extension_registry: None,
        }
    }

    /// Read-only access to the registry. Used by the duplicate handler
    /// to fire the per-extension `on_project_duplicated` hook inside the
    /// project's duplicate transaction.
    // Accessor for the duplicate handler's on_project_duplicated hook; that
    // wiring isn't active yet, so no caller today.
    #[allow(dead_code)]
    pub fn extension_registry(&self) -> Option<Arc<ProjectExtensionRegistry>> {
        self.extension_registry.clone()
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

        // Auto-register project extensions via the PROJECT_EXTENSIONS slice.
        // Sibling modules (e.g. file's project_extension) register themselves;
        // the project module never imports them directly. With no extensions
        // present, the registry is empty and behaves identically (extension
        // routes contribute zero, lifecycle hooks fan out to zero handlers).
        let registry = auto_register_project_extensions(
            (*ctx.db_pool).clone(),
            ctx.config.clone(),
        );
        let registry_arc = Arc::new(registry);
        self.extension_registry = Some(registry_arc.clone());

        // Run each extension's one-time `initialize()` lifecycle hook (DB prep /
        // cache warmup). `init` is sync but runs inside the tokio runtime, so we
        // spawn it; it's best-effort — a failure is logged, not fatal.
        {
            let registry = registry_arc.clone();
            let pool = (*ctx.db_pool).clone();
            tokio::spawn(async move {
                if let Err(e) = registry.initialize_all(&pool).await {
                    tracing::error!("Project extension initialize_all failed: {}", e);
                }
            });
        }

        // Expose the registry as a process-wide singleton so code paths
        // that don't receive it via axum Extension can fan out — notably
        // the project chat-extension, which runs inside chat's streaming
        // pipeline and never sees the project router's layers.
        core::extension::set_global_registry(registry_arc);

        tracing::info!("Project module initialized with auto-registered extensions");
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            // Two router layers:
            //   1. `register_routes(router)` folds in extension routes
            //      (e.g. file's `/projects/{id}/files*`).
            //   2. `project_router()` is the project module's own routes
            //      (CRUD, conversations, MCP settings, duplicate).
            // Both need the registry as an axum Extension so handlers
            // can fan out lifecycle hooks (notably duplicate_project →
            // fire_on_project_duplicated). Axum's `.merge()` does NOT
            // propagate parent `.layer()` calls onto merged routes
            // (see lib.rs comment), so we apply the Extension to EACH
            // router individually before merging.
            if let Some(registry) = &self.extension_registry {
                let ext_routes = registry
                    .register_routes(router)
                    .layer(axum::Extension(registry.clone()));
                let project_routes =
                    project_router().layer(axum::Extension(registry.clone()));
                ext_routes.merge(project_routes)
            } else {
                router.merge(project_router())
            }
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
