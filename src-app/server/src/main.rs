// Core modules for modular architecture
mod common;
mod core;
mod module_api;
mod modules;
mod openapi;
mod utils;

use clap::{Parser, Subcommand};
use module_api::ModuleContext;
use tokio::signal;

#[derive(Parser, Debug)]
#[command(name = "ziee-chat")]
#[command(version, about = "Ziee Chat backend server", long_about = None)]
struct Cli {
    /// Path to configuration file (overrides CONFIG_FILE env var)
    #[arg(long, value_name = "FILE")]
    config_file: Option<String>,

    /// Generate OpenAPI specification and TypeScript types, then exit
    /// If no value is provided, defaults to ../ui/openapi
    #[arg(long, value_name = "OUTPUT_DIR", num_args = 0..=1, default_missing_value = "../ui/openapi")]
    generate_openapi: Option<String>,

    /// Sub-commands for operational tasks (rootfs build/fetch/mount/gc,
    /// etc.). Without a subcommand, ziee-chat boots as a server.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build a sandbox rootfs squashfs from src-app/sandbox-rootfs/ sources.
    /// Wraps src-app/sandbox-rootfs/build.sh; pass-through args.
    BuildSandboxRootfs {
        /// Rootfs flavor: minimal (~150 MB) or full (~1.6 GB).
        #[arg(long, default_value = "full")]
        flavor: String,
        /// Optional output path override.
        #[arg(long)]
        output: Option<String>,
    },
    /// Mount a built rootfs squashfs via squashfuse and flip the
    /// `current` symlink atomically.
    MountSandboxRootfs {
        /// Path to the .squashfs to mount. If omitted, picks the most
        /// recent one in .ziee-cache/sandbox-rootfs/.
        #[arg(long)]
        rootfs: Option<String>,
    },
    /// Download a published rootfs from GitHub Releases (v2 stub).
    /// Currently prints the gh-release command; will perform real
    /// fetch + sha256 + cosign verification once releases exist.
    FetchSandboxRootfs {
        /// Tag suffix to download (e.g. v1.r0-x86_64-minimal).
        #[arg(long, default_value = "latest")]
        version: String,
        /// Flavor selector.
        #[arg(long, default_value = "minimal")]
        flavor: String,
    },
    /// Remove cached rootfs versions, keeping only the N most recent.
    GcSandboxRootfs {
        /// Number of recent versions to keep.
        #[arg(long, default_value_t = 2)]
        keep: usize,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Dispatch operational subcommands BEFORE any of the server
    // boot sequence runs (these are short-lived; they don't need the
    // full module init).
    if let Some(cmd) = cli.command {
        let code = run_sandbox_subcommand(cmd);
        std::process::exit(code);
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

    // Initialize tracing for logging based on config
    if let Some(ref logging_config) = config.logging {
        let level = logging_config
            .level
            .parse::<tracing_subscriber::filter::LevelFilter>()
            .unwrap_or(tracing_subscriber::filter::LevelFilter::INFO);

        match logging_config.format.as_str() {
            "compact" => {
                tracing_subscriber::fmt()
                    .compact()
                    .with_max_level(level)
                    .init();
            }
            "pretty" => {
                tracing_subscriber::fmt()
                    .pretty()
                    .with_max_level(level)
                    .init();
            }
            _ => {
                // Default format
                tracing_subscriber::fmt().with_max_level(level).init();
            }
        }
    } else {
        // Default logging if not configured
        tracing_subscriber::fmt()
            .with_max_level(tracing_subscriber::filter::LevelFilter::INFO)
            .init();
    }

    tracing::info!("Starting Ziee Chat backend server");

    // Initialize application data directory from config
    if let Some(ref app_config) = config.app {
        let data_dir = std::path::PathBuf::from(&app_config.data_dir);
        core::set_app_data_dir(data_dir);
    } else {
        // Use default if not configured
        let default_data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ziee-chat");
        core::set_app_data_dir(default_data_dir);
    }

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

    // Set up JWT service
    let jwt_service = std::sync::Arc::new(modules::auth::JwtService::new(config.jwt.clone()));
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

    // Convert ApiRouter to Router and add JWT service and CORS layers
    // Disable body size limit for model uploads (models can be very large)
    let app = api_router
        .finish_api(&mut api_doc)
        .layer(axum::extract::DefaultBodyLimit::disable())
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

    // Run server with graceful shutdown
    axum::serve(listener, app.into_make_service())
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

/// Dispatch the operational sandbox subcommands. Each wraps the
/// corresponding shell tooling so users don't have to remember the
/// exact path/flags. Returns the exit code to propagate.
fn run_sandbox_subcommand(cmd: Command) -> i32 {
    match cmd {
        Command::BuildSandboxRootfs { flavor, output } => {
            let script = repo_relative("src-app/sandbox-rootfs/build.sh");
            if !script.exists() {
                eprintln!("missing: {}", script.display());
                return 1;
            }
            let mut args: Vec<String> = vec!["--flavor".into(), flavor];
            if let Some(o) = output {
                args.push("--output".into());
                args.push(o);
            }
            std::process::Command::new(&script)
                .args(&args)
                .status()
                .map(|s| s.code().unwrap_or(1))
                .unwrap_or_else(|e| {
                    eprintln!("failed to invoke {}: {e}", script.display());
                    1
                })
        }
        Command::MountSandboxRootfs { rootfs } => {
            let cache = repo_relative(".ziee-cache/sandbox-rootfs");
            let sqfs = match rootfs {
                Some(p) => std::path::PathBuf::from(p),
                None => match latest_squashfs(&cache) {
                    Some(p) => p,
                    None => {
                        eprintln!(
                            "no squashfs found in {}; run `ziee-chat build-sandbox-rootfs` first",
                            cache.display()
                        );
                        return 1;
                    }
                },
            };
            if !sqfs.exists() {
                eprintln!("squashfs not found: {}", sqfs.display());
                return 1;
            }
            let stem = sqfs
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("sandbox-rootfs");
            let mnt = cache.join(stem);
            if let Err(e) = std::fs::create_dir_all(&mnt) {
                eprintln!("mkdir {}: {e}", mnt.display());
                return 1;
            }
            // Idempotent: skip if already mounted (mountpoint(1)).
            let already = std::process::Command::new("mountpoint")
                .args(["-q", mnt.to_str().unwrap()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !already {
                let status = std::process::Command::new("squashfuse")
                    .arg(&sqfs)
                    .arg(&mnt)
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        eprintln!("mounted {} at {}", sqfs.display(), mnt.display());
                    }
                    Ok(s) => {
                        eprintln!("squashfuse failed with exit {:?}", s.code());
                        return 1;
                    }
                    Err(e) => {
                        eprintln!("squashfuse not available: {e}");
                        return 1;
                    }
                }
            } else {
                eprintln!("already mounted: {}", mnt.display());
            }
            // Atomic symlink swap: write new symlink to a temp name,
            // then `rename` (POSIX-atomic).
            let current = cache.join("current");
            let tmp = cache.join(".current.new");
            let _ = std::fs::remove_file(&tmp);
            if let Err(e) = std::os::unix::fs::symlink(stem, &tmp) {
                eprintln!("symlink {}: {e}", tmp.display());
                return 1;
            }
            if let Err(e) = std::fs::rename(&tmp, &current) {
                eprintln!("rename current symlink: {e}");
                return 1;
            }
            eprintln!("current → {stem}");
            0
        }
        Command::FetchSandboxRootfs { version, flavor } => {
            // v1 stub: print the gh-release invocation for users to
            // run manually. v2 wires this into actual sha256+cosign
            // verification once releases exist.
            let arch = std::env::consts::ARCH.replace("aarch64", "aarch64");
            eprintln!("ziee-chat fetch-sandbox-rootfs (v1 stub)");
            eprintln!("Once releases exist, run:");
            eprintln!(
                "  gh release download sandbox-rootfs-{version}-{arch} \\"
            );
            eprintln!(
                "    --pattern '*-{arch}-{flavor}.squashfs*' \\"
            );
            eprintln!("    --dir .ziee-cache/sandbox-rootfs/");
            eprintln!("  ziee-chat mount-sandbox-rootfs");
            eprintln!();
            eprintln!("Or build locally with: ziee-chat build-sandbox-rootfs --flavor {flavor}");
            0
        }
        Command::GcSandboxRootfs { keep } => {
            let cache = repo_relative(".ziee-cache/sandbox-rootfs");
            let mut sqfs: Vec<std::path::PathBuf> = match std::fs::read_dir(&cache) {
                Ok(rd) => rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("squashfs"))
                    .collect(),
                Err(_) => return 0,
            };
            sqfs.sort_by_key(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
            });
            sqfs.reverse(); // newest first
            let to_delete: Vec<_> = sqfs.into_iter().skip(keep).collect();
            for p in &to_delete {
                // Also try to unmount the mountpoint that mirrors the
                // squashfs basename.
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let mnt = cache.join(stem);
                    let _ = std::process::Command::new("fusermount")
                        .args(["-u", mnt.to_str().unwrap_or("")])
                        .status();
                    let _ = std::fs::remove_dir(&mnt);
                }
                if let Err(e) = std::fs::remove_file(p) {
                    eprintln!("rm {}: {e}", p.display());
                } else {
                    eprintln!("removed {}", p.display());
                }
            }
            0
        }
    }
}

/// Resolve a path relative to the repo root, walking up from CWD
/// until we find a marker file. Falls back to CWD-relative.
fn repo_relative(suffix: &str) -> std::path::PathBuf {
    let mut cur = std::env::current_dir().unwrap_or_default();
    for _ in 0..6 {
        if cur.join("src-app").is_dir() {
            return cur.join(suffix);
        }
        if !cur.pop() {
            break;
        }
    }
    std::path::PathBuf::from(suffix)
}

fn latest_squashfs(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("squashfs"))
        .collect();
    entries.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
    });
    entries.last().cloned()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
