//! Built-in MCP server bridging open Microsoft Office documents — **desktop-only**.
//!
//! Lives in the desktop tauri crate (not the server crate): Office automation is
//! only possible where the embedded server runs on the user's own machine. The
//! server core stays generic — it exposes runtime registration seams
//! (`ziee::chat_extension::{register_chat_extension, register_auto_attach_builtin}`,
//! mirroring `code_sandbox::register_sandbox_mount_provider`), and this module
//! registers against them at boot. A standalone/remote-web `ziee` server links
//! none of this.
//!
//! Registers `office_bridge.ziee.internal` in `mcp_servers` (`is_built_in=true`,
//! `transport_type='http'`) pointing at a loopback URL, serves JSON-RPC at
//! `/api/office-bridge/mcp`, spawns the HTTPS+WSS add-in bridge listener + the
//! open/close document watch loop. Structure mirrors the `host_mount` desktop
//! module (a `DesktopModule` for its REST routes + a post-server-start
//! `register_office_bridge` hook once `ziee::Repos` exists).

use anyhow::Result;
use tauri::App;
use uuid::Uuid;
use ziee::ApiRouter;

use crate::module_api::DesktopModule;

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

/// Deterministic UUID for the built-in office_bridge MCP server row
/// (stable across deployments; mirrors `web_search_server_id`).
pub fn office_bridge_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"office_bridge.ziee.internal")
}

pub struct OfficeBridgeModule;

impl OfficeBridgeModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OfficeBridgeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopModule for OfficeBridgeModule {
    fn name(&self) -> &'static str {
        "office_bridge"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server bridging open Microsoft Office documents (desktop-only)."
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        // No app-level state. The MCP row upsert, bridge listener, watcher, and
        // chat-extension/auto-attach registration happen in `register_office_bridge`
        // once the embedded server has initialized `ziee::Repos` (see backend/mod.rs).
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::office_bridge_router())
    }
}

/// Post-server-start registration (mirrors `host_mount::register_provider`).
///
/// MUST be called AFTER the embedded server has initialized `ziee::Repos` (i.e.
/// from inside / after `start_server_with_routes`), NOT from `DesktopModule::init`
/// which runs before the pool exists. Call exactly once at boot.
/// Config kill-switch + host probe — the two gates shared by both registration
/// halves. Cheap + idempotent (`probe()` is a pure sync read), so it is safe to
/// call once from the pre-server-start static half and again from the
/// post-server-start runtime half.
fn office_bridge_enabled(config: &ziee::Config) -> bool {
    // FIRST gate — deploy-level kill switch, ON by default (an absent
    // `office_bridge:` config section means enabled). Operators opt OUT with
    // `office_bridge: { enabled: false }`.
    let enabled = config
        .office_bridge
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true);
    if !enabled {
        tracing::info!("office_bridge: disabled in config; skipping registration");
        return false;
    }

    // SECOND gate — runtime host probe. `None` means no Office automation backend
    // on this host; skip everything (the rest of the app is unaffected).
    match platform::active().probe() {
        None => {
            tracing::info!(
                "office_bridge: host probe returned None (no Office automation backend on {}); \
                 skipping registration",
                std::env::consts::OS
            );
            false
        }
        Some(caps) => {
            tracing::debug!(
                "office_bridge: host probe OK (desktop={}, office_present={})",
                caps.desktop,
                caps.office_present
            );
            true
        }
    }
}

/// STATIC (pool-free) registration — MUST run BEFORE `start_server_with_routes`
/// builds the chat module, which snapshots the `ExtensionRegistry` at init.
/// Pushing the chat extension / auto-attach entry *after* that snapshot would
/// silently no-op the whole office chat integration (the flag would never be set,
/// so `auto_attach_builtin_ids` would never attach the office server). Registers
/// only the two static seams (mirror `register_sandbox_mount_provider`): the
/// auto-attach entry (behind the chat-extension flag; NOT approval-bypassed —
/// mutating office tools stay behind per-call approval) and the chat extension.
pub fn register_office_bridge_static(config: &ziee::Config) {
    if !office_bridge_enabled(config) {
        return;
    }
    ziee::register_auto_attach_builtin(ziee::AutoAttachEntry {
        flag: chat_extension::ATTACH_FLAG,
        server_id: office_bridge_server_id,
    });
    ziee::chat_extension::register_chat_extension(chat_extension::extension::extension_entry());
}

/// RUNTIME (pool-dependent) registration — runs AFTER the server has started and
/// desktop migrations are applied. Upserts the `mcp_servers` row + spawns the
/// add-in bridge listener + document watcher. The static seams are registered
/// separately (and earlier) by [`register_office_bridge_static`].
pub fn register_office_bridge(config: &ziee::Config) {
    if !office_bridge_enabled(config) {
        return;
    }

    // Pin loopback regardless of the configured server host so the built-in MCP
    // URL can never be redirected to a non-loopback host.
    let host = ziee::code_sandbox::loopback_host(&config.server.host);
    let loopback_url = format!(
        "http://{host}:{port}/api/office-bridge/mcp",
        port = config.server.port,
    );

    let server_id = office_bridge_server_id();
    let pool = ziee::Repos.pool().clone();
    tokio::spawn(async move {
        let repo = repository::OfficeBridgeRepository::new(pool);
        match repo.upsert_builtin_server(server_id, &loopback_url).await {
            Ok(()) => tracing::info!(
                "office_bridge: built-in server {server_id} registered at {loopback_url}"
            ),
            Err(e) => tracing::error!("office_bridge: upsert_builtin_server failed: {e:?}"),
        }
    });

    // Standalone HTTPS + WSS add-in bridge listener. Fire-and-forget: the
    // `axum_server` accept loops run independently of the returned handle.
    tokio::spawn(async move {
        let repo = repository::OfficeBridgeRepository::new(ziee::Repos.pool().clone());
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
        let data_dir = ziee::get_app_data_dir();
        match bridge::server::start(port, data_dir.clone()).await {
            Ok(handle) => tracing::info!(
                "office_bridge: bridge listener started on {} (port {})",
                handle.origin,
                handle.port
            ),
            // The port is occupied (another process — often a second ziee
            // instance). Office caches the sideloaded manifest's URL, so the
            // bridge port must stay STABLE once the add-in has been sideloaded
            // (`last_connected_at` is set by the `[Connect]` flow). Therefore:
            //   - not yet connected → free to auto-migrate to an open port and
            //     persist it; the next `[Connect]` materializes the manifest at
            //     the new port, so nothing is stranded.
            //   - already connected → the sideloaded manifest points at the old
            //     port; migrating would silently break the pane, so surface an
            //     actionable error instead and leave the listener down.
            Err(e) if e.error_code() == bridge::server::PORT_IN_USE_CODE => {
                if settings.last_connected_at.is_some() {
                    tracing::error!(
                        "office_bridge: bridge port {port} is in use, and the add-in was \
                         already sideloaded at that port — NOT migrating (that would strand \
                         the sideloaded manifest). Free port {port}, or change the office_bridge \
                         port in settings and re-run Connect to re-sideload."
                    );
                } else if let Some(free) = bridge::server::find_free_loopback_port() {
                    let repo2 = repository::OfficeBridgeRepository::new(ziee::Repos.pool().clone());
                    if let Err(e) = repo2.update_settings(None, Some(i32::from(free))).await {
                        tracing::error!(
                            "office_bridge: port {port} in use; found free port {free} but \
                             persisting it failed: {e:?}; not starting listener"
                        );
                    } else {
                        match bridge::server::start(free, data_dir).await {
                            Ok(handle) => tracing::warn!(
                                "office_bridge: port {port} was in use; auto-migrated to free \
                                 port {} and persisted it (not yet connected, so the sideloaded \
                                 manifest will use the new port).",
                                handle.port
                            ),
                            Err(e) => tracing::error!(
                                "office_bridge: retry on migrated port {free} failed: {e:?}"
                            ),
                        }
                    }
                } else {
                    tracing::error!(
                        "office_bridge: bridge port {port} is in use and no free loopback port \
                         could be found; not starting listener"
                    );
                }
            }
            Err(e) => tracing::error!("office_bridge: bridge listener failed to start: {e:?}"),
        }
    });

    // Live open/close document watch loop — emits owner-scoped
    // `ziee::SyncEntity::OfficeDocument` events so the frontend panel updates live.
    tokio::spawn(async move {
        let repo = repository::OfficeBridgeRepository::new(ziee::Repos.pool().clone());
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
        watcher::watch_open_documents(platform::active(), user_id, std::future::pending::<()>())
            .await;
    });
}
