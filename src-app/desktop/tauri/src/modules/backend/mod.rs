//! Backend Module
//!
//! Manages embedded backend server lifecycle

pub mod commands;
mod handlers;
mod routes;
mod state;

#[cfg(not(debug_assertions))]
mod static_files;

pub use state::BackendState;

use crate::module_api::DesktopModule;
use anyhow::Result;
use axum::{body::Body, http::Request, response::Response};
use std::sync::{Arc, OnceLock};
use tauri::{App, Manager};
use ziee::ApiRouter;

/// Global storage for backend config (set during init, used when starting server)
static BACKEND_CONFIG: OnceLock<ziee::Config> = OnceLock::new();
static BACKEND_STATE: OnceLock<BackendState> = OnceLock::new();
static JWT_SERVICE: OnceLock<Arc<ziee::JwtService>> = OnceLock::new();

/// Get the JWT service (for Tauri commands)
pub fn get_jwt_service() -> Option<&'static Arc<ziee::JwtService>> {
    JWT_SERVICE.get()
}

pub struct BackendModule {
    config_file: Option<String>,
}

impl BackendModule {
    pub fn new(config_file: Option<String>) -> Self {
        Self { config_file }
    }
}

impl DesktopModule for BackendModule {
    fn name(&self) -> &'static str {
        "backend"
    }

    fn description(&self) -> &'static str {
        "Embedded backend server lifecycle management"
    }

    fn init(&mut self, app: &mut App) -> Result<()> {
        tracing::info!("Initializing backend module...");

        // Load config from file or generate default
        let mut config = if let Some(ref config_path) = self.config_file {
            tracing::info!("Loading config from file: {}", config_path);
            ziee::Config::load_from(Some(config_path.clone()))
                .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?
        } else {
            // Get app data directory for backend configuration
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?;

            tracing::info!("App data directory: {:?}", data_dir);

            // Create config directory
            std::fs::create_dir_all(&data_dir)?;

            // Find available port for backend
            let port = ziee::find_available_port(8080, 8180)
                .ok_or_else(|| anyhow::anyhow!("No available ports in range 8080-8180"))?;

            tracing::info!("Selected port {} for backend server", port);

            // Create backend configuration
            create_desktop_config(&data_dir, port)?
        };

        // The desktop app has its own auto-updater (tauri-plugin-updater), so
        // the embedded server's self-update notification must stay OFF — no
        // GitHub polling, no admin update banner. (The matching web UI module
        // is dropped from the desktop bundle via CORE_MODULE_BLOCKLIST.)
        config.update_check.enabled = false;

        // Desktop runs an embedded server reachable from three
        // origins: (a) `tauri://localhost` — the Tauri webview's
        // custom protocol, (b) `http://localhost:<port>` /
        // `http://127.0.0.1:<port>` — the dev Vite server + same-port
        // self-fetches, (c) the public ngrok tunnel domain when a
        // tunnel is up. Set an explicit allowlist instead of falling
        // through to the "permissive default" branch (which gives
        // Any origin Any method Any header — readable by any tab the
        // user happens to have open).
        //
        // The ngrok host is added at tunnel-start time; static
        // origins are below.
        let port = config.server.port;
        config.server.cors = Some(ziee::CorsConfig {
            allow_origins: vec![
                "tauri://localhost".to_string(),
                "http://tauri.localhost".to_string(),
                format!("http://127.0.0.1:{}", port),
                format!("http://localhost:{}", port),
                "http://localhost:1420".to_string(),
            ],
            allow_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "PATCH".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
            ],
            allow_headers: vec![
                "Authorization".to_string(),
                "Content-Type".to_string(),
                "Accept".to_string(),
                "Origin".to_string(),
                // Self-echo suppression for realtime sync: every
                // mutation from the SPA carries the SyncClient's
                // connection id, which the server echoes back so the
                // originating tab is skipped in fan-out. Without
                // this entry the browser preflight rejects every
                // mutating request as soon as the SyncClient is
                // connected — every form Save / Delete / Toggle in
                // the desktop UI fails with a CORS error before
                // ever reaching the handler.
                "X-Sync-Connection-Id".to_string(),
            ],
        });

        // Desktop flips every opt-in feature ON by default. The
        // single-admin device should NOT have to dig through admin
        // settings to turn things on that have a clear "use me"
        // value (Memory, Code Sandbox). On the server, these stay
        // opt-in for the operator to weigh deployment trade-offs.
        //
        // Code Sandbox: force the config flag to `true`. The module
        // boot probes host deps (bwrap on Linux, libkrun.dylib on
        // macOS, WSL2 on Windows) and gracefully no-ops the runtime
        // path on probe failure — so a Mac user without libkrun
        // installed isn't broken; the sandbox just stays
        // un-registered. Memory (DB-level) is flipped on inside
        // the post-migration hook below.
        let sandbox_cfg = config.code_sandbox.get_or_insert_with(Default::default);
        sandbox_cfg.enabled = true;

        // BioMCP: force the config flag on. The module self-disables if
        // the embedded biomcp binary is a build stub, and the managed
        // sidecar surfaces a clear error when offline — so a desktop user
        // without connectivity isn't broken; bio tools just stay
        // unavailable until the network returns. Connected-only by nature.
        let bio_cfg = config.bio_mcp.get_or_insert_with(Default::default);
        bio_cfg.enabled = true;

        tracing::info!("Backend will use port {}", port);

        // Publish the bound port into the remote_access state so the
        // ngrok tunnel forwards to the actual upstream (not the
        // fallback 8080). Must happen BEFORE `start_backend_server`
        // spawns the auto-start hook.
        crate::modules::remote_access::set_local_server_port(port);

        // Create backend state
        let state = BackendState::new(port);

        // Store state in app for Tauri command access
        app.manage(state.clone());

        // Store config and state globally for server start
        BACKEND_CONFIG
            .set(config)
            .map_err(|_| anyhow::anyhow!("Backend config already set"))?;
        BACKEND_STATE
            .set(state)
            .map_err(|_| anyhow::anyhow!("Backend state already set"))?;

        tracing::info!("Backend module initialized (server will start after route collection)");
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        tracing::info!("Registering backend API routes");
        router.merge(routes::backend_api_routes())
    }

    fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down backend module...");

        // Cleanup backend resources
        tauri::async_runtime::block_on(async {
            ziee::cleanup_server().await;
        });

        Ok(())
    }
}

use crate::modules::auth::ensure_desktop_admin;
use crate::modules::llm_provider::AutoAssignProviderHandler;
use crate::modules::mcp::{
    backfill_system_mcp_assignments, AutoAssignMcpServerHandler,
};

// =====================================================
// Backend Server Startup
// =====================================================

/// Start the backend server with collected routes from all modules
///
/// This should be called from lib.rs after all modules have been initialized
/// and routes have been collected.
pub fn start_backend_server(desktop_routes: ApiRouter, app_handle: tauri::AppHandle) {
    let config = BACKEND_CONFIG
        .get()
        .expect("Backend config not initialized - call init() first")
        .clone();
    let state = BACKEND_STATE
        .get()
        .expect("Backend state not initialized - call init() first")
        .clone();

    tracing::info!("Starting backend server with desktop routes...");

    // Create desktop-specific event handlers
    let handlers: Vec<Arc<dyn ziee::EventHandler>> = vec![
        AutoAssignProviderHandler::new(),
        // Mirror the LLM provider auto-assign for system MCP servers:
        // every new system server lands in every user group so the
        // single admin sees it in chat without a manual assignment.
        AutoAssignMcpServerHandler::new(),
    ];

    // Clone config so the closure can build a CORS layer that
    // matches the server's own — `start_server_with_routes` takes
    // ownership of `config`, so we need our own copy first.
    let cors_config = config.clone();

    tauri::async_runtime::spawn(async move {
        match ziee::start_server_with_routes(
            config,
            move |router, jwt| {
            // Store JWT service for Tauri command access
            let _ = JWT_SERVICE.set(jwt.clone());
            tracing::info!("JWT service stored for Tauri commands");

            // Initialize desktop repositories with server's pool
            // Repos is available here because start_server_with_routes
            // initializes it before calling this closure
            let pool = ziee::Repos.pool().clone();
            crate::core::init_desktop_repositories(pool);
            tracing::info!("Desktop repositories initialized with server pool");

            // Re-apply CORS + Extension(jwt) to the merged desktop
            // routes. `setup.app` already has these layers but axum's
            // `.merge()` does NOT propagate parent layers onto
            // merged routes. Without these:
            //   - Browser preflight OPTIONS → 405 (no CORS layer).
            //   - Authenticated requests → 500 "JWT service not
            //     configured" (RequirePermissions can't find the
            //     Arc<JwtService> extension).
            let desktop_cors = ziee::create_cors_layer(&cors_config);
            let router = router.merge(
                desktop_routes
                    .layer(axum::Extension(jwt.clone()))
                    .layer(desktop_cors),
            );

            // Development: proxy non-API requests to Vite dev server
            // This enables Playwright testing by serving both API and frontend from same origin
            #[cfg(debug_assertions)]
            let router = {
                tracing::info!("Development mode: enabling Vite proxy fallback");
                router.fallback(proxy_to_vite)
            };

            // Production: serve embedded static files
            #[cfg(not(debug_assertions))]
            let router = {
                tracing::info!("Production mode: serving embedded static files");
                router.fallback(static_files::serve_embedded_files)
            };

            router
            },
            handlers,
        )
        .await
        {
            Ok(addr) => {
                tracing::info!("Backend server started successfully on {}", addr);

                // Run desktop-specific migrations
                if let Err(e) = run_desktop_migrations().await {
                    // Surface as ERROR (not WARN) with an actionable
                    // message — the server stays up and /api/health
                    // continues to respond, but any handler that
                    // touches desktop-only tables (remote_access /
                    // magic_link / desktop_settings) will return 500
                    // until the operator intervenes. Cross-process
                    // signaling to the UI bootstrap spinner is
                    // tracked separately (M21 partial).
                    tracing::error!(
                        error = %e,
                        "Failed to run desktop migrations — the app may be unusable. \
                         Check the DB connection and consider resetting the data directory."
                    );
                }

                // Register the host-folder mount provider against the generic
                // sandbox seam (desktop-only feature #3, Part B). Safe after
                // migrations: the provider reads the host_mount_* tables lazily
                // at execute_command time.
                crate::modules::host_mount::register_provider();

                // Ensure admin exists (create on first run)
                if let Err(e) = ensure_desktop_admin().await {
                    tracing::error!("Failed to ensure desktop admin: {}", e);
                }

                // Idempotent backfill: every system MCP server gets a
                // row in `user_group_mcp_servers` for every group.
                // Runs every boot to catch built-in registrations
                // (memory MCP) whose insert-if-absent path may not
                // emit `SystemServerCreated`. See the function doc
                // for the full rationale.
                if let Err(e) = backfill_system_mcp_assignments().await {
                    tracing::error!(
                        error = %e,
                        "backfill_system_mcp_assignments failed — system MCP servers may not be visible to the admin until manually assigned"
                    );
                }

                // Flip the singleton `memory_admin_settings.enabled`
                // to TRUE on every boot. Defaults to FALSE in the
                // migration (server's opt-in posture) — desktop
                // single-admins shouldn't have to discover an admin
                // toggle to start using Memory. Idempotent.
                if let Err(e) = enable_memory_admin_default().await {
                    tracing::error!(
                        error = %e,
                        "enable_memory_admin_default failed — Memory will appear disabled until the admin flips it"
                    );
                }

                state.set_ready(true);

                // Create window now that server is ready
                create_main_window(&app_handle);
            }
            Err(e) => {
                tracing::error!("Failed to start backend server: {}", e);
            }
        }
    });
}

/// Desktop's "single-admin device" posture: flip
/// `memory_admin_settings.enabled` to TRUE on every boot.
///
/// The migration default is FALSE because the server treats every
/// opt-in capability as a deployment decision the operator must
/// make. On desktop there's only one operator (the auto-provisioned
/// admin) and there's no good reason to make them discover an admin
/// toggle before using Memory.
///
/// Idempotent — re-running flips an already-TRUE value to TRUE.
/// If the row was already TRUE (operator already enabled it via
/// the admin UI), this is a no-op. If they explicitly DISABLED it
/// and then restarted the app, this WILL re-enable on the next boot
/// — that's intentional for the default-on posture; if a user
/// genuinely wants Memory off forever on their desktop, they can
/// override via a config flag in the future.
async fn enable_memory_admin_default() -> Result<()> {
    let pool = ziee::Repos.pool();
    sqlx::query("UPDATE memory_admin_settings SET enabled = TRUE WHERE id = 1")
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to enable memory: {}", e))?;
    tracing::info!("Memory admin settings enabled (desktop default)");
    Ok(())
}

/// Run desktop-specific database migrations
async fn run_desktop_migrations() -> Result<()> {
    tracing::info!("Running desktop migrations...");

    let pool = ziee::Repos.pool();

    // Use set_ignore_missing(true) because server migrations are tracked
    // in the same _sqlx_migrations table but are not in our migrations folder
    sqlx::migrate!("./migrations")
        .set_ignore_missing(true)
        .run(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Desktop migration failed: {}", e))?;

    tracing::info!("Desktop migrations completed successfully");
    Ok(())
}


/// Proxy handler to forward non-API requests to a Vite dev server.
///
/// In production this path is unused — the static-file fallback in
/// `static_files.rs` serves the embedded WEB bundle. In dev we proxy:
/// if the incoming request's `Host` header is `127.0.0.1` or
/// `localhost` (the Tauri webview's own origin) we forward to the
/// DESKTOP UI Vite (default `localhost:1420`). Anything else (a
/// phone hitting the dev server via the Remote Access tunnel) is
/// forwarded to the WEB UI Vite (default `localhost:5173`, the
/// standard Vite port for the `src-app/ui` workspace). This mirrors
/// the production routing — tunnel users always see the web bundle.
#[cfg(debug_assertions)]
async fn proxy_to_vite(req: Request<Body>) -> Result<Response<Body>, axum::http::StatusCode> {
    // Single Vite — the desktop bundle is served to BOTH the Tauri
    // webview AND to phones reaching the backend over the ngrok
    // tunnel. The remote-access feature lives entirely in the desktop
    // UI workspace; no separate web bundle is involved.
    let vite_url =
        std::env::var("VITE_DEV_URL").unwrap_or_else(|_| "http://localhost:1420".to_string());

    let uri = req.uri();
    let path_and_query = uri
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or(uri.path());

    let proxy_url = format!("{}{}", vite_url, path_and_query);

    match reqwest::get(&proxy_url).await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .bytes()
                .await
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

            let mut builder = Response::builder().status(status);
            for (key, value) in headers.iter() {
                builder = builder.header(key.as_str(), value);
            }
            builder
                .body(Body::from(body))
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(e) => {
            // Helpful HTML body — without this, the browser shows a
            // blank "Bad Gateway" with no hint that the dev Vite
            // process is down. Common during dev when Vite is
            // restarting.
            tracing::warn!(target = %vite_url, error = %e, "proxy_to_vite: upstream unreachable");
            let html = format!(
                r#"<!DOCTYPE html><html><head><title>Vite dev server unreachable</title></head>
<body style="font-family:system-ui;padding:2rem;max-width:48rem;margin:auto">
<h2>Vite dev server unreachable</h2>
<p>The backend tried to proxy this request to <code>{vite_url}</code> but the connection failed:</p>
<pre style="background:#f4f4f4;padding:1rem;overflow:auto">{e}</pre>
<p>Start the desktop UI dev server with <code>npm run dev --workspace=@ziee/desktop-ui</code>, then reload.</p>
</body></html>"#,
                vite_url = vite_url,
                e = e,
            );
            Response::builder()
                .status(axum::http::StatusCode::BAD_GATEWAY)
                .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(html))
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Build the in-memory `ziee::Config` the embedded server boots from.
///
/// Construct as a YAML template + interpolate the per-launch values
/// (port, app_data_dir, random jwt secret). This matches the
/// canonical shape `Config` deserializes from (see
/// `src-app/server/config/dev.yaml` for the reference). The previous
/// implementation built a `HashMap` with ad-hoc top-level keys
/// (`database:` instead of `postgresql:`, `embedded: "true"` instead
/// of `use_embedded: true`, `access_token_expiry` instead of
/// `access_token_expiry_hours`, and no `issuer`/`audience` at all)
/// — serde silently dropped the typo'd keys and only worked because
/// `Config::resolve_paths` filled in the postgres dirs from
/// `app.data_dir` anyway.
///
/// Per-launch overrides:
///   - `app.data_dir` → Tauri's `app_data_dir()`
///   - `server.port` → first free port in 8080..8180
///   - `jwt.secret` → fresh 32-byte hex (regenerated every cold
///     start; persisted browser tokens from previous launches don't
///     validate, so `desktop-base` always re-runs `auto_login`).
///   - `secrets.storage_key` → 64-hex persistent key (created on
///     first boot under `app_data_dir/storage_key`, read back on
///     every subsequent boot). MUST persist — losing it makes the
///     ngrok auth token in `remote_access_settings` (and any other
///     pgcrypto-encrypted column) permanently undecryptable.
///
/// Everything else (postgres embedded version, install/data dirs via
/// resolve_paths, pool sizes, logging) uses the same defaults the
/// development config uses.
fn create_desktop_config(
    data_dir: &std::path::Path,
    port: u16,
) -> Result<ziee::Config> {
    use rand::Rng;
    let secret: String = rand::rng()
        .random_iter::<u8>()
        .take(32)
        .map(|b| format!("{:02x}", b))
        .collect();

    // Persistent at-rest encryption key. Created once on first boot
    // and reused forever after. Unlike the JWT secret (which is fine
    // to regenerate because losing it just invalidates outstanding
    // sessions), losing this key orphans every encrypted column in
    // the DB (the ngrok auth token, any future encrypted secret) —
    // they can never be decrypted back.
    let storage_key = ensure_persistent_storage_key(data_dir)?;

    // data_dir is already validated to exist; serialize the path with
    // YAML-safe quoting (the path may contain spaces on macOS:
    // "Application Support"). Single quotes are the simplest safe
    // YAML scalar — '' escapes single quotes inside.
    let data_dir_yaml = data_dir
        .to_string_lossy()
        .replace('\'', "''");
    let storage_key_yaml = storage_key.replace('\'', "''");

    let yaml = format!(
        r#"app:
  data_dir: '{data_dir_yaml}'

postgresql:
  use_embedded: true
  embedded:
    version: "18.3.0"
    port: 0
    bind_address: "127.0.0.1"
    username: "postgres"
    password: "password"
    database: "postgres"
    timezone: "UTC"
    log_timezone: "UTC"
    logging:
      collector: true
      directory: "log"
      filename: "postgresql-%Y-%m-%d_%H%M%S.log"
      statement: "ddl"
  pool:
    max_connections: 10
    min_connections: 1
    acquire_timeout_secs: 5
    idle_timeout_secs: 30
    max_lifetime_secs: 300

server:
  host: "127.0.0.1"
  port: {port}
  api_prefix: "/api"
  # NO global rate limit. The desktop embeds this server and the
  # Tauri webview makes bursts of parallel requests during normal
  # use (settings polls, chat streams, multi-fetch on page mount);
  # any cap low enough to slow down a brute-forcer would also 429
  # legitimate UI traffic. The tunneled-auth endpoints are
  # protected by other means:
  #   - magic-link/exchange: 256-bit URL-safe tokens with 5-min TTL,
  #     unbruteable at any rate.
  #   - login-password-only: bcrypt (default cost) naturally paces
  #     to ~10 attempts/sec regardless; a weak admin password is the
  #     real risk, not raw request rate.
  # If we ever surface a NEW unauth endpoint that lacks intrinsic
  # cost, add a per-route governor on THAT route specifically rather
  # than reintroducing a global cap.

jwt:
  secret: "{secret}"
  issuer: "ziee"
  audience: "ziee-api"
  access_token_expiry_hours: 1
  refresh_token_expiry_days: 7

secrets:
  storage_key: '{storage_key_yaml}'

logging:
  level: "info"
  format: "compact"
"#
    );

    let mut config: ziee::Config = serde_yaml::from_str(&yaml)?;

    // CRITICAL: `Config::load_from` normally calls `resolve_paths()` at
    // the end (it's what fills optional path fields like
    // `postgresql.embedded.installation_dir` and
    // `caches.llm_engines_dir` from `app.data_dir`). We bypassed
    // `load_from` by parsing the YAML directly, so we have to do it
    // ourselves — otherwise the first `installation_dir` /
    // `llm_engines_dir` accessor panics with
    // "<field> filled by Config::resolve_paths".
    config.resolve_paths();

    // The desktop has its own auto-updater; the embedded server must never run
    // the server self-update notification. (init() also force-disables this for
    // file-loaded configs — this keeps the default-built config correct too.)
    config.update_check.enabled = false;

    Ok(config)
}

/// Create-or-read a persistent storage key under
/// `<data_dir>/storage_key`. Returns the hex key as a String.
///
/// 32 bytes of OS randomness, hex-encoded to a 64-char string. File
/// permissions are restricted to 0600 on Unix (Windows doesn't have
/// the equivalent but the path is inside the user-owned AppData
/// directory).
///
/// The key is exposed in the in-memory config so the pgcrypto
/// encrypt/decrypt helpers can find it; it never leaves the
/// machine.
pub fn ensure_persistent_storage_key(data_dir: &std::path::Path) -> Result<String> {
    use rand::Rng;
    let key_path = data_dir.join("storage_key");

    if key_path.exists() {
        let existing = std::fs::read_to_string(&key_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", key_path.display(), e))?;
        let trimmed = existing.trim().to_string();
        if trimmed.len() >= 32 {
            return Ok(trimmed);
        }
        // File exists but content is too short — overwrite with a
        // fresh key. This is the same "log + repair" pattern the
        // memory_admin reaper uses for malformed rows.
        tracing::warn!(
            "storage_key at {} is too short ({} chars); regenerating",
            key_path.display(),
            trimmed.len()
        );
    }

    let key: String = rand::rng()
        .random_iter::<u8>()
        .take(32)
        .map(|b| format!("{:02x}", b))
        .collect();

    std::fs::write(&key_path, &key)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", key_path.display(), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = std::fs::set_permissions(&key_path, perms) {
            tracing::warn!(
                "Failed to set 0600 perms on {}: {} (key written, but file is world-readable)",
                key_path.display(),
                e
            );
        }
    }

    tracing::info!("Generated persistent storage_key at {}", key_path.display());
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ensure_persistent_storage_key_creates_64_hex_chars_on_first_call() {
        let tmp = TempDir::new().unwrap();
        let key = ensure_persistent_storage_key(tmp.path()).expect("create key");
        assert_eq!(key.len(), 64, "key should be 64 hex chars (32 bytes)");
        assert!(
            key.chars().all(|c| c.is_ascii_hexdigit()),
            "key should be all hex"
        );
    }

    #[test]
    fn ensure_persistent_storage_key_returns_same_key_on_second_call() {
        let tmp = TempDir::new().unwrap();
        let first = ensure_persistent_storage_key(tmp.path()).expect("create");
        let second = ensure_persistent_storage_key(tmp.path()).expect("read back");
        assert_eq!(first, second, "key MUST persist across calls");
    }

    #[test]
    fn ensure_persistent_storage_key_persists_to_disk() {
        let tmp = TempDir::new().unwrap();
        let key = ensure_persistent_storage_key(tmp.path()).expect("create");
        let on_disk = std::fs::read_to_string(tmp.path().join("storage_key"))
            .expect("file should exist on disk");
        assert_eq!(on_disk.trim(), key);
    }

    #[test]
    fn ensure_persistent_storage_key_regenerates_when_existing_is_too_short() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("storage_key"), "too-short").unwrap();
        let key = ensure_persistent_storage_key(tmp.path()).expect("regen");
        assert!(key.len() >= 32, "regenerated key must be ≥32 chars");
        assert_ne!(key, "too-short");
    }

    /// REGRESSION GUARD — this is the test that would have caught
    /// the "Cannot persist ngrok auth token: secrets.storage_key is
    /// not configured" error. The Tauri desktop config MUST set
    /// `secrets.storage_key`, otherwise the remote_access PUT
    /// /settings handler returns 500 the moment the user clicks
    /// Save on the ngrok token form. Mock tests + the Tier 8
    /// real-ngrok test all run with the test harness's storage_key
    /// set, sidestepping this code path entirely.
    #[test]
    fn create_desktop_config_sets_secrets_storage_key() {
        let tmp = TempDir::new().unwrap();
        let config = create_desktop_config(tmp.path(), 8080)
            .expect("desktop config should build");
        let storage_key = config
            .secrets
            .as_ref()
            .and_then(|s| s.storage_key.as_deref())
            .expect(
                "create_desktop_config MUST set secrets.storage_key — otherwise the \
                 Remote Access ngrok token can't be encrypted at rest. See \
                 server/src/modules/remote_access/repository.rs::update_settings",
            );
        assert!(
            storage_key.len() >= 32,
            "storage_key must be at least 32 chars (pgcrypto requirement); got {}",
            storage_key.len()
        );
    }

    #[test]
    fn create_desktop_config_disables_server_update_check() {
        // The desktop has its own auto-updater; the embedded server must NOT run
        // the server self-update notification (no GitHub poll, no admin banner).
        let tmp = TempDir::new().unwrap();
        let config = create_desktop_config(tmp.path(), 8080)
            .expect("desktop config should build");
        assert!(
            !config.update_check.enabled,
            "embedded desktop server MUST have update_check disabled — the desktop \
             uses tauri-plugin-updater. See modules/backend/mod.rs + the web \
             server-update CORE_MODULE_BLOCKLIST entry."
        );
    }

    #[test]
    fn create_desktop_config_returns_same_storage_key_across_calls() {
        let tmp = TempDir::new().unwrap();
        let c1 = create_desktop_config(tmp.path(), 8080).unwrap();
        let c2 = create_desktop_config(tmp.path(), 8080).unwrap();
        let k1 = c1.secrets.as_ref().unwrap().storage_key.clone().unwrap();
        let k2 = c2.secrets.as_ref().unwrap().storage_key.clone().unwrap();
        assert_eq!(k1, k2, "storage_key MUST persist across launches");
    }
}

/// Create the main window with platform-specific customizations
fn create_main_window(app_handle: &tauri::AppHandle) {
    tracing::info!("Creating main window...");

    // macOS: no native decorations initially (overlay titlebar set below).
    #[cfg(target_os = "macos")]
    let mut main_window_builder = tauri::webview::WebviewWindowBuilder::new(
        app_handle,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("")
    .inner_size(1200.0, 800.0)
    .min_inner_size(400.0, 600.0)
    .resizable(true)
    .fullscreen(false)
    .decorations(false)
    .center()
    .effects(tauri::utils::config::WindowEffectsConfig {
        effects: vec![
            tauri::window::Effect::Mica,
            tauri::window::Effect::Acrylic,
            tauri::window::Effect::Blur,
        ],
        state: Some(tauri::window::EffectState::Active),
        radius: Some(8.0),
        color: None,
    });

    // Windows + Linux: native decorations.
    //  - Windows then overlays decorum's custom titlebar (preserves Aero
    //    snapping, snap layouts that a pure HTML titlebar can't replicate).
    //  - Linux relies entirely on the WM's server-side decorations
    //    (xfwm4 / Mutter / KWin) for border + shadow + close/min/max
    //    trio. XFCE / GNOME / KDE all default to right-aligned buttons,
    //    which matches the project's other platforms.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let main_window_builder = tauri::webview::WebviewWindowBuilder::new(
        app_handle,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("")
    .inner_size(1200.0, 800.0)
    // Match macOS's `min_inner_size(400, 600)` — the previous 800-wide
    // floor blocked the sidebar's xs-mode (drawer / overlay) layout
    // path from ever rendering on Windows. The responsive UI already
    // handles widths down to 400.
    .min_inner_size(400.0, 600.0)
    .resizable(true)
    .fullscreen(false)
    .decorations(true)
    .center()
    .effects(tauri::utils::config::WindowEffectsConfig {
        effects: vec![
            tauri::window::Effect::Mica,
            tauri::window::Effect::Acrylic,
            tauri::window::Effect::Blur,
        ],
        state: Some(tauri::window::EffectState::Active),
        radius: Some(8.0),
        color: None,
    });

    // macOS: overlay titlebar with native traffic light position (no glitch on resize)
    // x=20 matches the standard inset Apple uses in Notes / Finder /
    // Mail; the earlier x=12 sat too close to the window edge and
    // read as "tighter than other macOS apps".
    #[cfg(target_os = "macos")]
    {
        main_window_builder = main_window_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .decorations(true)
            .traffic_light_position(tauri::LogicalPosition::new(20.0, 22.0));
    }

    main_window_builder.build().unwrap();

    // Post-build: Windows overlay
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        use tauri_plugin_decorum::WebviewWindowExt;
        let main_window = app_handle.get_webview_window("main").unwrap();
        main_window.create_overlay_titlebar().unwrap();
    }

    tracing::info!("Main window created successfully");
}
