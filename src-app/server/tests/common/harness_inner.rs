//! Binary-agnostic TestServer harness. Shared between the server
//! crate's integration_tests binary and the desktop crate's
//! integration_tests binary via `#[path]` reuse. The OAuth/LDAP/
//! Apple mocks live alongside this file and are declared by the
//! server crate's `common/mod.rs` (NOT here) because they pull in
//! heavy deps the desktop crate doesn't want.

use sqlx::postgres::PgPoolOptions;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;
use uuid::Uuid;

// Per-worktree DB isolation helper, shared verbatim with build.rs so the
// suffix derivation is identical on both sides.
#[path = "../../build_helper/worktree_db.rs"]
mod worktree_db;

/// Stable per-worktree suffix for this test binary's template DB, derived
/// from the worktree root (same value for the server + desktop crates of one
/// worktree). Empty when DATABASE_URL is a deliberate override or auto-isolate
/// is opted out — preserving the historical single-worktree template names.
fn worktree_suffix() -> String {
    let explicit = env::var("DATABASE_URL").ok();
    if worktree_db::should_auto_isolate(&explicit) {
        format!("_{}", worktree_db::worktree_key(env!("CARGO_MANIFEST_DIR")))
    } else {
        String::new()
    }
}

/// Get database URL from environment or use default
fn database_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string())
}

/// (Windows) Ensure the LocalSystem code-sandbox helper service is installed
/// before any sandbox-enabled test exercises the WSL2 backend. Runs once per
/// test process.
///
/// Delegates to `ziee --install-sandbox-helper`, which is self-checking +
/// self-elevating: it silently no-ops if the service is already registered,
/// and only triggers a UAC prompt the first time it actually installs. That's
/// the exact same code path the desktop app will call on launch, so the tests
/// exercise it too.
///
///   - `ZIEE_WSL_VM_ID` set → dev/CI bypass; no service needed, skip entirely.
///   - Otherwise → run the installer command; panic on failure (the sandbox
///     tiers require the helper on Windows — there's no in-process fallback).
///     In headless/CI runs UAC can't prompt, so set `ZIEE_WSL_VM_ID` there.
#[cfg(windows)]
fn ensure_sandbox_helper_for_tests() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if env::var("ZIEE_WSL_VM_ID").is_ok() {
            return;
        }

        // Integration-test bins live in `target/<profile>/deps/`; the built
        // `ziee.exe` sits one level up in `target/<profile>/`.
        let exe = env::current_exe()
            .ok()
            .and_then(|p| p.parent().and_then(|d| d.parent()).map(|d| d.join("ziee.exe")))
            .filter(|p| p.exists())
            .expect(
                "could not locate ziee.exe next to the test binary; \
                 build it first (`cargo build -p ziee`) so the harness can \
                 install the sandbox helper",
            );

        // The command self-checks (silent if already installed) and
        // self-elevates (one UAC) only when it needs to install.
        let status = Command::new(&exe)
            .arg("--install-sandbox-helper")
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => panic!(
                "`ziee --install-sandbox-helper` exited unsuccessfully ({s:?}). \
                 Install it manually as Administrator, or set ZIEE_WSL_VM_ID."
            ),
            Err(e) => panic!(
                "failed to run `ziee --install-sandbox-helper` ({e}). \
                 Install it manually as Administrator, or set ZIEE_WSL_VM_ID."
            ),
        }

        if !ziee::sandbox_helper_is_running() {
            panic!(
                "sandbox helper still not reachable after install. Check the \
                 Windows Event Log, or run `ziee --install-sandbox-helper` \
                 manually and confirm the 'Ziee Sandbox Helper' service is running."
            );
        }
    });
}

pub struct TestServer {
    process: Child,
    pub base_url: String,
    pub database_name: String,
    pub database_url: String,
    temp_config_path: PathBuf,
    /// Per-test workspace dir for `code_sandbox.workspace_root` (when
    /// sandbox is enabled). Held here so Drop cleans it; tests that
    /// need the path can read `workspace_root` below.
    _workspace_tempdir: Option<tempfile::TempDir>,
    // (workspace_root was removed — unused; the config string still
    // injects it into the test YAML, but no test reads this field.)
    /// Tier-6 cache TempDir holding the staged test squashfs +
    /// known_revisions.dev.toml on Mac/Windows. Held for the
    /// TestServer's lifetime; dropped (which deletes the tree) when
    /// the test ends. Unset on Linux.
    _sandbox_cache_tempdir: Option<std::sync::Arc<tempfile::TempDir>>,
    /// Per-test isolated hub-catalog dir (ZIEE_HUB_DATA_DIR_OVERRIDE).
    /// The hub catalog (`current/`) is per-deployment global mutable
    /// state; without a fresh dir per test, a refresh/activate in one
    /// test contaminates the seed every other test reads. Dropped (tree
    /// deleted) at test end.
    _hub_tempdir: tempfile::TempDir,
    /// Per-test isolated data_dir (mutable state); dropped at test end.
    _data_tempdir: tempfile::TempDir,
}

/// Repo-relative shared cache dir for tests. The test harness injects
/// this as `app.data_dir` in every test config so:
///   - extractions of pandoc/pdfium/uv/bun + the sandbox-runtime bundle
///     happen ONCE across `cargo test` invocations (postgresql_embedded
///     + embedded::ensure both honor the sha/marker → skip re-extract).
///   - tests don't fall back to the dev's real `~/.ziee/` and contaminate
///     production state (the latent harness bug that consolidation
///     surfaced).
/// Lives under `.ziee-cache/` which is gitignored.
pub fn shared_test_app_data_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // <repo>/.ziee-cache/test-app-data/  (manifest_dir = src-app/server)
    let path = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|repo| repo.join(".ziee-cache").join("test-app-data"))
        .expect("repo root walk");
    fs::create_dir_all(&path).expect("create shared test app_data_dir");
    path
}

/// Per-test isolated `app.data_dir` that keeps the EXPENSIVE binary caches
/// shared. Each test gets a fresh TempDir for its MUTABLE state
/// (`files/`, `sandboxes/`, `skills/`, `workflows/`, …) — which is what makes
/// the suite safe to run WITHOUT `--test-threads=1` — while the read-only
/// extracted caches (`bin/` = pandoc/pdfium/uv/bun, `llm-engines/`,
/// `lit-cache/`) are SYMLINKED in from the shared `.ziee-cache` dir so the
/// hundreds-of-MB extraction still happens once per `cargo test` run, not once
/// per test. The TempDir is held on `TestServer` and dropped (tree deleted) at
/// test end. Non-unix: falls back to the shared dir (perf isolation only
/// matters where symlinks exist; the CI parallel target is linux).
pub fn make_isolated_data_dir() -> tempfile::TempDir {
    let shared = shared_test_app_data_dir();
    let td = tempfile::Builder::new()
        .prefix("ziee-test-data-")
        .tempdir()
        .expect("create per-test data_dir TempDir");
    // Symlink the read-only / content-addressed caches so they stay shared.
    // `lib` is load-bearing for the macOS sandbox: the embedded sandbox-runtime
    // bundle extracts its launcher to `bin/` and its dylibs (libkrun, …) to
    // `lib/`, and the launcher's rpath is `@executable_path/../lib`. If `bin`
    // is symlinked to the shared cache but `lib` isn't, the launcher (shared
    // bin) and its dylibs (per-test lib) live in different trees and dyld can't
    // find libkrun → the VM never boots. Keeping `lib` shared alongside `bin`
    // co-locates them, exactly as in a production single-app_data layout.
    for sub in ["bin", "lib", "llm-engines", "lit-cache"] {
        let target = shared.join(sub);
        fs::create_dir_all(&target).ok();
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&target, td.path().join(sub));
        }
    }
    td
}

// Per-test DBs are cloned from a fully-migrated TEMPLATE via
// `CREATE DATABASE ... TEMPLATE`, so migrations run exactly ONCE per test
// process instead of once per test — eliminating the per-test migration races
// that broke parallel runs (a half-applied schema → "relation does not exist")
// and making DB setup dramatically faster (a byte-copy vs replaying 118
// migrations per test).

/// True when this harness is compiled into the `ziee-desktop` crate's test
/// binary (vs the server crate's). Decided at compile time via the package
/// name, so the SAME `#[path]`-shared source picks the right migration set
/// and template name for whichever crate it's built into.
fn is_desktop() -> bool {
    env!("CARGO_PKG_NAME") == "ziee-desktop"
}

/// Name of the fully-migrated TEMPLATE database. The desktop and server test
/// binaries use DISTINCT names so they never clobber each other's template
/// even when run against the same Postgres (the desktop template carries the
/// extra 5 desktop migrations on top of the server's 118).
fn test_template_db() -> String {
    let base = if is_desktop() {
        "ziee_test_template_desktop"
    } else {
        "ziee_test_template"
    };
    // Suffix with the per-worktree key so concurrent suites in different
    // worktrees (sharing the same :54321 cluster) never DROP/CREATE the same
    // template database out from under each other.
    format!("{base}{}", worktree_suffix())
}

/// Ordered migration directories to apply when building the template.
/// Server build: just the server's `migrations/`. Desktop build: the server's
/// 118 migrations FIRST (resolved relative to the desktop crate manifest),
/// THEN the desktop crate's 5 — mirroring the real desktop boot path in
/// `src-app/desktop/tauri/src/lib.rs`.
fn template_migration_dirs() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if is_desktop() {
        vec![
            manifest.join("../../server/migrations"),
            manifest.join("migrations"),
        ]
    } else {
        vec![manifest.join("migrations")]
    }
}

static TEST_TEMPLATE: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// Build the migrated template DB exactly once per process (the OnceCell makes
/// every concurrent test await the single build before any of them clone). The
/// template must have NO active connections when a clone runs, so we close our
/// pools and terminate any stragglers before returning.
async fn ensure_test_template(admin_url: &str) {
    TEST_TEMPLATE
        .get_or_init(|| async {
            let admin = PgPoolOptions::new()
                .max_connections(1)
                .connect(admin_url)
                .await
                .expect("connect postgres to build test template");
            let template_db = test_template_db();
            let term = format!(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{template_db}' AND pid <> pg_backend_pid()"
            );
            let _ = sqlx::query(&term).execute(&admin).await;
            // Rebuild fresh each process so migration changes are picked up.
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {template_db}"))
                .execute(&admin)
                .await;
            sqlx::query(&format!("CREATE DATABASE {template_db}"))
                .execute(&admin)
                .await
                .expect("create test template database");
            admin.close().await;

            // Migrate the template at RUNTIME from the on-disk migration dirs.
            // We deliberately do NOT use the compile-time crate-relative
            // `sqlx::migrate!("./migrations")` macro: compiled into the
            // `ziee-desktop` test binary it resolves to the desktop crate's
            // 5-migration dir and misses the server's 118 (every desktop
            // integration test then failed with `relation
            // "user_group_llm_providers" does not exist`). The runtime
            // Migrator lets the desktop build apply server-then-desktop.
            let mut tmpl = url::Url::parse(admin_url).expect("admin url");
            tmpl.set_path(&template_db);
            let tmpl_pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(tmpl.as_str())
                .await
                .expect("connect template database");
            for dir in template_migration_dirs() {
                let mut migrator = sqlx::migrate::Migrator::new(dir.clone())
                    .await
                    .unwrap_or_else(|e| {
                        panic!("create migrator for {}: {e}", dir.display())
                    });
                // Desktop migrations carry version numbers far above the
                // server's; ignore-missing lets each migrator run against a
                // DB that already has the other set applied.
                migrator.set_ignore_missing(true);
                migrator
                    .run(&tmpl_pool)
                    .await
                    .unwrap_or_else(|e| {
                        panic!("migrate test template from {}: {e}", dir.display())
                    });
            }
            tmpl_pool.close().await;

            // Drop any lingering backend on the template so clones can copy it.
            let admin2 = PgPoolOptions::new()
                .max_connections(1)
                .connect(admin_url)
                .await
                .expect("connect postgres to quiesce template");
            let _ = sqlx::query(&term).execute(&admin2).await;
            admin2.close().await;
        })
        .await;
}

/// Options for spinning up a TestServer with non-default features.
///
/// Default = `code_sandbox.enabled: false`, matching legacy behavior.
/// Tier-6 HTTP-E2E tests opt into sandbox enablement via
/// `start_with_options`; the test MUST call
/// `harness::skip_if_no_rootfs()` first to skip cleanly when bwrap or
/// the rootfs aren't available on this host.
#[derive(Debug, Clone, Default)]
pub struct TestServerOptions {
    /// Enable code_sandbox in the test server's config. Requires bwrap
    /// installed AND `sandbox_rootfs` pointing at a mounted rootfs.
    pub sandbox_enabled: bool,
    /// Rootfs path written into the test config when sandbox_enabled.
    /// Tests should pass `harness::rootfs_path()`.
    pub sandbox_rootfs: Option<PathBuf>,
    /// Cgroup parent path. Empty = rlimits-only mode (no cgroup
    /// delegation needed, runs fine in plain containers / dev shells).
    /// Tests that need cgroup enforcement should set this AND call
    /// `harness::needs_cgroup_delegation()` first.
    pub sandbox_cgroup_parent: String,
    /// Extra env vars to set on the spawned server process (e.g.
    /// `ANTHROPIC_API_KEY` for Tier-5 LLM tests). Honored verbatim;
    /// the test is responsible for unsetting sensitive ones it didn't
    /// intend to expose.
    pub extra_env: Vec<(String, String)>,
    /// Tier-6 only: the staged-rootfs cache TempDir whose lifetime
    /// must exceed the spawned server's. Set by
    /// `harness::enabled_test_server()` on Mac/Windows (where the
    /// rootfs is staged inline into a TempDir + fake known_revisions
    /// TOML); unset on Linux (the rootfs is a real FUSE mount owned
    /// by the operator). TestServer holds it alive via Arc so this
    /// struct stays Clone; drops on TestServer::Drop.
    pub sandbox_cache_tempdir: Option<std::sync::Arc<tempfile::TempDir>>,
    /// Spawn the `ziee-desktop --headless` binary instead of the
    /// server-only `ziee` binary. Required for tests that exercise
    /// HTTP routes owned by the desktop crate (remote_access,
    /// magic_link, tunnel_auth). See
    /// `desktop/tauri/tests/remote_access/*.rs`.
    pub use_desktop_binary: bool,
    /// Override the global rate limiter: `(enabled, per_second, burst_size)`.
    /// `None` (default) keeps the very-high test caps so a sequential test
    /// sweep against the single 127.0.0.1 peer-IP bucket never self-429s.
    /// The rate-limit regression test sets this to small or disabled values
    /// to exercise the governor on/off behavior.
    pub rate_limit: Option<(bool, u64, u32)>,
    /// Override `code_sandbox.public_base_url` in the test config. Only
    /// written when `sandbox_enabled` is also true (it lives under the
    /// `code_sandbox:` section). Lets a test assert that file/resource links
    /// are rooted at a reachable public origin instead of the loopback.
    pub sandbox_public_base_url: Option<String>,
    /// Server self-update check. Defaults to OFF (`None` → `enabled: false`) so
    /// no test-server boot makes a live api.github.com call. The mock-GitHub
    /// test sets `Some(true)` + a `SERVER_UPDATE_API_MIRROR` in `extra_env`.
    pub update_check_enabled: Option<bool>,
    /// Enable the `bio_mcp` built-in MCP server in the test config. Defaults
    /// to FALSE so the (production-default-ON) BioMCP sidecar never spawns
    /// during unrelated tests and chat tests don't auto-attach it; bio tests
    /// opt in explicitly. The `BIO_MCP_SIDECAR_URL` debug seam (set via
    /// `extra_env`) lets a test point the proxy at a mock sidecar.
    pub bio_mcp_enabled: bool,
    /// Deploy-level kill-switch for the `control_mcp` built-in. `None` omits the
    /// config section (module default = enabled). `Some(false)` disables the
    /// whole control surface (no MCP row, no route).
    pub control_mcp_enabled: Option<bool>,
    /// DEBUG-ONLY seconds-granularity access-token TTL, written as
    /// `jwt.access_token_expiry_seconds` in the test config. Lets a test
    /// exercise real token expiry in seconds instead of hours (the seam
    /// is honored only under `cfg!(debug_assertions)`, which the test
    /// server binary is). `None` omits the line (24h default).
    pub access_token_expiry_seconds: Option<i64>,
    /// Override `jwt.refresh_token_expiry_days` in the test config
    /// (seed value for `session_settings` on first boot).
    pub refresh_token_expiry_days: Option<i64>,
    /// Override `server.max_file_upload_mb` in the test config. Lets upload
    /// boundary tests spawn a server with a tiny per-file cap (e.g. `Some(1)`)
    /// and exercise accept/reject with KB-to-low-MB bodies instead of allocating
    /// the 128 MiB default. `None` omits the line (server default = 128).
    pub max_file_upload_mb: Option<u64>,
}

impl TestServer {
    /// Start a TestServer with the default options (sandbox disabled).
    /// Equivalent to `start_with_options(TestServerOptions::default())`.
    /// Used by the ziee server tests but not the desktop tests (which use
    /// `start_desktop`), so it reads as dead on the desktop build — keep it.
    #[allow(dead_code)]
    pub async fn start() -> Self {
        Self::start_with_options(TestServerOptions::default()).await
    }

    /// Start a TestServer that spawns `ziee-desktop --headless`
    /// instead of the server-only `ziee` binary. Required for tests
    /// that exercise routes owned by the desktop crate.
    ///
    /// Used cross-crate by the `ziee-desktop` integration tests, so it
    /// appears unused from the `ziee` crate's own build — keep it.
    #[allow(dead_code)]
    pub async fn start_desktop() -> Self {
        Self::start_with_options(TestServerOptions {
            use_desktop_binary: true,
            ..Default::default()
        })
        .await
    }

    /// Start a TestServer with the given options. Use this when a test
    /// needs the code_sandbox enabled or wants to inject extra env.
    pub async fn start_with_options(opts: TestServerOptions) -> Self {
        // Initialise the at-rest secret storage_key in the *test* process
        // too. The spawned server process initialises its own key from
        // the YAML config, but tests that construct repositories
        // directly against the test DB pool (UserKeyRepository,
        // LlmRepositoryRepository) decrypt rows in-process and need the
        // key to be available via ziee::storage_key(). Idempotent —
        // OnceCell::set after first call is a noop.
        ziee::init_storage_key(Some(
            "test-storage-key-for-pgcrypto-min-32-chars-long".to_string(),
        ));

        // Windows: the WSL2 sandbox backend resolves the utility-VM id through
        // the LocalSystem helper service. Auto-install it (elevated) the first
        // time a sandbox-enabled test runs, so the tier6/tier8 suites "just
        // run" without a manual admin step. No-op on a machine that already
        // has it (or has ZIEE_WSL_VM_ID set).
        #[cfg(windows)]
        if opts.sandbox_enabled {
            ensure_sandbox_helper_for_tests();
        }

        // Generate unique identifiers
        let test_id = Uuid::new_v4().to_string();
        let database_name = format!("test_db_{}", test_id.replace("-", "_"));
        // Use OS-aware port reservation instead of a random pick.
        // The previous `rand::rng().random_range(10000..60000)`
        // collided with OTHER listeners (system services, prior
        // TestServers in TIME_WAIT, parallel test harnesses) and the
        // resulting "Address already in use" left the server unable
        // to bind → health-poll timeout → TestServer panicked. Closes
        // the 19-of-29 boot-timeout cluster in the diagnostic run.
        let server_port = portpicker::pick_unused_port()
            .expect("No free TCP port available for TestServer");

        // Parse DATABASE_URL to extract connection details
        let db_url = database_url();
        let url = url::Url::parse(&db_url).expect("Invalid DATABASE_URL");

        let host = url.host_str().unwrap_or("127.0.0.1");
        let port = url.port().unwrap_or(54321);
        let username = url.username();
        let password = url.password().unwrap_or("");

        // Shared cache dir (extractions persist across runs); the path
        // resolution layer in Config::resolve_paths derives every
        // subdir from this. Tests inherit:
        //   - <shared>/bin/{pandoc,libpdfium,uv,bun} extracted once
        //   - <shared>/{bin,lib,share,etc}/* sandbox-runtime extracted once
        //   - <shared>/sandboxes/  ← SHARED across tests, but EVERY
        //     sandbox-enabled test overrides workspace_root below to a
        //     per-test TempDir for isolation. Tests that don't enable
        //     the sandbox don't touch this dir.
        // Per-test isolated data_dir (mutable state fresh per test; binary
        // caches symlinked-in shared). This is what lets the suite run without
        // `--test-threads=1`. Held on TestServer so its tree is reaped at end.
        let data_tempdir = make_isolated_data_dir();
        let data_dir_path = data_tempdir.path().to_path_buf();

        // Rate-limit override: default to very-high caps so sequential test
        // sweeps against the single 127.0.0.1 bucket don't self-429; the
        // rate-limit regression test passes explicit small/disabled values.
        let (rl_enabled, rl_per_sec, rl_burst) =
            opts.rate_limit.unwrap_or((true, 10000, 10000));

        // Create test config for the server.
        //
        // Path values are written with SINGLE quotes (YAML's "flow scalar")
        // because Windows paths contain backslashes — `C:\Users\...` —
        // and YAML's double-quoted scalars interpret `\` as an escape
        // (e.g. `\U` starts a Unicode escape that demands hex digits and
        // fails to parse). Single-quoted YAML scalars treat backslashes
        // literally, so any host-OS path string round-trips correctly.
        let mut config = format!(
            r#"
app:
  data_dir: '{shared}'

postgresql:
  use_embedded: false

  external:
    host: "{}"
    port: {}
    username: "{}"
    password: "{}"
    database: "{}"

  pool:
    max_connections: 5
    min_connections: 1
    acquire_timeout_secs: 3
    idle_timeout_secs: 10
    max_lifetime_secs: 60

server:
  host: "127.0.0.1"
  port: {}
  api_prefix: "/api"
  # Tests run many sequential requests against a single peer IP
  # (127.0.0.1), so they share one tower-governor bucket. The
  # production default (5 req/s, burst 60) self-429s under sustained
  # test load. Set extremely high caps here — the global cap is
  # still exercised via the dedicated A3 rate-limit regression test
  # which sets its own low values.
  rate_limit:
    enabled: {rl_enabled}
    per_second: {rl_per_sec}
    burst_size: {rl_burst}
  # OAuth tests drive flows against the testcontainer mock; the
  # reqwest client doesn't set X-Forwarded-* so the backend derives
  # redirect_uri from HOST. The flag's value doesn't matter for
  # tests except that we want to exercise the default-safe path.
  trust_forwarded_headers: false
{max_upload}
jwt:
  # Must match the production issuer/audience because the MCP client
  # (modules/mcp/client/manager.rs) hardcodes these values when minting
  # JWTs for built-in MCP servers (code_sandbox loopback). If the
  # TestServer used different values, the validator (JwtService) would
  # reject the MCP client's tokens with InvalidIssuer and Tier-5 tests
  # (LLM → sandbox via MCP) would fail with "no tools available".
  secret: "test-secret-key-for-jwt-tokens-min-32-chars-long"
  issuer: "ziee"
  audience: "ziee-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: {refresh_days}
{access_seconds}
# At-rest secret storage key — enables pgcrypto encryption on api_key /
# token / password columns. See common/secret.rs. Closes 06-llm-provider
# F-02 once the repository wiring lands.
secrets:
  storage_key: "test-storage-key-for-pgcrypto-min-32-chars-long"
"#,
            host, port, username, password, database_name, server_port,
            shared = data_dir_path.display(),
            rl_enabled = rl_enabled,
            rl_per_sec = rl_per_sec,
            rl_burst = rl_burst,
            refresh_days = opts.refresh_token_expiry_days.unwrap_or(30),
            access_seconds = opts
                .access_token_expiry_seconds
                .map(|s| format!("  access_token_expiry_seconds: {s}\n"))
                .unwrap_or_default(),
            max_upload = opts
                .max_file_upload_mb
                .map(|mb| format!("  max_file_upload_mb: {mb}\n"))
                .unwrap_or_default(),
        );

        // Optional code_sandbox section. Only written when the test
        // explicitly opts in; otherwise the server boots with sandbox
        // disabled (the default behavior every existing test relies on).
        let (workspace_tempdir, _) = if opts.sandbox_enabled {
            let rootfs = opts
                .sandbox_rootfs
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| {
                    panic!(
                        "TestServerOptions.sandbox_enabled=true requires \
                         sandbox_rootfs to be set. Call \
                         harness::rootfs_path() and skip the test if it \
                         returns None."
                    )
                });
            // Per-test workspace_root override. Each sandbox-enabled test
            // gets a fresh TempDir; held on TestServer so Drop reaps it.
            // Without this, parallel sandbox tests would race on the
            // shared `<app_data>/sandboxes/` dir created by the harness's
            // shared app.data_dir.
            let ws = tempfile::tempdir().expect("workspace TempDir");
            let ws_path = ws.path().to_path_buf();
            // On Mac, the sandbox-runtime guest VM accesses this dir
            // via virtio-fs as `--unshare-user --uid 1001`; uid 1001's
            // write attempts on the host fs need permissive mode.
            // tempfile defaults to 0700 (owner-only). Same chmod also
            // applies on Linux without harm.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    &ws_path,
                    std::fs::Permissions::from_mode(0o1777),
                );
            }
            // Single-quote the path values (see note above the `data_dir`
            // formatter); Windows paths contain backslashes that break
            // YAML double-quoted scalars.
            // require_download_consent: false — tests drive execute_command
            // over a one-shot HTTP call and can't answer the interactive
            // "download this ~900 MB environment?" consent elicitation. With
            // consent on, a large (uncached) flavor like `full` would block on
            // the elicitation for CONSENT_TIMEOUT_SECS (600s) and then decline,
            // so the rootfs never downloads. Auto-download instead.
            config.push_str(&format!(
                "\ncode_sandbox:\n  enabled: true\n  rootfs_path: '{}'\n  workspace_root: '{}'\n  cgroup_parent: '{}'\n  require_download_consent: false\n",
                rootfs,
                ws_path.display(),
                opts.sandbox_cgroup_parent
            ));
            if let Some(public_base_url) = opts.sandbox_public_base_url.as_deref() {
                config.push_str(&format!("  public_base_url: '{public_base_url}'\n"));
            }
            (Some(ws), Some(ws_path))
        } else {
            (None, None)
        };

        // Write temporary config file (cross-platform: `/tmp/...` doesn't
        // exist on Windows, so use `std::env::temp_dir()` which resolves
        // to `%TEMP%` on Windows and `/tmp` on Unix).
        let temp_config_path = std::env::temp_dir().join(format!("ziee-test-{test_id}.yaml"));
        // Server self-update check — OFF by default so no test boot calls
        // api.github.com (the mock test opts in via update_check_enabled).
        let update_check_enabled = opts.update_check_enabled.unwrap_or(false);
        config.push_str(&format!(
            "\nupdate_check:\n  enabled: {update_check_enabled}\n"
        ));

        // bio_mcp defaults ON in production but OFF in tests (see field
        // doc) — write the section explicitly so the test server never
        // spawns the BioMCP sidecar unless a bio test opts in.
        config.push_str(&format!(
            "\nbio_mcp:\n  enabled: {}\n",
            opts.bio_mcp_enabled
        ));

        // control_mcp defaults ON; only write the section when a test overrides
        // it (the kill-switch test sets Some(false)).
        if let Some(control_enabled) = opts.control_mcp_enabled {
            config.push_str(&format!("\ncontrol_mcp:\n  enabled: {control_enabled}\n"));
        }

        fs::write(&temp_config_path, config).expect("Failed to write temporary config");

        // Ensure the fully-migrated template exists (built once per process),
        // then clone the per-test DB from it — no migrations run per test, so
        // nothing races and there's no half-applied schema under parallelism.
        ensure_test_template(&db_url).await;

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to PostgreSQL - ensure docker compose is running");

        sqlx::query(&format!(
            "CREATE DATABASE {} TEMPLATE {}",
            database_name,
            test_template_db()
        ))
        .execute(&pool)
        .await
        .expect("Failed to create test database from template");

        pool.close().await;

        // Start the server process with the temporary config. Windows
        // appends `.exe`; cargo emits both `ziee` (the artifact stem,
        // present as a hard link on Unix) and `ziee.exe` on Windows.
        // The workspace refactor moved target/ to the parent dir
        // (`src-app/target` rather than `src-app/server/target`), so
        // resolve relative to CARGO_MANIFEST_DIR's parent.
        // Pick binary: server-only `ziee` (default) or
        // `ziee-desktop --headless` (tests for routes owned by the
        // desktop crate — remote_access, magic_link, tunnel_auth).
        let exe_stem = if opts.use_desktop_binary {
            "ziee-desktop"
        } else {
            "ziee"
        };
        let exe_name = if cfg!(windows) {
            format!("{}.exe", exe_stem)
        } else {
            exe_stem.to_string()
        };
        // Walk up from CARGO_MANIFEST_DIR looking for `target/debug/<exe>`.
        // - Server crate test: manifest=src-app/server, parent=src-app ✓
        // - Desktop crate test: manifest=src-app/desktop/tauri,
        //   parent=src-app/desktop (no target), grandparent=src-app ✓
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let binary_path = {
            let candidates = [
                manifest.parent().map(|p| p.join("target/debug").join(&exe_name)),
                manifest
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.join("target/debug").join(&exe_name)),
                Some(manifest.join("target/debug").join(&exe_name)),
            ];
            candidates
                .into_iter()
                .flatten()
                .find(|p| p.exists())
                .unwrap_or_else(|| manifest.join("target/debug").join(&exe_name))
        };

        // Isolate the hub catalog dir per test. The hub catalog
        // (`<app_data>/hub/current/`) is durable global state shared
        // across the run's shared app_data dir; a refresh/activate in
        // one test would otherwise rotate it and break every other
        // test that reads the seed. ZIEE_HUB_DATA_DIR_OVERRIDE is
        // debug-gated (compiled out of release). Held until Drop.
        let hub_tempdir = tempfile::tempdir().expect("create per-test hub dir");

        let mut cmd = Command::new(&binary_path);
        cmd.arg("--config-file").arg(&temp_config_path);
        if opts.use_desktop_binary {
            cmd.arg("--headless");
        }
        cmd.env("ZIEE_HUB_DATA_DIR_OVERRIDE", hub_tempdir.path());
        for (k, v) in &opts.extra_env {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("Failed to start test server");

        // Construct base URL
        let base_url = format!("http://127.0.0.1:{}", server_port);

        // Construct database URL for the test database
        let test_database_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, password, host, port, database_name
        );

        // Wait for server to be ready. Bumped from 30 × 200ms = 6s to
        // 150 × 200ms = 30s after observing test failures where the
        // server boot ran past 6s on a busy CI/dev box. The added
        // security middleware stack (rate-limit init, security
        // headers, etc) + module registration + migration apply +
        // external Postgres connect all add up — 30s is a safe
        // ceiling that still surfaces a genuinely-hung server.
        let client = reqwest::Client::new();
        let health_url = format!("{}/api/health", base_url);

        let mut ready = false;
        for _ in 0..150 {
            if let Ok(response) = client.get(&health_url).send().await
                && response.status().is_success() {
                    ready = true;
                    break;
                }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if !ready {
            panic!(
                "TestServer at {} did not become healthy within 30s",
                base_url
            );
        }

        TestServer {
            process: child,
            base_url,
            database_name,
            database_url: test_database_url,
            temp_config_path,
            _workspace_tempdir: workspace_tempdir,
            // workspace_root field removed — see struct doc
            _sandbox_cache_tempdir: opts.sandbox_cache_tempdir.clone(),
            _hub_tempdir: hub_tempdir,
            _data_tempdir: data_tempdir,
        }
    }

    /// Get the base URL for API requests
    pub fn api_url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }

    /// The spawned server process's per-test `app.data_dir`.
    ///
    /// An in-process test that drives a file-reading helper directly
    /// (e.g. `ziee::file_routing::process_file_blocks`, which calls the
    /// process-global `get_file_storage()`) must point that global at the
    /// SAME directory the spawned server wrote the HTTP-uploaded bytes to:
    /// `init_file_storage(server.data_dir().join("files"))`. The file store's
    /// base path is `<app_data_dir>/files` (see `file::mod` init).
    #[allow(dead_code)] // used by server integration tests; dead in the desktop test binary that reincludes this harness
    pub fn data_dir(&self) -> &std::path::Path {
        self._data_tempdir.path()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Kill the server process
        let _ = self.process.kill();
        let _ = self.process.wait();

        // Delete the temporary config file
        let _ = fs::remove_file(&self.temp_config_path);

        // Cleanup database
        let database_name = self.database_name.clone();
        let db_url = database_url();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let _ = handle.spawn(async move {
                if let Ok(pool) = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&db_url)
                    .await
                {
                    // Terminate existing connections
                    let _ = sqlx::query(&format!(
                        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> pg_backend_pid()",
                        database_name
                    ))
                    .execute(&pool)
                    .await;

                    // Drop the database
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {}", database_name))
                        .execute(&pool)
                        .await;

                    pool.close().await;
                }
            });
        }
    }
}

/// Common test helpers for creating users and managing permissions
pub mod test_helpers {
    use super::TestServer;
    use serde_json::json;
    use uuid::Uuid;

    /// Test user with token and ID
    #[derive(Debug, Clone)]
    pub struct TestUser {
        pub token: String,
        pub user_id: String,
    }

    /// Create a user with specific permissions for testing
    pub async fn create_user_with_permissions(
        server: &TestServer,
        username: &str,
        permissions: &[&str],
    ) -> TestUser {
        let unique_username = format!("{}_{}", username, &Uuid::new_v4().to_string()[..8]);

        // Register user via API
        let register_response = reqwest::Client::new()
            .post(server.api_url("/auth/register"))
            .json(&json!({
                "username": &unique_username,
                "email": format!("{}@example.com", unique_username),
                "password": "password123"
            }))
            .send()
            .await
            .expect("Failed to register user");

        assert_eq!(
            register_response.status(),
            201,
            "Registration should succeed"
        );

        let register_body: serde_json::Value = register_response
            .json()
            .await
            .expect("Failed to parse register response");

        let token = register_body["access_token"]
            .as_str()
            .expect("access_token missing")
            .to_string();
        let user_id = register_body["user"]["id"]
            .as_str()
            .expect("user id missing")
            .to_string();

        // Assign permissions if needed
        if !permissions.is_empty() {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&server.database_url)
                .await
                .expect("Failed to connect to test database");

            let group_id = Uuid::new_v4();
            let group_name = format!("test_group_{}", &group_id.to_string()[..8]);
            let permissions_json: Vec<String> = permissions.iter().map(|s| s.to_string()).collect();

            sqlx::query(
                "INSERT INTO groups (id, name, description, permissions, is_system, is_active, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, false, true, NOW(), NOW())"
            )
            .bind(group_id)
            .bind(&group_name)
            .bind("Test group for permissions")
            .bind(&permissions_json)
            .execute(&pool)
            .await
            .expect("Failed to create test group");

            // Assign user to custom permissions group
            let user_uuid = Uuid::parse_str(&user_id).expect("Invalid user ID");
            sqlx::query(
                "INSERT INTO user_groups (user_id, group_id, assigned_at)
                 VALUES ($1, $2, NOW())",
            )
            .bind(user_uuid)
            .bind(group_id)
            .execute(&pool)
            .await
            .expect("Failed to assign user to custom group");

            // Also assign user to default group (like real registration does)
            let default_group_result =
                sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
                    .fetch_optional(&pool)
                    .await
                    .expect("Failed to query default group");

            if let Some(default_group) = default_group_result {
                sqlx::query(
                    "INSERT INTO user_groups (user_id, group_id, assigned_at)
                     VALUES ($1, $2, NOW())
                     ON CONFLICT DO NOTHING",
                )
                .bind(user_uuid)
                .bind(default_group.id)
                .execute(&pool)
                .await
                .expect("Failed to assign user to default group");
            }

            pool.close().await;
        }

        TestUser { token, user_id }
    }

    /// Create a user that has NO permissions at all — including no membership
    /// in the default "Users" group (which grants `mcp_servers::*`, `chat::*`,
    /// etc. via migration 27).
    ///
    /// Use this for route-level "should return 403" tests where you need to
    /// prove the authorization gate works. `create_user_with_permissions(_, _, &[])`
    /// is NOT suitable — registration auto-assigns the default group, so the
    /// resulting user actually has a broad set of inherited permissions.
    pub async fn create_user_with_no_permissions(
        server: &TestServer,
        username: &str,
    ) -> TestUser {
        let user = create_user_with_permissions(server, username, &[]).await;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&server.database_url)
            .await
            .expect("Failed to connect to test database");

        let user_uuid = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
        sqlx::query("DELETE FROM user_groups WHERE user_id = $1")
            .bind(user_uuid)
            .execute(&pool)
            .await
            .expect("Failed to strip user from groups");

        pool.close().await;
        user
    }

    /// Create a user with EXACTLY the listed permissions — no default-group
    /// inheritance. Use when a test needs to prove "X works with perm A,
    /// fails without perm B" but B is in the default Users group too.
    /// `create_user_with_permissions(_, _, &["A"])` would leave the user
    /// in default + add a separate group with [A], giving them both A
    /// AND every default permission.
    // Shared cross-crate harness: used by the ziee server tests but not the
    // desktop tests, so it reads as dead on the desktop build — keep it.
    #[allow(dead_code)]
    pub async fn create_user_with_only_permissions(
        server: &TestServer,
        username: &str,
        permissions: &[&str],
    ) -> TestUser {
        let user = create_user_with_permissions(server, username, permissions).await;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&server.database_url)
            .await
            .expect("Failed to connect to test database");

        let user_uuid = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
        // Drop user from the system default group; leave them in the
        // per-test group created above (which carries the explicit
        // permission list).
        sqlx::query(
            "DELETE FROM user_groups WHERE user_id = $1 AND group_id IN (\
                SELECT id FROM groups WHERE is_default = true\
             )",
        )
        .bind(user_uuid)
        .execute(&pool)
        .await
        .expect("Failed to strip user from default group");

        pool.close().await;
        user
    }

    /// Create a test user via API (requires admin token)
    // Shared cross-crate harness: used by the ziee server tests but not the
    // desktop tests, so it reads as dead on the desktop build — keep it.
    #[allow(dead_code)]
    pub async fn create_test_user(
        server: &TestServer,
        admin_token: &str,
        username: &str,
        password: &str,
    ) -> serde_json::Value {
        let url = server.api_url("/users");
        let payload = json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": password
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", admin_token))
            .json(&payload)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201, "Failed to create test user");
        response.json().await.expect("Failed to parse JSON")
    }
}

// http helper module removed — all functions were unused.
