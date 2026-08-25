// User module - User and group management.
//
// Chunk BA-full moved the user CORE (repositories + `query!` macros, wire DTOs,
// the effective-permissions service, user-lifecycle events) into `ziee-auth`.
// This module keeps the HTTP/aide boundary (`handlers` — including the
// domain-coupled `delete_user` admin cascade over skill/file/hub cleanup —
// `routes`, `permissions`) + the `models` shim, and re-exports the moved pieces
// so every `crate::modules::user::…` call site is unchanged.
pub mod handlers;
pub mod models;
pub mod permissions;
mod routes;

// Re-export shims for the moved core (module paths + item re-exports preserved).
pub use models::*;
pub use routes::{group_router, user_router};
#[allow(unused_imports)]
pub use ziee_auth::user::{GroupRepository, UserRepository, UserService, events, repository, types};

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

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
            router.merge(user_router()).merge(group_router())
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
