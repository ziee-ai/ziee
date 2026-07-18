//! Binary-agnostic TestServer harness (thin app-side shim).
//!
//! The GENERIC spawn/isolation/DB-clone/Drop engine now lives in the SDK crate
//! `ziee-test-harness` (it names ONLY the `HarnessApp` seam, never `ziee`). This
//! file is ziee's `HarnessApp` implementation plus a thin `TestServer` wrapper
//! that re-exposes the historical surface (`TestServer::start*`,
//! `TestServerOptions`, `test_helpers`) with identical names + signatures, so
//! the ~288 server + desktop test files compile UNCHANGED.
//!
//! Shared between the server crate's integration_tests binary and the desktop
//! crate's integration_tests binary via `#[path]` reuse. Because it is
//! `#[path]`-compiled PER CRATE, `env!("CARGO_MANIFEST_DIR")` + the
//! `is_desktop()` probe below resolve to the CONSUMING crate correctly — which
//! is exactly why they must be evaluated HERE (in the shim) and passed into the
//! SDK harness as runtime values, never inside the compiled SDK crate. The
//! OAuth/LDAP/Apple mocks live alongside this file and are declared by the
//! server crate's `common/mod.rs` (NOT here) because they pull in heavy deps the
//! desktop crate doesn't want.

use std::any::Any;
use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

use ziee_test_harness::{HarnessApp, SpawnFacts, SpawnPlan, SpawnedServer, TestHarness, Variant};

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
    use std::process::Command;
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

/// True when this harness is compiled into the `ziee-desktop` crate's test
/// binary (vs the server crate's). Decided at compile time via the package
/// name — correct because this file is `#[path]`-compiled per crate — and used
/// to seed the runtime [`Variant`] the SDK harness keys the template name +
/// migration set off.
fn is_desktop() -> bool {
    env!("CARGO_PKG_NAME") == "ziee-desktop"
}

fn variant() -> Variant {
    if is_desktop() {
        Variant::Desktop
    } else {
        Variant::Server
    }
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
    /// Deploy-level kill-switch for the `voice` dictation runtime. `None` omits
    /// the config section (module default = enabled). `Some(false)` disables the
    /// whole voice surface (no routes mounted, no reaper) — mirrors
    /// `control_mcp_enabled`.
    pub voice_enabled: Option<bool>,
}

/// Ziee's `HarnessApp` implementation — supplies the app-specific couplings the
/// generic SDK harness needs: the binary name, the template DB base + migration
/// dirs, the pre-spawn side effects (storage-key init + Windows helper), and the
/// full config-YAML render.
struct ZieeApp;

impl HarnessApp for ZieeApp {
    type Options = TestServerOptions;

    fn template_db_base(&self, variant: Variant) -> String {
        // The desktop + server test binaries use DISTINCT names so they never
        // clobber each other's template even against the same Postgres (the
        // desktop template carries the extra desktop migrations on top of the
        // server's).
        match variant {
            Variant::Desktop => "ziee_test_template_desktop".to_string(),
            Variant::Server => "ziee_test_template".to_string(),
        }
    }

    fn migration_dirs(&self, variant: Variant, manifest_dir: &std::path::Path) -> Vec<PathBuf> {
        // Server build: the composed `migrations-merged/`. Desktop build: the
        // server's merged set FIRST (resolved relative to the desktop crate
        // manifest), THEN the desktop crate's own `migrations/` — mirroring the
        // real desktop boot path in `src-app/desktop/tauri/src/lib.rs`.
        match variant {
            Variant::Desktop => vec![
                manifest_dir.join("../../server/migrations-merged"),
                manifest_dir.join("migrations"),
            ],
            Variant::Server => vec![manifest_dir.join("migrations-merged")],
        }
    }

    fn before_spawn(&self, opts: &TestServerOptions) {
        // Initialise the at-rest secret storage_key in the *test* process too.
        // The spawned server process initialises its own key from the YAML
        // config, but tests that construct repositories directly against the
        // test DB pool (UserKeyRepository, LlmRepositoryRepository) decrypt rows
        // in-process and need the key available via ziee::storage_key().
        // Idempotent — OnceCell::set after first call is a noop.
        ziee::init_storage_key(Some(
            "test-storage-key-for-pgcrypto-min-32-chars-long".to_string(),
        ));

        // Windows: the WSL2 sandbox backend resolves the utility-VM id through
        // the LocalSystem helper service. Auto-install it (elevated) the first
        // time a sandbox-enabled test runs.
        #[cfg(windows)]
        if opts.sandbox_enabled {
            ensure_sandbox_helper_for_tests();
        }
        // Silence the unused-var warning on non-Windows.
        let _ = opts;
    }

    fn plan_spawn(&self, opts: &TestServerOptions, facts: &SpawnFacts) -> SpawnPlan {
        let mut keep_alive: Vec<Box<dyn Any + Send + Sync>> = Vec::new();

        // Rate-limit override: default to very-high caps so sequential test
        // sweeps against the single 127.0.0.1 bucket don't self-429; the
        // rate-limit regression test passes explicit small/disabled values.
        let (rl_enabled, rl_per_sec, rl_burst) = opts.rate_limit.unwrap_or((true, 10000, 10000));

        // Create test config for the server.
        //
        // Path values are written with SINGLE quotes (YAML's "flow scalar")
        // because Windows paths contain backslashes — `C:\Users\...` — and
        // YAML's double-quoted scalars interpret `\` as an escape. Single-quoted
        // scalars treat backslashes literally, so any host-OS path round-trips.
        let mut config = format!(
            r#"
app:
  data_dir: '{shared}'

postgresql:
  use_embedded: false

  external:
    host: "{host}"
    port: {port}
    username: "{username}"
    password: "{password}"
    database: "{database}"

  pool:
    max_connections: 5
    min_connections: 1
    acquire_timeout_secs: 3
    idle_timeout_secs: 10
    max_lifetime_secs: 60

server:
  host: "127.0.0.1"
  port: {server_port}
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
            shared = facts.data_dir.display(),
            host = facts.db.host,
            port = facts.db.port,
            username = facts.db.username,
            password = facts.db.password,
            database = facts.database_name,
            server_port = facts.server_port,
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

        // Optional code_sandbox section. Only written when the test explicitly
        // opts in; otherwise the server boots with sandbox disabled.
        if opts.sandbox_enabled {
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
            // Per-test workspace_root override. Each sandbox-enabled test gets a
            // fresh TempDir; held via keep_alive so Drop reaps it. Without this,
            // parallel sandbox tests would race on the shared
            // `<app_data>/sandboxes/` dir.
            let ws = tempfile::tempdir().expect("workspace TempDir");
            let ws_path = ws.path().to_path_buf();
            // On Mac, the sandbox-runtime guest VM accesses this dir via
            // virtio-fs as `--unshare-user --uid 1001`; uid 1001's write
            // attempts on the host fs need permissive mode. Same chmod applies
            // on Linux without harm.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    &ws_path,
                    std::fs::Permissions::from_mode(0o1777),
                );
            }
            // require_download_consent: false — tests drive execute_command over
            // a one-shot HTTP call and can't answer the interactive consent
            // elicitation.
            config.push_str(&format!(
                "\ncode_sandbox:\n  enabled: true\n  rootfs_path: '{}'\n  workspace_root: '{}'\n  cgroup_parent: '{}'\n  require_download_consent: false\n",
                rootfs,
                ws_path.display(),
                opts.sandbox_cgroup_parent
            ));
            if let Some(public_base_url) = opts.sandbox_public_base_url.as_deref() {
                config.push_str(&format!("  public_base_url: '{public_base_url}'\n"));
            }
            keep_alive.push(Box::new(ws));
        }

        // Server self-update check — OFF by default so no test boot calls
        // api.github.com (the mock test opts in via update_check_enabled).
        let update_check_enabled = opts.update_check_enabled.unwrap_or(false);
        config.push_str(&format!("\nupdate_check:\n  enabled: {update_check_enabled}\n"));

        // bio_mcp defaults ON in production but OFF in tests (see field doc) —
        // write the section explicitly so the test server never spawns the
        // BioMCP sidecar unless a bio test opts in.
        config.push_str(&format!("\nbio_mcp:\n  enabled: {}\n", opts.bio_mcp_enabled));

        // control_mcp defaults ON; only write the section when a test overrides
        // it (the kill-switch test sets Some(false)).
        if let Some(control_enabled) = opts.control_mcp_enabled {
            config.push_str(&format!("\ncontrol_mcp:\n  enabled: {control_enabled}\n"));
        }

        // voice defaults ON; only write the section when a test overrides it
        // (the voice config-gate test sets Some(false) to prove the deploy-level
        // kill switch unmounts the whole voice surface).
        if let Some(voice_enabled) = opts.voice_enabled {
            config.push_str(&format!("\nvoice:\n  enabled: {voice_enabled}\n"));
        }

        // Pick binary: server-only `ziee` (default) or `ziee-desktop --headless`
        // (tests for routes owned by the desktop crate).
        let binary_name = if opts.use_desktop_binary {
            "ziee-desktop".to_string()
        } else {
            "ziee".to_string()
        };
        let mut extra_argv = Vec::new();
        if opts.use_desktop_binary {
            extra_argv.push("--headless".to_string());
        }

        // Isolate the hub catalog dir per test. The hub catalog
        // (`<app_data>/hub/current/`) is durable global state shared across the
        // run's shared app_data dir; a refresh/activate in one test would
        // otherwise rotate it and break every other test that reads the seed.
        // ZIEE_HUB_DATA_DIR_OVERRIDE is debug-gated (compiled out of release).
        let hub_tempdir = tempfile::tempdir().expect("create per-test hub dir");
        let mut extra_env = vec![(
            "ZIEE_HUB_DATA_DIR_OVERRIDE".to_string(),
            hub_tempdir.path().to_string_lossy().to_string(),
        )];
        extra_env.extend(opts.extra_env.iter().cloned());
        keep_alive.push(Box::new(hub_tempdir));

        // Hold the Tier-6 staged-rootfs cache TempDir alive for the server's
        // lifetime (Arc so TestServerOptions stays Clone).
        if let Some(cache) = opts.sandbox_cache_tempdir.clone() {
            keep_alive.push(Box::new(cache));
        }

        SpawnPlan {
            config_yaml: config,
            binary_name,
            extra_argv,
            extra_env,
            keep_alive,
        }
    }
}

/// The process-global harness, seeded ONCE with this crate's
/// `env!("CARGO_MANIFEST_DIR")` (correct per `#[path]`-compiled crate) + the
/// runtime variant.
fn harness() -> &'static TestHarness<ZieeApp> {
    static HARNESS: OnceLock<TestHarness<ZieeApp>> = OnceLock::new();
    HARNESS.get_or_init(|| {
        TestHarness::new(
            ZieeApp,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            variant(),
        )
    })
}

/// A running test server. Thin wrapper over the SDK's `SpawnedServer` that
/// preserves the historical public field/method surface so every call site
/// compiles unchanged. The wrapped `SpawnedServer` owns the child process +
/// tempdirs and reaps them (incl. `DROP DATABASE`) when this struct drops.
pub struct TestServer {
    pub base_url: String,
    pub database_name: String,
    pub database_url: String,
    inner: SpawnedServer,
}

impl TestServer {
    /// Start a TestServer with the default options (sandbox disabled).
    /// Equivalent to `start_with_options(TestServerOptions::default())`.
    #[allow(dead_code)]
    pub async fn start() -> Self {
        Self::start_with_options(TestServerOptions::default()).await
    }

    /// Start a TestServer that spawns `ziee-desktop --headless` instead of the
    /// server-only `ziee` binary. Required for tests that exercise routes owned
    /// by the desktop crate.
    #[allow(dead_code)]
    pub async fn start_desktop() -> Self {
        Self::start_with_options(TestServerOptions {
            use_desktop_binary: true,
            ..Default::default()
        })
        .await
    }

    /// Start a TestServer with the given options. Use this when a test needs the
    /// code_sandbox enabled or wants to inject extra env.
    pub async fn start_with_options(opts: TestServerOptions) -> Self {
        let inner = harness().start(opts).await;
        TestServer {
            base_url: inner.base_url.clone(),
            database_name: inner.database_name.clone(),
            database_url: inner.database_url.clone(),
            inner,
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
        self.inner.data_dir()
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
