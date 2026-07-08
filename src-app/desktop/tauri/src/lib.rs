//! Ziee Desktop - Library
//!
//! Tauri application with modular desktop features.
//! All functionality (except get_server_port) communicates via HTTP routes.

mod core;
mod module_api;
pub mod modules;
pub mod openapi;

use anyhow::Result;

/// Wire all desktop `#[tauri::command]` functions into a Builder's invoke
/// handler. Exposed so integration tests (`tests/tauri_commands_test.rs`)
/// can register the same commands without needing access to the per-command
/// `__cmd__*` macros, which only resolve inside this crate's scope.
pub fn register_desktop_invoke_handler<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        crate::modules::backend::commands::get_server_port,
        crate::modules::auth::commands::auto_login,
    ])
}

/// Run the desktop server in HEADLESS mode (no Tauri window).
///
/// Used by the TestServer integration-test harness so the
/// remote_access / magic_link / tunnel_auth routes — which live
/// only in the desktop crate — can be exercised by tests that
/// shell out to a binary, identical to how `TestServer` already
/// spawns `ziee` for server-only routes.
///
/// Differences vs `run()`:
///   - No Tauri::Builder / no window / no system tray / no IPC handler.
///   - No desktop module `init()` calls (which expect a Tauri `App`);
///     instead we manually wire just the things the embedded server
///     needs: config, storage_key, repositories, and ROUTE registration.
///   - Blocks forever on `tokio::signal::ctrl_c()` so the parent
///     process (TestServer) can SIGKILL it cleanly.
pub async fn run_headless(config_file: Option<String>) -> Result<()> {
    use ziee::{ApiRouter, EventHandler, JwtService, set_app_data_dir, set_caches_config};

    tracing::info!("Starting Ziee Desktop in HEADLESS mode (no window)");

    // Load config from the file the test harness wrote (mirror of
    // server's main.rs config-load path).
    let config = ziee::Config::load_from(config_file)
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;

    // Initialize globals normally set by `setup_server` in the
    // server binary's main. Same calls the test harness made before,
    // but for the desktop binary path.
    if let Some(app_cfg) = &config.app {
        set_app_data_dir(std::path::PathBuf::from(&app_cfg.data_dir));
    }
    set_caches_config(config.caches.clone());
    ziee::init_storage_key(
        config
            .secrets
            .as_ref()
            .and_then(|s| s.storage_key.clone()),
    );

    // Routes from desktop modules. We collect WITHOUT calling
    // `init()` because the trait signature demands a `tauri::App`
    // and we don't have one in headless mode. Routes self-contain
    // every handler dependency (state is global statics or
    // `ziee::Repos`), so `init()` is unnecessary for the test path.
    let mut desktop_modules = core::create_desktop_modules(None);
    let desktop_routes = {
        let mut router = ApiRouter::new();
        for m in &desktop_modules {
            router = m.register_api_routes(router);
        }
        router
    };

    // CRITICAL ORDER for EXTERNAL postgres (test harness path):
    // run desktop migrations BEFORE `start_server_with_routes`. The
    // server's `run_server` binds the listener + spawns axum::serve
    // into a detached task as soon as it returns — at that point
    // `/api/health` is reachable and the test harness considers the
    // server "ready". If we apply desktop migrations AFTER that,
    // there's a race where tests hit endpoints that query the
    // `remote_access_settings` / `magic_link_tokens` tables BEFORE
    // the migration adds them, and get 500s.
    //
    // SKIP for embedded postgres: the embedded PG binary is started
    // INSIDE `setup_server`'s `initialize_database` call, which
    // happens further down. Connecting up here against the embedded
    // URL would fail with "connection refused". For the embedded
    // path we let `initialize_database` apply server migrations and
    // then `run_desktop_migrations` (called by the GUI path) handle
    // the desktop ones. Headless against embedded is not a
    // production path; the test harness always points at external
    // postgres (the docker-compose test DB on 54322).
    if !config.postgresql.use_embedded {
        use sqlx::postgres::PgPoolOptions;
        eprintln!("[headless] pre-migration block: connecting to {}", config.database_url());
        let setup_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&config.database_url())
            .await
            .map_err(|e| anyhow::anyhow!("Headless pre-migration connect failed: {}", e))?;
        eprintln!("[headless] connected; running server+desktop migrations");

        // Apply server migrations first (so the build DB schema
        // matches). The next `start_server_with_routes` call will
        // re-run them and find them already applied — sqlx::migrate
        // is idempotent.
        let mut server_migrator = sqlx::migrate::Migrator::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../server/migrations"),
        )
        .await
        .map_err(|e| anyhow::anyhow!("Headless server-migrator create failed: {}", e))?;
        server_migrator.set_ignore_missing(true);
        server_migrator
            .run(&setup_pool)
            .await
            .map_err(|e| anyhow::anyhow!("Headless server migrations failed: {}", e))?;

        // Then desktop migrations.
        let mut desktop_migrator = sqlx::migrate!("./migrations");
        desktop_migrator.set_ignore_missing(true);
        desktop_migrator
            .run(&setup_pool)
            .await
            .map_err(|e| anyhow::anyhow!("Headless desktop migrations failed: {}", e))?;

        setup_pool.close().await;
        eprintln!("[headless] pre-server migrations applied; pool closed");
    }

    // Server-side bootstrapping. Mirrors what the Tauri-launched
    // `start_backend_server` does internally. `start_server_with_routes`
    // binds the listener, spawns axum::serve into a detached task,
    // and RETURNS the bound SocketAddr — the server keeps running
    // in the background. So we await it inline (don't spawn), then
    // block on Ctrl+C forever so the parent test process keeps the
    // server alive until it's done driving requests at us.
    //
    // PARITY: keep this handler list in lockstep with the GUI path
    // (`modules::backend::start_backend_server`). Integration tests
    // (`TestServer::start_desktop()`) spawn THIS function — if a
    // handler is only registered in the GUI path, every test that
    // depends on it silently sees zero side-effects.
    let handlers: Vec<std::sync::Arc<dyn EventHandler>> = vec![
        crate::modules::llm_provider::AutoAssignProviderHandler::new(),
        crate::modules::mcp::AutoAssignMcpServerHandler::new(),
    ];

    // Build a CORS layer identical to the one the server applies to
    // its own routes. Required because we re-layer the desktop sub-
    // router below (see comment in the closure for the full why).
    let desktop_cors = ziee::create_cors_layer(&config);

    // PARITY with the GUI path (`start_backend_server`): office_bridge's STATIC
    // seams (chat extension + auto-attach entry) MUST register BEFORE
    // `start_server_with_routes` builds the chat module (which snapshots the
    // ExtensionRegistry) — a post-start push would be too late. Pool-free; no-op
    // without Office. `config` is moved into the server below, so clone it for the
    // post-start runtime half.
    let ob_config = config.clone();
    crate::modules::office_bridge::register_office_bridge_static(&config);

    let addr = ziee::start_server_with_routes(
        config,
        move |router, jwt: std::sync::Arc<JwtService>| {
            // Initialize desktop repos with the server's pool.
            let pool = ziee::Repos.pool().clone();
            crate::core::init_desktop_repositories(pool);

            // CRITICAL: re-apply layers to the merged desktop routes.
            // `setup.app` was layered with CORS + Extension(jwt_service)
            // BEFORE this closure runs, but axum's `.merge()` does NOT
            // propagate parent layers onto newly-merged routes. Without
            // these explicit layers:
            //   - Browser preflight (OPTIONS) → 405 because the merged
            //     routes have no CORS layer.
            //   - Authenticated requests → 500 "JWT service not
            //     configured" because `RequirePermissions` can't find
            //     `Arc<JwtService>` in request extensions.
            router.merge(
                desktop_routes
                    .layer(axum::Extension(jwt))
                    .layer(desktop_cors),
            )
        },
        handlers,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Headless server failed: {}", e))?;

    // Publish the actually-bound port to the remote_access state so
    // the auto-start ngrok forwarder targets the right upstream
    // (not the fallback 8080).
    crate::modules::remote_access::set_local_server_port(addr.port());

    // PARITY with the GUI path (`start_backend_server`): register the
    // host-folder mount provider against the sandbox seam now that the pool
    // exists. Without this, headless `execute_command` sees no host mounts.
    crate::modules::host_mount::register_provider();

    // PARITY with the GUI path: office_bridge's RUNTIME half — upsert its
    // mcp_servers row + spawn the add-in bridge listener + document watcher. Safe
    // now: migrations ran pre-server, `ziee::Repos` is initialized. No-op without
    // Office. (The static half registered before start_server_with_routes above.)
    crate::modules::office_bridge::register_office_bridge(&ob_config);

    // PARITY with the GUI path: idempotent backfill of system MCP →
    // group assignments. The per-event handler above catches NEW
    // creates; this catches built-in registrations whose insert-if-
    // absent path may not emit `SystemServerCreated` and pre-existing
    // rows on a re-run against an existing DB.
    if let Err(e) = crate::modules::mcp::backfill_system_mcp_assignments().await {
        tracing::error!(
            error = %e,
            "headless: backfill_system_mcp_assignments failed"
        );
    }

    tracing::info!("Headless server live at {}; waiting for shutdown signal", addr);

    // Block on Ctrl+C / SIGTERM. The TestServer in the parent
    // process sends SIGKILL when the test finishes; on Ctrl+C we
    // exit cleanly.
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to install Ctrl+C handler: {}", e))?;
    tracing::info!("Headless server received Ctrl+C; shutting down");

    // Suppress unused-warning on the locally-bound module collection;
    // we own it for the lifetime of the run.
    let _ = &mut desktop_modules;
    Ok(())
}

/// Run the desktop application
///
/// # Arguments
/// * `config_file` - Optional path to a YAML config file (like server's dev.yaml)
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(config_file: Option<String>) -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Ziee Desktop...");
    if let Some(ref path) = config_file {
        tracing::info!("Using config file: {}", path);
    }

    // Create desktop modules with config
    let mut modules = core::create_desktop_modules(config_file);
    tracing::info!("Created {} desktop modules", modules.len());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_decorum::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    register_desktop_invoke_handler(builder)
        .setup(move |app| {
            tracing::info!("Tauri setup starting...");

            // Store AppHandle globally for route handlers
            core::set_app_handle(app.handle().clone());

            // Initialize all modules
            core::initialize_modules(&mut modules, app)?;

            // Collect API routes from all modules (with OpenAPI documentation)
            let desktop_routes = core::build_desktop_api_routes(&modules);

            // Start the backend server with collected routes (pass AppHandle for window creation)
            modules::backend::start_backend_server(desktop_routes, app.handle().clone());

            tracing::info!("Tauri setup complete");
            Ok(())
        })
        // Window event handler for cleanup
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    tracing::info!("Main window close requested, cleaning up...");
                    tauri::async_runtime::spawn(async move {
                        // Stop the ngrok tunnel BEFORE tearing down
                        // the DB. Skipping this leaves ngrok's edge
                        // holding the reservation (next launch fails
                        // to bind the domain), and the spawned
                        // forwarder task is killed mid-flight without
                        // graceful close.
                        if let Err(e) = crate::modules::remote_access::tunnel_driver()
                            .0
                            .stop()
                            .await
                        {
                            tracing::warn!(error = %e, "tunnel stop on shutdown failed");
                        }
                        ziee::cleanup_server().await;
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
