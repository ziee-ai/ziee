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
            // v2: real download + sha256 verify against embedded
            // known_revisions.toml + optional cosign verify + atomic
            // install. The `version` arg may be:
            //   - "latest" → resolve to the newest non-yanked entry
            //     in the embedded known_revisions.toml that matches
            //     this binary's SANDBOX_ROOTFS_SCHEMA_VERSION
            //   - "v1.r3" → exact pin (must appear in known_revisions)
            let arch = std::env::consts::ARCH.to_string();
            fetch_sandbox_rootfs(&version, &flavor, &arch)
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

/// Phase 8d: real fetch implementation.
///
/// Downloads the requested rootfs from GitHub Releases (override the
/// base URL via `CODE_SANDBOX_ROOTFS_MIRROR`), verifies the sha256
/// against the embedded `known_revisions.toml`, and runs cosign
/// verification when the bundle file is published alongside the
/// release. Falls back to sha256-only when cosign isn't installed.
///
/// Returns the process exit code (0 on success).
fn fetch_sandbox_rootfs(version: &str, flavor: &str, arch: &str) -> i32 {
    // Use the in-crate path, NOT `ziee_chat::code_sandbox::*`. The
    // library-crate import causes linkme to see the MODULE_ENTRIES
    // distributed_slice as registered twice at runtime (once via
    // `mod modules` in main.rs, once via the implicit `extern crate
    // ziee_chat` that `use ziee_chat::...` activates), panicking the
    // binary with "duplicate #[distributed_slice]".
    use crate::modules::code_sandbox::{
        SANDBOX_KNOWN_REVISIONS_TOML, SANDBOX_ROOTFS_SCHEMA_VERSION,
    };
    // Parse the embedded known_revisions.toml.
    let known: toml::Value = match toml::from_str(SANDBOX_KNOWN_REVISIONS_TOML) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "ERROR: embedded known_revisions.toml is invalid TOML: {e}\n\
                 The server binary was built from an inconsistent tree. Re-build.\n\
                 (If you're trying to bootstrap the first release, run\n\
                 ./scripts/bootstrap-first-rootfs-release.sh first.)"
            );
            return 2;
        }
    };
    let entries: Vec<&toml::value::Table> = known
        .get("revision")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_table()).collect())
        .unwrap_or_default();
    if entries.is_empty() {
        eprintln!(
            "ERROR: embedded known_revisions.toml is empty.\n\
             No published releases yet — run\n\
             ./scripts/bootstrap-first-rootfs-release.sh to cut the first one.\n\
             Or build locally: ziee-chat build-sandbox-rootfs --flavor {flavor}"
        );
        return 2;
    }

    // Helper: numeric extraction of an "rN" revision string. Used
    // for `latest` resolution so r10 sorts AFTER r9 (lexicographic
    // sort would put r10 before r2 — silent downgrade once revisions
    // exceed r9).
    fn revision_number(rev: &str) -> Option<u32> {
        rev.strip_prefix('r').and_then(|n| n.parse().ok())
    }
    // Helper: is this entry marked yanked?
    fn is_yanked(e: &toml::value::Table) -> bool {
        e.get("yanked").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    // Resolve the version.
    let resolved = if version == "latest" {
        // Pick the newest non-yanked entry matching schema + arch + flavor.
        let mut candidates: Vec<&toml::value::Table> = entries
            .iter()
            .copied()
            .filter(|e| {
                !is_yanked(e)
                    && e.get("schema").and_then(|v| v.as_integer())
                        == Some(SANDBOX_ROOTFS_SCHEMA_VERSION as i64)
                    && e.get("arch").and_then(|v| v.as_str()) == Some(arch)
                    && e.get("flavor").and_then(|v| v.as_str()) == Some(flavor)
            })
            .collect();
        // Numeric sort by revision number — lexicographic would give
        // r10 < r2 < r9 → `latest` picks r9 once we hit r10+.
        candidates.sort_by_key(|e| {
            e.get("revision")
                .and_then(|v| v.as_str())
                .and_then(revision_number)
                .unwrap_or(0)
        });
        match candidates.last() {
            Some(c) => (*c).clone(),
            None => {
                eprintln!(
                    "ERROR: no published, non-yanked revision matches \
                     schema={} arch={} flavor={}",
                    SANDBOX_ROOTFS_SCHEMA_VERSION, arch, flavor
                );
                return 2;
            }
        }
    } else {
        // Exact match: parse "v1.r3" → schema=1, revision="r3".
        let v = version.strip_prefix('v').unwrap_or(version);
        let mut parts = v.splitn(2, '.');
        let schema: i64 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(n) => n,
            None => {
                eprintln!("ERROR: invalid version {version:?} (expected vN.rM)");
                return 2;
            }
        };
        let revision = match parts.next() {
            Some(s) => s.to_string(),
            None => {
                eprintln!("ERROR: invalid version {version:?} (expected vN.rM)");
                return 2;
            }
        };
        match entries.iter().find(|e| {
            e.get("schema").and_then(|v| v.as_integer()) == Some(schema)
                && e.get("revision").and_then(|v| v.as_str()) == Some(&revision)
                && e.get("arch").and_then(|v| v.as_str()) == Some(arch)
                && e.get("flavor").and_then(|v| v.as_str()) == Some(flavor)
        }) {
            Some(c) => (*c).clone(),
            None => {
                eprintln!(
                    "ERROR: {version} (arch={arch} flavor={flavor}) not in known_revisions.toml"
                );
                return 2;
            }
        }
    };

    let schema = resolved.get("schema").and_then(|v| v.as_integer()).unwrap_or(0);
    let revision = resolved.get("revision").and_then(|v| v.as_str()).unwrap_or("");
    // Normalize sha256 to lowercase + assert canonical 64-hex-char
    // shape. A hand-edited known_revisions.toml with an uppercase or
    // wrong-length sha would otherwise mismatch every download with a
    // confusing error.
    let expected_sha = match resolved.get("sha256").and_then(|v| v.as_str()) {
        Some(s) => {
            let lc = s.trim().to_lowercase();
            if lc.len() != 64 || !lc.chars().all(|c| c.is_ascii_hexdigit()) {
                eprintln!(
                    "ERROR: known_revisions entry has malformed sha256 {:?} \
                     (expected 64 lowercase hex chars)",
                    s
                );
                return 2;
            }
            lc
        }
        None => {
            eprintln!("ERROR: known_revisions entry missing sha256 field");
            return 2;
        }
    };
    let signed_required = resolved
        .get("signed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Schema compatibility check.
    if schema != SANDBOX_ROOTFS_SCHEMA_VERSION as i64 {
        eprintln!(
            "ERROR: schema mismatch — server binary expects {} but \
             resolved revision is schema {}. Either rebuild the server \
             or pin a compatible revision.",
            SANDBOX_ROOTFS_SCHEMA_VERSION, schema
        );
        return 2;
    }

    // Build the URL. Mirror env var honored, but HTTPS-only (sha256
    // verification below makes payload tampering moot, but rejecting
    // plain http stops downgrade-via-misconfig early).
    let base_url = std::env::var("CODE_SANDBOX_ROOTFS_MIRROR")
        .unwrap_or_else(|_| "https://github.com/phibya/ziee-chat/releases/download".to_string());
    if !base_url.starts_with("https://") {
        eprintln!(
            "ERROR: CODE_SANDBOX_ROOTFS_MIRROR must be https:// (got {base_url:?}). \
             Plain http is rejected even though sha256 verification \
             would catch tampering — refusing to fall back."
        );
        return 2;
    }
    if std::env::var("CODE_SANDBOX_ROOTFS_MIRROR").is_ok() {
        eprintln!("    WARNING: using mirror {base_url}");
    }
    let tag = format!("sandbox-rootfs-v{schema}.{revision}-{arch}");
    let asset = format!("ziee-sandbox-rootfs-v{schema}.{revision}-{arch}-{flavor}.squashfs");
    let url = format!("{base_url}/{tag}/{asset}");

    // Output path.
    let cache_dir = repo_relative(".ziee-cache/sandbox-rootfs");
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        eprintln!("ERROR: cannot create cache dir {}: {e}", cache_dir.display());
        return 2;
    }
    let out_path = cache_dir.join(&asset);
    let tmp_path = out_path.with_extension("squashfs.tmp");

    // Download via curl (no async runtime here; main is already inside
    // a tokio runtime but the subcommand path is sync — use curl to
    // keep the dep surface small).
    eprintln!("==> Downloading {url}");
    let curl = std::process::Command::new("curl")
        .args(["-fSL", "--retry", "3", "--retry-delay", "2", "-o"])
        .arg(&tmp_path)
        .arg(&url)
        .status();
    match curl {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("ERROR: curl exited {s} downloading {url}");
            let _ = std::fs::remove_file(&tmp_path);
            return 2;
        }
        Err(e) => {
            eprintln!("ERROR: curl spawn failed: {e}. Install curl: apt install curl");
            return 2;
        }
    }

    // Verify sha256 against embedded known value (NOT a downloaded
    // .sha256 file — that would be circular).
    eprintln!("==> Verifying sha256");
    let actual_sha = match sha256_file(&tmp_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: sha256 failed: {e}");
            let _ = std::fs::remove_file(&tmp_path);
            return 2;
        }
    };
    if actual_sha != expected_sha {
        eprintln!(
            "ERROR: sha256 mismatch.\n  expected: {expected_sha}\n  got:      {actual_sha}\n\
             The download may be corrupt, intercepted, or the embedded\n\
             known_revisions.toml is stale (rebuild the server)."
        );
        let _ = std::fs::remove_file(&tmp_path);
        return 2;
    }
    eprintln!("    sha256 OK ({actual_sha})");

    // Cosign verify path. If the entry says `signed = true` in
    // known_revisions.toml, we REQUIRE cosign + the bundle and fail
    // closed on any error. If `signed = false` (or absent for
    // legacy entries), we attempt cosign opportunistically but
    // proceed sha256-only on miss.
    //
    // The certificate-identity-regexp is ANCHORED at both ends and
    // pins the specific workflow file. An unanchored pattern would
    // let any branch/workflow in the real repo mint signatures —
    // e.g., a PR adding `.github/workflows/evil.yml` that calls
    // `cosign sign-blob` would pass verification. The anchored
    // regex below requires the signature to come from a tag-push
    // run of sandbox-rootfs-release.yml.
    let bundle_url = format!("{url}.cosign.bundle");
    let bundle_path = out_path.with_extension("squashfs.cosign.bundle");
    let cosign_installed = std::process::Command::new("cosign")
        .arg("version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !cosign_installed {
        if signed_required {
            eprintln!(
                "ERROR: this revision has `signed = true` but cosign is \
                 not installed. Install cosign \
                 (https://docs.sigstore.dev/system_config/installation/) \
                 or pin to an unsigned revision."
            );
            let _ = std::fs::remove_file(&tmp_path);
            return 2;
        }
        eprintln!("    (cosign not installed; sha256-only verification)");
    } else {
        eprintln!("==> Downloading cosign bundle");
        let bundle_dl = std::process::Command::new("curl")
            .args(["-fSL", "-o"])
            .arg(&bundle_path)
            .arg(&bundle_url)
            .status();
        let bundle_present = matches!(bundle_dl, Ok(s) if s.success()) && bundle_path.exists();
        if !bundle_present {
            if signed_required {
                eprintln!(
                    "ERROR: this revision has `signed = true` but the \
                     cosign bundle was not downloadable from {bundle_url}. \
                     Refusing to install — this could be a signature \
                     downgrade attack. If you're certain the signature \
                     was lost, unmark the revision as signed."
                );
                let _ = std::fs::remove_file(&tmp_path);
                return 2;
            }
            eprintln!("    (no cosign bundle published; sha256-only)");
        } else {
            eprintln!("==> Verifying cosign signature");
            let v = std::process::Command::new("cosign")
                .args(["verify-blob", "--bundle"])
                .arg(&bundle_path)
                .args([
                    "--certificate-identity-regexp",
                    // Anchored: only signatures from a tag-push run of
                    // sandbox-rootfs-release.yml on the official repo
                    // satisfy this. Any other branch/workflow/repo
                    // produces a different SAN that fails the match.
                    r"^https://github\.com/phibya/ziee-chat/\.github/workflows/sandbox-rootfs-release\.yml@refs/tags/sandbox-rootfs-v[0-9]+\.r[0-9]+-[a-z0-9_]+$",
                    "--certificate-oidc-issuer",
                    "https://token.actions.githubusercontent.com",
                ])
                .arg(&tmp_path)
                .status();
            match v {
                Ok(s) if s.success() => eprintln!("    cosign OK"),
                _ => {
                    eprintln!("ERROR: cosign verification failed. Refusing to install.");
                    let _ = std::fs::remove_file(&tmp_path);
                    let _ = std::fs::remove_file(&bundle_path);
                    return 2;
                }
            }
        }
    }

    // Atomic rename — last step so a partial install doesn't shadow
    // a previously-good rootfs.
    if let Err(e) = std::fs::rename(&tmp_path, &out_path) {
        eprintln!("ERROR: rename to {} failed: {e}", out_path.display());
        let _ = std::fs::remove_file(&tmp_path);
        return 2;
    }
    eprintln!("==> Installed {}", out_path.display());
    eprintln!();
    eprintln!("Next: ziee-chat mount-sandbox-rootfs");
    0
}

/// Compute sha256 of a file. Inline-implemented to avoid pulling
/// `sha2` into main.rs's compile surface (it's already a transitive
/// dep; we just need lowercase hex of the hash).
fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
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
