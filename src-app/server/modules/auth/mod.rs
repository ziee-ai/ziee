// Auth module - JWT-based authentication
pub mod backend;
mod errors;
pub mod handlers;
pub mod jwt;
pub mod jwt_extractor;
pub mod password;
pub mod providers;
pub mod routes;
pub mod types;

// Re-exports
pub use backend::{AuthBackend, AuthSession, AuthSessionWrapper, Credentials};
pub use errors::*;
pub use jwt::{Claims, JwtService, TokenPair};
pub use jwt_extractor::{JwtAuth, OptionalJwtAuth};
pub use providers::{AuthError as ProvidersAuthError, AuthProviderTrait, AuthResult, OAuthResult, UserAttributes};
pub use routes::auth_routes;
pub use types::{
    AuthResponse, MeResponse, RegisterRequest, LoginRequest,
    RefreshTokenRequest, OAuthAuthorizeQuery, OAuthCallbackQuery,
};

// Modules to be added:
// - provisioning: User auto-provisioning from external auth

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleContext};

/// Auth module for authentication and authorization
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

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create a stateful auth router
            let auth_router_with_state = ApiRouter::new()
                .nest("/auth", auth_routes())
                .with_state((**pool).clone());

            // Merge the stateful router into the provided stateless router
            router.merge(auth_router_with_state)
        } else {
            // Pool not initialized - this shouldn't happen in normal flow
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
