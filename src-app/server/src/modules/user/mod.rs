// User module - User and group management
pub mod models;
pub mod types;
pub mod permissions;
pub mod repository;
pub mod events;
mod handlers;
mod routes;
mod group_routes;
mod service;

// Re-exports
pub use models::*;
pub use repository::{GroupRepository, UserRepository};
pub use routes::user_router;
pub use group_routes::group_router;
pub use service::UserService;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleContext, ModuleEntry, MODULE_ENTRIES};

/// Register user module
#[distributed_slice(MODULE_ENTRIES)]
static USER_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "user",
    order: 10,
    description: "User and group management",
    constructor: || Box::new(UserModule::new()),
};

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
        if let Some(_pool) = &self.pool {
            router
                .merge(user_router())
                .merge(group_router())
        } else {
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
