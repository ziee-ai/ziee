// Core modules for modular architecture
mod common;
mod core;
mod module_api;
mod modules;
mod openapi;
mod utils;

use clap::Parser;
use module_api::ModuleContext;
use tokio::signal;

#[derive(Parser, Debug)]
#[command(name = "ziee")]
#[command(version, about = "Ziee Chat backend server", long_about = None)]
struct Cli {
    /// Path to configuration file (overrides CONFIG_FILE env var)
    #[arg(long, value_name = "FILE")]
    config_file: Option<String>,

    /// Generate OpenAPI specification and TypeScript types, then exit
    /// If no value is provided, defaults to ../ui/openapi
    #[arg(long, value_name = "OUTPUT_DIR", num_args = 0..=1, default_missing_value = "../ui/openapi")]
    generate_openapi: Option<String>,

    /// (Windows, internal) Run as the LocalSystem code-sandbox helper service.
    /// Invoked by the Service Control Manager — not meant to be run by hand.
    #[cfg(windows)]
    #[arg(long, hide = true)]
    run_sandbox_helper_service: bool,

    /// (Windows) Install the code-sandbox helper as a LocalSystem service.
    /// Must be run as Administrator. Registers the vsock GUIDs + restarts WSL.
    #[cfg(windows)]
    #[arg(long)]
    install_sandbox_helper: bool,

    /// (Windows) Stop + remove the code-sandbox helper service.
    /// Must be run as Administrator.
    #[cfg(windows)]
    #[arg(long)]
    uninstall_sandbox_helper: bool,

    /// (Windows, internal) Set on the elevated child that
    /// `--install-sandbox-helper` spawns, so it performs the install instead
    /// of trying to elevate again (prevents a UAC loop).
    #[cfg(windows)]
    #[arg(long, hide = true)]
    sandbox_helper_elevated: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Windows code-sandbox helper service dispatch. These short-circuit the
    // normal server boot. Gated to Windows — the helper brokers privileged
    // WSL ops (see modules::code_sandbox::backend::helper_service).
    #[cfg(windows)]
    {
        use crate::modules::code_sandbox::backend::helper_service;

        if cli.run_sandbox_helper_service {
            // Launched by the SCM: hand control to the service dispatcher.
            if let Err(e) = helper_service::service::run() {
                eprintln!("ziee sandbox helper service failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        if cli.install_sandbox_helper {
            // Self-checking + self-elevating: silent no-op if already
            // installed, one UAC prompt if it needs installing. Safe to call
            // on every app launch.
            match helper_service::install::install(cli.sandbox_helper_elevated) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("install-sandbox-helper failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        if cli.uninstall_sandbox_helper {
            match helper_service::install::uninstall() {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("uninstall-sandbox-helper failed: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    // Check for OpenAPI generation flag
    if cli.generate_openapi.is_some() {
        let output_dir = cli.generate_openapi.unwrap_or_else(|| {
            // CARGO_MANIFEST_DIR is only available during development builds with Cargo
            // In production, you must explicitly specify the output directory with --generate-openapi <DIR>
            match option_env!("CARGO_MANIFEST_DIR") {
                Some(manifest_dir) => format!("{}/../ui/openapi", manifest_dir),
                None => {
                    eprintln!("Please specify an output directory explicitly:");
                    eprintln!("  --generate-openapi /path/to/output");
                    std::process::exit(1);
                }
            }
        });

        match openapi::generate_openapi_spec(&output_dir, cli.config_file).await {
            Ok(_) => {
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Error generating OpenAPI spec: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Load configuration first (use --config-file if provided, otherwise fall back to CONFIG_FILE env)
    let config = match core::config::Config::load_from(cli.config_file) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize tracing for logging based on config.
    //
    // Uses EnvFilter so operators can do `RUST_LOG=ziee=debug,sqlx=warn`
    // for module-level filtering. Closes 14-core F-23 (Info). Falls back
    // to the config-file level when RUST_LOG is unset.
    use tracing_subscriber::filter::EnvFilter;
    let config_level = config
        .logging
        .as_ref()
        .map(|l| l.level.as_str())
        .unwrap_or("info");
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config_level));

    let format = config
        .logging
        .as_ref()
        .map(|l| l.format.as_str())
        .unwrap_or("default");
    match format {
        "compact" => {
            tracing_subscriber::fmt()
                .compact()
                .with_env_filter(env_filter)
                .init();
        }
        "pretty" => {
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(env_filter)
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();
        }
    }

    tracing::info!("Starting Ziee backend server");

    // Initialize application data directory + caches config from the
    // resolved Config. `Config::resolve_paths` (called inside load_from)
    // guarantees app.data_dir is set and every caches.*_dir is Some(...).
    if let Some(ref app_config) = config.app {
        let data_dir = std::path::PathBuf::from(&app_config.data_dir);
        core::set_app_data_dir(data_dir);
    } else {
        // Use default if not configured
        let default_data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ziee");
        core::set_app_data_dir(default_data_dir);
    }
    core::set_caches_config(config.caches.clone());
    // Capture server addr so the llm_local_runtime URL injection
    // (repository read-time) can derive the live proxy base_url
    // without holding a reference to the full Config. The api_prefix
    // is included because module routes are nested under it.
    core::set_server_addr(
        config.server.host.clone(),
        config.server.port,
        config.server.api_prefix.clone(),
    );

    // Initialize database
    let pool = match core::database::initialize_database(&config).await {
        Ok(pool) => {
            tracing::info!("Database initialized with {} connections", pool.num_idle());
            pool
        }
        Err(e) => {
            tracing::error!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize global repository factory
    core::init_repositories((*pool).clone());
    tracing::info!("Global repository factory initialized");

    // Initialize at-rest secret storage key from config. Closes
    // 06-llm-provider F-02 (Critical) once configured; compat mode if
    // secrets.storage_key is absent.
    core::secrets::init_storage_key(
        config
            .secrets
            .as_ref()
            .and_then(|s| s.storage_key.clone()),
    );

    // Initialize modules
    let module_context = ModuleContext::new(pool.clone(), std::sync::Arc::new(config.clone()));
    let mut modules = core::app_builder::create_modules();

    // Initialize all modules
    if let Err(e) = core::app_builder::initialize_modules(&mut modules, &module_context) {
        tracing::error!("Failed to initialize modules: {}", e);
        std::process::exit(1);
    }

    // Register event handlers from all modules
    let event_bus = std::sync::Arc::new(core::app_builder::register_event_handlers(
        &modules,
        pool.clone(),
    ));
    tracing::info!(
        "Event bus initialized with {} handlers",
        event_bus.handler_count()
    );

    // Setup CORS from config
    let cors = core::app_builder::create_cors_layer(&config);

    // Set up JWT service. try_new refuses weak/placeholder secrets so
    // the server never boots with a known signer. Closes 01-auth F-10
    // + 14-core F-03.
    let jwt_service = match modules::auth::JwtService::try_new(config.jwt.clone()) {
        Ok(svc) => std::sync::Arc::new(svc),
        Err(e) => {
            tracing::error!("Failed to initialize JWT service: {}", e);
            std::process::exit(1);
        }
    };
    tracing::info!("JWT service initialized");

    // Set up MCP session manager
    let mcp_session_manager = std::sync::Arc::new(modules::mcp::client::McpSessionManager::new(
        module_context.config.clone(),
    ));
    tracing::info!("MCP session manager initialized");

    // Build API router with all module routes (including auth)
    let (api_router, mut api_doc) = core::app_builder::build_api_router(
        &modules,
        &config.server.api_prefix,
        (*module_context.db_pool).clone(),
    );

    // Convert ApiRouter to Router and add JWT service and CORS layers.
    //
    // SECURITY: the global body limit is set to 16 MB here. Upload routes
    // that legitimately need more (file upload, model upload, etc.) opt
    // into a higher per-route limit via `.layer(DefaultBodyLimit::max(N))`
    // on their handler. The previous `disable()` here let unauthenticated
    // POSTs to ANY endpoint stream multi-GB bodies and OOM the server —
    // see 14-core-infrastructure F-01.
    // SECURITY: middleware stack (A3). Layers wrap from bottom-up so
    // a request flows through cors → headers → timeout → body-limit
    // before reaching the handler.
    //
    // - DefaultBodyLimit::max — 16 MB cap (per-route upload routes raise this).
    // - TimeoutLayer 60s — request hard-deadline. SSE/streaming routes that
    //   need longer override per-route; this is the global default.
    //   Closes 05-file F-09 generalization + similar.
    // - Security headers (X-Content-Type-Options, X-Frame-Options,
    //   Referrer-Policy, Permissions-Policy, Strict-Transport-Security).
    //   These are response-only defenses but cheap and audit-recommended.
    // Rate limiter: 5 req/sec per peer IP, burst-able to 60.
    // PeerIpKeyExtractor uses the TCP peer address (not X-Forwarded-For)
    // — appropriate for direct-connect deployments and TestServer.
    // Production behind a reverse proxy should swap for
    // SmartIpKeyExtractor and configure trusted-forwarded-for sources.
    // Closes a substantial chunk of the auth/file/chat rate-limit
    // findings (01-auth F-05, 03-user F-12, 04-chat F-04 message-stream
    // rate, 06-llm-provider F-13, 08-llm-local-runtime F-06).
    // Config-driven rate limits. Defaults to 50 req/s sustained,
    // 500-burst — wide enough that a normal SPA cold-load (15-25
    // parallel API calls + secondary fetches) doesn't trip 429,
    // tight enough to still blunt brute-force / scraping. Hardened
    // deployments behind a real reverse proxy should override
    // downward via `server.rate_limit` in config; tests already
    // override upward to handle sequential-burst sweeps against a
    // single peer-IP bucket.
    let (rl_per_sec, rl_burst) = config
        .server
        .rate_limit
        .as_ref()
        .map(|r| (r.per_second, r.burst_size))
        .unwrap_or((50, 500));
    let governor_conf = std::sync::Arc::new(
        tower_governor::governor::GovernorConfigBuilder::default()
            .per_second(rl_per_sec)
            .burst_size(rl_burst)
            .key_extractor(tower_governor::key_extractor::PeerIpKeyExtractor)
            .finish()
            .expect("Failed to build governor config"),
    );
    let governor_layer = tower_governor::GovernorLayer {
        config: governor_conf,
    };

    let app = api_router
        .finish_api(&mut api_doc)
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(60)))
        .layer(governor_layer)
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            axum::http::HeaderValue::from_static("no-referrer"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("permissions-policy"),
            axum::http::HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("strict-transport-security"),
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(axum::Extension(event_bus))
        .layer(axum::Extension(jwt_service))
        .layer(axum::Extension(mcp_session_manager.clone()))
        .layer(cors);

    // Get server address
    let addr = config.server_address();
    tracing::info!("Starting HTTP server on {}", addr);

    // Create listener
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    tracing::info!("Ziee Chat backend server started successfully on {}", addr);

    // Run server with graceful shutdown. into_make_service_with_connect_info
    // surfaces the TCP peer address so tower_governor's PeerIpKeyExtractor
    // can read it (otherwise rate-limiting fails with HTTP 500 because the
    // extractor can't find the peer IP). Closes A3 rate-limit wiring.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Failed to start server");

    tracing::info!("Shutting down...");

    // Close all MCP sessions
    if let Err(e) = mcp_session_manager.close_all().await {
        tracing::warn!("Error closing MCP sessions during shutdown: {}", e);
    }

    core::database::cleanup_database().await;
}

async fn shutdown_signal() {
    // Graceful-with-warning instead of panicking. Closes 14-core F-19
    // (Low): a container that strips signal-handler installation
    // (e.g. unusual seccomp profile) used to crash here; now it
    // logs + falls back to "never returns", which lets the runtime's
    // normal shutdown path take over.
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            tracing::warn!("Failed to install Ctrl+C handler: {}", e);
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!("Failed to install SIGTERM handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");

    // Tear down the server-owned squashfuse FUSE daemon (if any was
    // lazily spawned by code_sandbox). No-op if sandbox is disabled
    // or no execute_command ever ran. PDEATHSIG handles SIGKILL paths
    // where this hook can't run.
    modules::code_sandbox::backend::active().shutdown().await;
}
