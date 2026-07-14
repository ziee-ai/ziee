// Auth module - JWT-based authentication
pub mod cookie;
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
pub mod session_settings;
pub mod types;

// Re-exports
pub use jwt::JwtService;
// Suppress unused-import false positive: the re-export IS needed for `pub use modules::auth::hash_password` in lib.rs.
#[allow(unused_imports)]
pub use password::hash_password;
pub use repository::AuthRepository;
pub use routes::{auth_admin_routes, auth_routes};
pub use session_settings::SessionSettingsRepository;
pub use types::AuthResponse;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use once_cell::sync::OnceCell;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Set at module init from `config.server.trust_forwarded_headers`.
/// When false, the OAuth-authorize handler derives redirect_uri from
/// the HOST header only — defending self-hosted-direct deployments
/// against attacker-supplied X-Forwarded-Host headers.
static TRUST_FORWARDED_HEADERS: OnceCell<bool> = OnceCell::new();

/// Returns true if the deployment configured a trusted reverse proxy
/// in front of the server. Handlers use this to decide whether to
/// honor X-Forwarded-Host / X-Forwarded-Proto. Defaults to `false`
/// (the safer self-hosted-direct posture) when init() hasn't run
/// (e.g. in unit tests that bypass the module loader).
pub fn trust_forwarded_headers() -> bool {
    TRUST_FORWARDED_HEADERS.get().copied().unwrap_or(false)
}

/// Operator-configured public origin to root OAuth `redirect_uri`s at,
/// cached at module init from `code_sandbox.public_base_url`
/// (ZIEE_PUBLIC_FILE_ORIGIN) — but ONLY when it declares an `https://`
/// origin (see `https_public_origin`). `None` until init() runs.
static CONFIGURED_PUBLIC_ORIGIN: OnceCell<Option<String>> = OnceCell::new();

/// The operator-configured https public origin (no trailing slash) that
/// OAuth `redirect_uri`s should be rooted at, or `None` to derive the
/// origin from request headers. Behind an HTTPS edge that terminates TLS
/// and forwards plain HTTP to this container, the header-derived scheme is
/// `http`, producing `http://` redirect_uris that Google rejects; a
/// configured https origin fixes that deterministically. Safe against the
/// F-07 header-spoofing class because the value is operator-controlled, not
/// request-derived.
pub fn configured_public_origin() -> Option<String> {
    CONFIGURED_PUBLIC_ORIGIN.get().cloned().flatten()
}

/// Return the trimmed origin (no trailing slash) IFF `raw` is a non-empty
/// `https://` URL; otherwise `None`. An http / loopback value — e.g. the
/// LOCAL default `http://172.21.0.1:8080` that `code_sandbox.public_base_url`
/// carries for host-gateway file fetches — returns `None`, so local dev keeps
/// deriving the redirect_uri from request headers and is unaffected.
pub(crate) fn https_public_origin(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    // Case-insensitive scheme check; must be EXACTLY the https scheme, not
    // merely a string that contains "https".
    if s.is_empty() || !s.to_ascii_lowercase().starts_with("https://") {
        return None;
    }
    Some(s.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod public_origin_tests {
    use super::https_public_origin;

    #[test]
    fn only_https_origins_are_used() {
        // Deploy: an https public origin is adopted (trailing slash trimmed).
        assert_eq!(
            https_public_origin(Some("https://biognosia.tinnguyen-lab.com")).as_deref(),
            Some("https://biognosia.tinnguyen-lab.com")
        );
        assert_eq!(
            https_public_origin(Some("https://x.example/")).as_deref(),
            Some("https://x.example")
        );
        assert_eq!(
            https_public_origin(Some("HTTPS://x.example")).as_deref(),
            Some("HTTPS://x.example")
        );
        // Local default + empties → None → fall back to header derivation.
        assert_eq!(https_public_origin(Some("http://172.21.0.1:8080")), None);
        assert_eq!(https_public_origin(Some("")), None);
        assert_eq!(https_public_origin(Some("   ")), None);
        assert_eq!(https_public_origin(None), None);
    }
}

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
        let _ = TRUST_FORWARDED_HEADERS.set(ctx.config.server.trust_forwarded_headers);
        // Cache the operator-configured https public origin (if any) the same
        // way, so the free-function OAuth-authorize handler can root
        // redirect_uris at it without threading Arc<Config> through Axum.
        let _ = CONFIGURED_PUBLIC_ORIGIN.set(https_public_origin(
            ctx.config
                .code_sandbox
                .as_ref()
                .and_then(|cs| cs.public_base_url.as_deref()),
        ));

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
                let repo = session_settings::SessionSettingsRepository::new((*pool).clone());
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
