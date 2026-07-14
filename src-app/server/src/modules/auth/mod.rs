// Auth module - JWT-based authentication.
//
// Chunk BA-full moved the auth CORE (repositories + `query!` macros,
// login/register/LDAP/OAuth2, the JWT + Session & Token-Refresh subsystem, the
// cookie helpers, the injected `context` seams, the at-rest secret provider
// repository) into the `ziee-auth` crate. This module keeps the HTTP/aide
// boundary (`handlers` / `routes` / `permissions` / `jwt_extractor` + the
// session-settings REST handlers + the `AuthModule` registration) and re-exports
// the moved pieces as equivalence-preserving shims, so every
// `crate::modules::auth::…` call site is unchanged.

// App-side (HTTP/aide/permission boundary).
pub mod handlers;
pub mod jwt_extractor;
pub mod permissions;
pub mod routes;
pub mod session_settings;

// Re-export shims for the moved core (module paths + item re-exports preserved).
// The `context`/`AuthContext`/sink types are reached via the `context` module
// path (`crate::modules::auth::context::…`), so only the module is re-exported.
#[allow(unused_imports)]
pub use ziee_auth::auth::{
    AuthRepository, AuthResponse, JwtService, SessionSettingsRepository, context, cookie, events,
    hash_password, jwt, password, providers, refresh_tokens, repository, trust_forwarded_headers,
    types,
};

pub use routes::{auth_admin_routes, auth_routes};

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

        // Cache the reverse-proxy trust flag in a static so the
        // free-function OAuth handlers can read it without threading
        // Arc<Config> through every Axum extension layer. OnceCell::set
        // is idempotent on second-call (returns Err which we ignore —
        // module re-init isn't expected but isn't an error condition).
        ziee_auth::auth::set_trust_forwarded_headers(ctx.config.server.trust_forwarded_headers);

        // One-time copy of the YAML jwt lifetimes into the session_settings
        // singleton (migration 129). Writes only while seeded_from_config is
        // FALSE, so an operator's customized YAML values survive the upgrade
        // that introduced the DB-backed setting; thereafter the DB row is
        // authoritative. Failure is non-fatal — mint_session_tokens falls
        // back to the YAML values whenever the DB read fails.
        {
            let pool = ctx.db_pool.clone();
            let access_hours = ctx.config.jwt.access_token_expiry_hours;
            let refresh_days = ctx.config.jwt.refresh_token_expiry_days;
            tokio::spawn(async move {
                let repo = SessionSettingsRepository::new((*pool).clone());
                if let Err(e) = repo.seed_from_config_once(access_hours, refresh_days).await {
                    tracing::warn!(error = ?e, "session_settings config seed failed; DB defaults remain");
                }
            });
        }

        // Spawn a periodic cleanup task: prune expired oauth_sessions
        // and pending_account_links rows. Both have TTL columns, but
        // rows that are never re-touched (abandoned OAuth dances,
        // unused link tokens) would accumulate indefinitely. Runs
        // every 5 minutes; tick failures are logged and the loop
        // continues. The whole loop body runs inside an
        // AssertUnwindSafe::catch_unwind so a panic in one tick
        // (e.g. pool dropped) doesn't silently kill the task —
        // it logs, waits a tick, and tries again.
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = crate::modules::auth::repository::AuthRepository::new((*pool).clone());
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5 * 60));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let outcome = std::panic::AssertUnwindSafe(repo.cleanup_expired_auth_rows());
                let result = futures::FutureExt::catch_unwind(outcome).await;
                match result {
                    Ok(Ok((s, p, r))) if s > 0 || p > 0 || r > 0 => {
                        tracing::debug!(
                            sessions_pruned = s,
                            pending_links_pruned = p,
                            refresh_tokens_pruned = r,
                            "auth cleanup tick"
                        );
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        tracing::warn!(error = ?e, "auth cleanup tick failed");
                    }
                    Err(panic_payload) => {
                        let msg = panic_payload
                            .downcast_ref::<&'static str>()
                            .copied()
                            .or_else(|| {
                                panic_payload
                                    .downcast_ref::<String>()
                                    .map(String::as_str)
                            })
                            .unwrap_or("<non-string panic>");
                        tracing::error!(panic = msg, "auth cleanup tick PANICKED — task will retry next interval");
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
            // NOTE: `/users/me/password` (change_password) lives in the
            // desktop tunnel_auth crate now — only the desktop feature
            // (Remote Access password-auth gate) needs it.
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
