// Auth module - JWT-based authentication
pub mod backend;
pub mod events;
pub mod handlers;
pub mod jwt;
pub mod jwt_extractor;
pub mod password;
pub mod providers;
pub mod refresh_tokens;
mod repository;
pub mod routes;
pub mod types;

// Re-exports
pub use jwt::JwtService;
pub use password::hash_password;
pub use repository::AuthRepository;
pub use routes::auth_routes;
pub use types::AuthResponse;

// Modules to be added:
// - provisioning: User auto-provisioning from external auth

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register auth module
#[distributed_slice(MODULE_ENTRIES)]
static AUTH_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "auth",
    order: 5,
    description: "JWT-based authentication and authorization",
    constructor: || Box::new(AuthModule::new()),
};

/// Auth module for authentication and authorization
/// Note: Kept as manual registration due to complex route state requirements
pub struct AuthModule {
    pool: Option<Arc<PgPool>>,
}

impl AuthModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for AuthModule {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "JWT-based authentication and authorization"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let auth_router_with_state = ApiRouter::new().nest("/auth", auth_routes());
            router.merge(auth_router_with_state)
        } else {
            tracing::error!("AuthModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for AuthModule {
    fn default() -> Self {
        Self::new()
    }
}
