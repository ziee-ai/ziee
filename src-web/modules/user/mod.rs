// User module - User and group management
pub mod models;
pub mod permissions;
pub mod repository;
mod routes;
mod group_routes;
mod service;

// Re-exports
pub use models::*;
pub use repository::{GroupRepository, UserRepository};
pub use routes::user_router;
pub use group_routes::group_router;
pub use service::{GroupService, UserService};

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleContext};

/// User module for user and group management
pub struct UserModule {
    pool: Option<Arc<PgPool>>,
}

impl UserModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for UserModule {
    fn name(&self) -> &'static str {
        "user"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create a stateful user router with permission-based access control
            let user_module_router = ApiRouter::new()
                .merge(user_router())
                .merge(group_router())
                .with_state((**pool).clone());

            // Merge the stateful router into the provided stateless router
            router.merge(user_module_router)
        } else {
            // Pool not initialized - this shouldn't happen in normal flow
            tracing::error!("UserModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for UserModule {
    fn default() -> Self {
        Self::new()
    }
}
