// Auth module - JWT-based authentication
pub mod backend;
pub mod events;
pub mod handlers;
pub mod jwt;
pub mod jwt_extractor;
pub mod password;
pub mod permissions;
pub mod providers;
pub mod refresh_tokens;
mod repository;
pub mod routes;
pub mod types;

// Re-exports
pub use jwt::JwtService;
pub use password::hash_password;
pub use repository::AuthRepository;
pub use routes::{auth_admin_routes, auth_routes};
pub use types::AuthResponse;

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

        // Spawn a periodic cleanup task: prune expired oauth_sessions
        // and pending_account_links rows. Both have TTL columns, but
        // rows that are never re-touched (abandoned OAuth dances,
        // unused link tokens) would accumulate indefinitely. Runs
        // every 5 minutes; failure is logged and ignored — next tick
        // tries again.
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = crate::modules::auth::repository::AuthRepository::new((*pool).clone());
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5 * 60));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                match repo.cleanup_expired_auth_rows().await {
                    Ok((s, p)) if s > 0 || p > 0 => {
                        tracing::debug!(
                            sessions_pruned = s,
                            pending_links_pruned = p,
                            "auth cleanup tick"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = ?e, "auth cleanup tick failed");
                    }
                }
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let auth_router = ApiRouter::new()
                .nest("/auth", auth_routes())
                .merge(auth_admin_routes());
            router.merge(auth_router)
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
