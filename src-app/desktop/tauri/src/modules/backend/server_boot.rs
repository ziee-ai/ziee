//! `ZieeServerBoot` — ziee's concrete impl of the harness's [`ServerBoot`] seam
//! (Chunk BG-3).
//!
//! This is the app-side embed-server boundary the reusable Tauri shell is built
//! on. It wraps the app's whole non-agnostic server assembly
//! (`ziee::start_server_with_routes` → `setup_server`: `Repos` init,
//! `create_modules`, `ZieeIdentityResolver`, `build_auth_context`, control-mcp
//! catalog) plus the desktop route re-layering (CORS + `Extension(jwt)`), and
//! hands the harness back a [`BootHandle`] carrying `{addr, pool, jwt}`.
//!
//! The harness names ONLY the [`ServerBoot`] trait + [`BootHandle`] — never
//! `ziee::` — so once D-full moves the live Tauri shell (`run`/`run_headless`,
//! the 2 IPC commands, `create_main_window`) into the harness, it drives the
//! embedded server entirely through this seam.

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use ziee::ApiRouter;
use ziee_desktop_harness::boot::{BootHandle, ServerBoot};

/// The app-side `ServerBoot`. Holds the config + the module-system-collected
/// desktop routes + the desktop event handlers, consumed once by [`boot`].
///
/// [`boot`]: ServerBoot::boot
pub struct ZieeServerBoot {
    config: ziee::Config,
    // ApiRouter isn't `Clone`, and `boot(&self)` can't move out of `&self`, so
    // the once-consumed inputs live behind `Mutex<Option<..>>` — `take()`n on the
    // single boot call (the guard is dropped before the first await, so the boot
    // future stays `Send`).
    desktop_routes: Mutex<Option<ApiRouter>>,
    handlers: Mutex<Option<Vec<Arc<dyn ziee::EventHandler>>>>,
}

impl ZieeServerBoot {
    /// Build the seam from the collected desktop routes + handlers.
    pub fn new(
        config: ziee::Config,
        desktop_routes: ApiRouter,
        handlers: Vec<Arc<dyn ziee::EventHandler>>,
    ) -> Self {
        Self {
            config,
            desktop_routes: Mutex::new(Some(desktop_routes)),
            handlers: Mutex::new(Some(handlers)),
        }
    }
}

#[async_trait]
impl ServerBoot for ZieeServerBoot {
    async fn boot(&self) -> anyhow::Result<BootHandle> {
        let config = self.config.clone();
        // Clone config so the closure can build a CORS layer that matches the
        // server's own — `start_server_with_routes` takes ownership of `config`.
        let cors_config = config.clone();
        let desktop_routes = self
            .desktop_routes
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| anyhow::anyhow!("ZieeServerBoot::boot called more than once"))?;
        let handlers = self.handlers.lock().unwrap().take().unwrap_or_default();

        // The route-builder closure runs synchronously inside `setup_server`
        // (before `axum::serve` is spawned), so the JWT service is captured here
        // and read back into the `BootHandle` after the await resolves.
        let jwt_slot: Arc<OnceLock<Arc<ziee::JwtService>>> = Arc::new(OnceLock::new());
        let jwt_capture = jwt_slot.clone();

        let addr = ziee::start_server_with_routes(
            config,
            move |router, jwt| {
                // Capture the JWT service for the returned BootHandle (the
                // harness stashes it for the `auto_login` command).
                let _ = jwt_capture.set(jwt.clone());

                // Initialize desktop repositories with the server's pool. `Repos`
                // is available here because `start_server_with_routes`
                // initializes it before calling this closure.
                let pool = ziee::Repos.pool().clone();
                crate::core::init_desktop_repositories(pool);

                // Re-apply CORS + Extension(jwt) to the merged desktop routes.
                // `setup.app` already has these layers but axum's `.merge()` does
                // NOT propagate parent layers onto merged routes. Without these:
                //   - Browser preflight OPTIONS → 405 (no CORS layer).
                //   - Authenticated requests → 500 "JWT service not configured"
                //     (RequirePermissions can't find the Arc<JwtService> ext).
                let desktop_cors = ziee::create_cors_layer(&cors_config);
                let router = router.merge(
                    desktop_routes
                        .layer(axum::Extension(jwt.clone()))
                        .layer(desktop_cors),
                );

                // Development: proxy non-API requests to the Vite dev server.
                // This enables Playwright testing by serving both API and
                // frontend from the same origin.
                #[cfg(debug_assertions)]
                let router = {
                    tracing::info!("Development mode: enabling Vite proxy fallback");
                    router.fallback(super::proxy_to_vite)
                };

                // Production: serve embedded static files, br/gzip-compressed.
                #[cfg(not(debug_assertions))]
                let router = {
                    tracing::info!("Production mode: serving embedded static files (br/gzip)");
                    router.fallback_service(
                        axum::routing::any(super::static_files::serve_embedded_files)
                            .layer(tower_http::compression::CompressionLayer::new()),
                    )
                };

                router
            },
            handlers,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start backend server: {}", e))?;

        let jwt = jwt_slot
            .get()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("JWT service was not captured during boot"))?;
        let pool = ziee::Repos.pool().clone();

        Ok(BootHandle { addr, pool, jwt })
    }

    async fn shutdown(&self) {
        ziee::cleanup_server().await;
    }
}
