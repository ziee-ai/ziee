//! Built-in MCP server bridging open Microsoft Office documents.
//!
//! Registers `office_bridge.ziee.internal` as a row in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`), pointing at a loopback URL on
//! the same axum app, and serves JSON-RPC at `/api/office-bridge/mcp`. The MCP
//! client at `mcp/client/manager.rs` injects the JWT for built-in servers.
//!
//! This is the FIRST increment (ITEM-1/2/3): the module skeleton, permissions,
//! settings surface, and the built-in-server upsert. The bridge listener,
//! COM/`OfficePlatform` seam, MCP tool dispatch, sync, and frontend land in
//! later items. Structure mirrors `web_search/`.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod bridge;
pub mod chat_extension;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod platform;
pub mod repository;
pub mod routes;
pub mod tools;
pub mod watcher;

pub use repository::OfficeBridgeRepository;

/// Deterministic UUID for the built-in office_bridge MCP server row.
/// Stable across deployments (mirrors `web_search_server_id`).
pub fn office_bridge_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"office_bridge.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static OFFICE_BRIDGE_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "office_bridge",
    // After mcp (65) so mcp_servers exists. 97 is the next free order
    // (96=web_search, then 100/102/103).
    order: 97,
    description:
        "Built-in MCP server bridging open Microsoft Office documents (Word/Excel/PowerPoint)",
    constructor: || Box::new(OfficeBridgeModule::new()),
};

pub struct OfficeBridgeModule {
    // Module handle retained for parity with other modules; not read yet.
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl OfficeBridgeModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for OfficeBridgeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for OfficeBridgeModule {
    fn name(&self) -> &'static str {
        "office_bridge"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server bridging open Microsoft Office documents"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // FIRST gate — deploy-level kill switch, ON by default (an absent
        // `office_bridge:` config section means enabled). Operators opt OUT with
        // `office_bridge: { enabled: false }`; an admin cannot re-enable it
        // (distinct from the runtime `office_bridge_settings.enabled` toggle —
        // DEC-12: two independent levels). Mirrors web_search. Config disabled ⇒
        // skip regardless of the host probe below.
        let enabled = ctx
            .config
            .office_bridge
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(true);
        if !enabled {
            tracing::info!("office_bridge: disabled in config; skipping registration");
            return Ok(());
        }

        // SECOND gate — runtime host probe (DEC-3, mirrors code_sandbox's
        // `probe_host`). `platform::active().probe()` is cheap + sync; `None`
        // means "not a supported desktop OS with an Office automation backend"
        // (a headless / Linux server), so we log the reason and skip MCP-row
        // registration + (later items) the bridge listener entirely — the rest
        // of the server boots fine.
        match platform::active().probe() {
            None => {
                tracing::info!(
                    "office_bridge: host probe returned None (no Office \
                     automation backend on {}); skipping registration",
                    std::env::consts::OS
                );
                return Ok(());
            }
            Some(caps) => {
                tracing::info!(
                    "office_bridge: host probe OK (desktop={}, office_present={}); \
                     registering built-in MCP server",
                    caps.desktop,
                    caps.office_present
                );
            }
        }

        // Pin loopback regardless of the configured server host (same helper
        // code_sandbox/web_search use) so the built-in MCP URL can never be
        // redirected to a non-loopback host.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/office-bridge/mcp",
            port = ctx.config.server.port,
        );

        let server_id = office_bridge_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::OfficeBridgeRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &loopback_url).await {
                Ok(()) => tracing::info!(
                    "office_bridge: built-in server {server_id} registered at {loopback_url}"
                ),
                Err(e) => tracing::error!("office_bridge: upsert_builtin_server failed: {e:?}"),
            }
        });

        // Spawn the standalone HTTPS + WSS bridge listener (ITEM-5). Fire-and-
        // forget: the `axum_server` accept loops run independently of the handle
        // returned by `start` (dropping it does NOT stop them), so we log
        // start/failure and never block `init`. The runtime port comes from the
        // settings row (default 44300); a runtime `enabled = false` skips it
        // (DEC-12 — the second, admin-facing kill level below the config gate).
        let bridge_pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::OfficeBridgeRepository::new((*bridge_pool).clone());
            let settings = match repo.get_settings().await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(
                        "office_bridge: reading settings for bridge start failed: {e:?}; \
                         not starting listener"
                    );
                    return;
                }
            };
            if !settings.enabled {
                tracing::info!(
                    "office_bridge: runtime settings.enabled = false; not starting bridge listener"
                );
                return;
            }
            let port = u16::try_from(settings.port).unwrap_or(44300);
            let data_dir = crate::core::get_app_data_dir();
            match bridge::server::start(port, data_dir).await {
                Ok(handle) => tracing::info!(
                    "office_bridge: bridge listener started on {} (port {})",
                    handle.origin,
                    handle.port
                ),
                Err(e) => tracing::error!("office_bridge: bridge listener failed to start: {e:?}"),
            }
        });

        // Spawn the live open/close document watch loop (ITEM-11). Fire-and-
        // forget, non-blocking: it polls the native platform, diffs successive
        // snapshots, and emits owner-scoped `SyncEntity::OfficeDocument` events
        // so the frontend panel updates live. Owner audience (DEC-7) needs the
        // desktop's single interactive user — resolved here (async) and passed
        // into the watcher, which never resolves it itself. If no user exists
        // yet (pre-setup), we skip: nothing to scope events to.
        let watch_pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::OfficeBridgeRepository::new((*watch_pool).clone());
            let user_id = match repo.resolve_primary_user_id().await {
                Ok(Some(uid)) => uid,
                Ok(None) => {
                    tracing::info!(
                        "office_bridge: no active user to scope open/close sync to; \
                         not starting watch loop"
                    );
                    return;
                }
                Err(e) => {
                    tracing::error!(
                        "office_bridge: resolving primary user for watch loop failed: {e:?}; \
                         not starting watch loop"
                    );
                    return;
                }
            };
            // Process-lifetime loop (mirrors the local-runtime reaper): no
            // discrete shutdown signal is threaded through module init, so pass
            // a never-resolving future; the task ends when the process does.
            watcher::watch_open_documents(
                platform::active(),
                user_id,
                std::future::pending::<()>(),
            )
            .await;
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::office_bridge_router())
    }
}
