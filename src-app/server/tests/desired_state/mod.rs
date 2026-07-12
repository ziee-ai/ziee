//! Integration tests for the config-as-code desired-state reconciler.
//!
//! These drive the REAL boot path: a desired-state YAML is written to a temp
//! file and handed to the spawned `ziee` binary via `ZIEE_DESIRED_STATE_FILE`
//! (+ the `${VAR}` values) in `TestServerOptions::extra_env`, exactly as the
//! container does. No shared-harness change is needed.
//!
//! "A second deploy" is exercised by `reboot()`, which spawns the SAME binary a
//! second time against the SAME database and waits for it to become healthy —
//! i.e. the reconciler genuinely runs twice against one DB, which is what
//! idempotency means here. (Calling `reconcile()` in-process is NOT an option:
//! `core::init_repositories` installs a process-global factory, so an in-process
//! call would race the other tests sharing this test binary.)

use std::path::PathBuf;
use std::process::{Child, Command};

use crate::common::{TestServer, TestServerOptions};

// ───────────────────────────── fixtures ─────────────────────────────

const RCPA_URL: &str = "http://rcpa.test.internal:9101/mcp";
const DSCC_URL: &str = "http://dscc.test.internal:9102/mcp";
const BIOGNOSIA_URL: &str = "http://biognosia.test.internal:9103/mcp";
const ADMIN_PASSWORD: &str = "ds-admin-pw-1";
const USER_PASSWORD: &str = "ds-user-pw-1";

/// The shape of the file we ship, kept in one place so each test can tweak it.
fn manifest(mode: &str) -> String {
    format!(
        r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
    description: RCPA analysis tools
    url: ${{RCPA_MCP_URL}}
    enabled: true
    supports_sampling: false
    timeout_seconds: 300
    groups: [Users]
    mode: {mode}
  - name: dscc
    display_name: DSCC
    url: ${{DSCC_MCP_URL}}
    enabled: true
    supports_sampling: false
    timeout_seconds: 300
    groups: [Users]
    mode: {mode}
  - name: biognosia
    display_name: Biognosia
    url: ${{BIOGNOSIA_MCP_URL}}
    enabled: true
    supports_sampling: true
    groups: [Users]
    mode: {mode}

admin:
  username: admin
  email: admin@tinnguyen-lab.com
  display_name: Administrator
  password: ${{ZIEE_ADMIN_PASSWORD}}

users:
  - username: user
    email: user@tinnguyen-lab.com
    display_name: User
    password: ${{ZIEE_DEFAULT_USER_PASSWORD}}

groups:
  - name: Users
    remove:
      - projects::*
      - hub::*
      - assistants::*
"#
    )
}

/// Write `yaml` to a temp file that outlives the test, and return the env pairs
/// a server needs to reconcile it.
fn env_for(yaml: &str, admin_pw: &str, user_pw: &str) -> (tempfile::TempDir, Vec<(String, String)>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("desired-state.yaml");
    std::fs::write(&path, yaml).expect("write manifest");

    let env = vec![
        (
            "ZIEE_DESIRED_STATE_FILE".to_string(),
            path.to_string_lossy().to_string(),
        ),
        ("RCPA_MCP_URL".to_string(), RCPA_URL.to_string()),
        ("DSCC_MCP_URL".to_string(), DSCC_URL.to_string()),
        ("BIOGNOSIA_MCP_URL".to_string(), BIOGNOSIA_URL.to_string()),
        ("ZIEE_ADMIN_PASSWORD".to_string(), admin_pw.to_string()),
        ("ZIEE_DEFAULT_USER_PASSWORD".to_string(), user_pw.to_string()),
    ];
    (dir, env)
}

/// Start a test server whose boot reconciles `yaml`.
async fn server_with(yaml: &str) -> (TestServer, tempfile::TempDir) {
    let (dir, extra_env) = env_for(yaml, ADMIN_PASSWORD, USER_PASSWORD);
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env,
        ..Default::default()
    })
    .await;
    (server, dir)
}

/// A second `ziee` process, booted against an ALREADY-reconciled database —
/// i.e. "deploy the same stack again". Kills the process once it reports
/// healthy (its reconcile has run by then: `reconcile` completes before the
/// listener binds).
struct Reboot(Child);

impl Drop for Reboot {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn reboot(server: &TestServer, yaml: &str, admin_pw: &str, user_pw: &str) {
    let (_dir, env) = env_for(yaml, admin_pw, user_pw);

    // Parse the harness's DB url: postgresql://user:pass@host:port/db
    let url = server.database_url.clone();
    let rest = url.strip_prefix("postgresql://").expect("pg url");
    let (creds, host_db) = rest.split_once('@').expect("creds@host");
    let (user, pass) = creds.split_once(':').expect("user:pass");
    let (host_port, db) = host_db.split_once('/').expect("host/db");
    let (host, port) = host_port.split_once(':').expect("host:port");

    // A free port for this throwaway server.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let http_port = listener.local_addr().unwrap().port();
    drop(listener);

    let data_dir = tempfile::tempdir().expect("data dir");
    let config = format!(
        r#"
app:
  data_dir: '{data}'
postgresql:
  use_embedded: false
  external:
    host: "{host}"
    port: {port}
    username: "{user}"
    password: "{pass}"
    database: "{db}"
  pool:
    max_connections: 5
    min_connections: 1
    acquire_timeout_secs: 3
    idle_timeout_secs: 10
    max_lifetime_secs: 60
server:
  host: "127.0.0.1"
  port: {http_port}
  api_prefix: "/api"
  rate_limit:
    enabled: false
    per_second: 1000
    burst_size: 5000
jwt:
  secret: "test-secret-for-reboot-at-least-32-characters-long"
  issuer: "ziee"
  audience: "ziee-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
update_check:
  enabled: false
"#,
        data = data_dir.path().display(),
    );

    let config_path = data_dir.path().join("reboot.yaml");
    std::fs::write(&config_path, config).expect("write reboot config");

    // Same binary the harness spawns (src-app/target/debug/ziee).
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let exe = if cfg!(windows) { "ziee.exe" } else { "ziee" };
    let binary = manifest_dir
        .parent()
        .map(|p| p.join("target/debug").join(exe))
        .filter(|p| p.exists())
        .unwrap_or_else(|| manifest_dir.join("target/debug").join(exe));

    let mut cmd = Command::new(&binary);
    cmd.arg("--config-file").arg(&config_path);
    for (k, v) in &env {
        cmd.env(k, v);
    }
    let child = cmd.spawn().expect("spawn second ziee");
    let _guard = Reboot(child);

    // Wait for the second boot to finish (health implies reconcile is done).
    let health = format!("http://127.0.0.1:{http_port}/api/health");
    let client = reqwest::Client::new();
    let mut ready = false;
    for _ in 0..150 {
        if let Ok(res) = client.get(&health).send().await {
            if res.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    assert!(ready, "the second (re-deploy) server never became healthy");
    // `_guard` drops here → the throwaway server is killed.
}

async fn login(server: &TestServer, username: &str, password: &str) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .expect("login request")
}

async fn login_token(server: &TestServer, username: &str, password: &str) -> String {
    let res = login(server, username, password).await;
    assert_eq!(res.status(), 200, "{username} should be able to log in");
    let body: serde_json::Value = res.json().await.unwrap();
    body["tokens"]["access_token"]
        .as_str()
        .expect("access token")
        .to_string()
}

async fn pool_of(server: &TestServer) -> sqlx::PgPool {
    sqlx::PgPool::connect(&server.database_url).await.unwrap()
}

// ───────────────────────────── TEST-5 ─────────────────────────────

/// TEST-5 — a fresh deploy registers the 3 org MCP servers exactly once, with
/// the declared fields, each usable by the Users group.
#[tokio::test]
async fn test_fresh_deploy_creates_system_mcp_servers() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let pool = pool_of(&server).await;

    let rows = sqlx::query!(
        r#"SELECT name, display_name, url, enabled, is_system, is_built_in, user_id,
                  transport_type, timeout_seconds, supports_sampling, usage_mode
           FROM mcp_servers
           WHERE name IN ('rcpa', 'dscc', 'biognosia')
           ORDER BY name"#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 3, "expected exactly the 3 declared servers");

    let bio = &rows[0];
    assert_eq!(bio.name, "biognosia");
    assert_eq!(bio.url.as_deref(), Some(BIOGNOSIA_URL));
    assert!(bio.supports_sampling, "biognosia declares sampling support");

    let dscc = &rows[1];
    assert_eq!(dscc.name, "dscc");
    assert_eq!(dscc.url.as_deref(), Some(DSCC_URL));
    assert_eq!(dscc.timeout_seconds, 300);
    assert!(!dscc.supports_sampling);

    let rcpa = &rows[2];
    assert_eq!(rcpa.name, "rcpa");
    assert_eq!(rcpa.display_name, "RCPA");
    assert_eq!(rcpa.url.as_deref(), Some(RCPA_URL));
    assert_eq!(rcpa.timeout_seconds, 300);
    assert!(!rcpa.supports_sampling);

    for row in &rows {
        assert!(row.enabled, "{} must be enabled", row.name);
        assert!(row.is_system, "{} must be a SYSTEM server", row.name);
        assert!(
            !row.is_built_in,
            "{} must stay admin-configurable (not a zero-config built-in)",
            row.name
        );
        assert!(
            row.user_id.is_none(),
            "{} is a system server → no owner",
            row.name
        );
        assert_eq!(row.transport_type, "http");
        // The model decides when to call these tools — never force-attached.
        assert_eq!(
            row.usage_mode, "auto",
            "{} must be usage_mode=auto (LLM decides)",
            row.name
        );
    }

    // Assigned to the Users group — without this, non-admin users cannot use them.
    let assigned = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM user_group_mcp_servers ugms
           JOIN mcp_servers s ON s.id = ugms.mcp_server_id
           JOIN groups g ON g.id = ugms.group_id
           WHERE g.name = 'Users' AND s.name IN ('rcpa', 'dscc', 'biognosia')"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(assigned, 3, "all 3 servers must be assigned to the Users group");
}

// ───────────────────────────── TEST-6 ─────────────────────────────

/// TEST-6 — idempotency. A second deploy against the same DB creates no
/// duplicate rows or assignments; `ensure` does not clobber an admin's edit,
/// while `enforce` re-syncs it back to the file.
#[tokio::test]
async fn test_second_deploy_is_idempotent_and_respects_mode() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let pool = pool_of(&server).await;

    // Simulate an admin editing the row after the first deploy.
    sqlx::query!(
        "UPDATE mcp_servers SET enabled = false, display_name = 'RCPA (edited)' WHERE name = 'rcpa'"
    )
    .execute(&pool)
    .await
    .unwrap();

    // ── second deploy, ensure mode ──
    reboot(&server, &manifest("ensure"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let counts = sqlx::query!(
        r#"SELECT name, COUNT(*) as "count!" FROM mcp_servers
           WHERE name IN ('rcpa', 'dscc', 'biognosia') GROUP BY name"#
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(counts.len(), 3);
    for row in &counts {
        assert_eq!(row.count, 1, "{} was duplicated by the re-deploy", row.name);
    }

    let assignments = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM user_group_mcp_servers ugms
           JOIN mcp_servers s ON s.id = ugms.mcp_server_id
           WHERE s.name IN ('rcpa', 'dscc', 'biognosia')"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(assignments, 3, "group assignments were duplicated");

    let rcpa = sqlx::query!("SELECT enabled, display_name FROM mcp_servers WHERE name = 'rcpa'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !rcpa.enabled && rcpa.display_name == "RCPA (edited)",
        "ensure mode must NOT clobber a later admin edit"
    );

    // ── third deploy, enforce mode → the file wins ──
    reboot(&server, &manifest("enforce"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let rcpa = sqlx::query!("SELECT enabled, display_name, url FROM mcp_servers WHERE name = 'rcpa'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(rcpa.enabled, "enforce mode must re-sync `enabled` from the file");
    assert_eq!(
        rcpa.display_name, "RCPA",
        "enforce mode must re-sync `display_name` from the file"
    );
    assert_eq!(rcpa.url.as_deref(), Some(RCPA_URL));

    // Still no duplicates after the enforce pass.
    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name IN ('rcpa','dscc','biognosia')"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 3);
}

// ───────────────────────────── TEST-7 ─────────────────────────────

/// TEST-7 — env-secret resolution. An entry whose env var is unset is skipped
/// (and the rest still apply); an INLINE secret is rejected outright. Neither
/// stops the server from booting.
#[tokio::test]
async fn test_unset_env_skips_entry_and_inline_secret_is_rejected() {
    let yaml = r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
    url: ${RCPA_MCP_URL}
    groups: [Users]
  - name: dscc
    display_name: DSCC
    url: ${DSCC_MCP_URL_NOT_SET}
    groups: [Users]

users:
  - username: inline
    email: inline@tinnguyen-lab.com
    password: hunter2-inline-literal
"#;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desired-state.yaml");
    std::fs::write(&path, yaml).unwrap();

    // Deliberately provide ONLY the rcpa URL.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            (
                "ZIEE_DESIRED_STATE_FILE".to_string(),
                path.to_string_lossy().to_string(),
            ),
            ("RCPA_MCP_URL".to_string(), RCPA_URL.to_string()),
        ],
        ..Default::default()
    })
    .await;

    // The server booted (the harness only returns once /api/health is green),
    // which is itself the "a bad entry never crashes boot" assertion.
    let pool = pool_of(&server).await;

    let rcpa = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'rcpa'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rcpa, 1, "the resolvable server must still be created");

    let dscc = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'dscc'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dscc, 0, "a server whose URL env var is unset must be SKIPPED");

    let inline = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE username = 'inline'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        inline, 0,
        "a user whose password is an INLINE literal must be rejected, not created"
    );
}

// ───────────────────────────── TEST-8 ─────────────────────────────

/// TEST-8 — the admin is created on a fresh deploy, and a later deploy NEVER
/// resets its password (the operator's rotation must stick).
#[tokio::test]
async fn test_admin_is_ensured_once_and_password_is_never_reset() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let pool = pool_of(&server).await;

    let admin = sqlx::query!(
        "SELECT id, email, is_admin, is_active FROM users WHERE username = 'admin'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(admin.is_admin, "the seeded admin must be the root admin");
    assert!(admin.is_active);
    assert_eq!(admin.email, "admin@tinnguyen-lab.com");

    // …and it is in BOTH Administrators and Users (create_admin_user's contract).
    let groups: Vec<String> = sqlx::query_scalar!(
        "SELECT g.name FROM groups g
         JOIN user_groups ug ON ug.group_id = g.id
         WHERE ug.user_id = $1 ORDER BY g.name",
        admin.id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(groups, vec!["Administrators".to_string(), "Users".to_string()]);

    // It can log in with the password from the env.
    assert_eq!(login(&server, "admin", ADMIN_PASSWORD).await.status(), 200);

    // ── re-deploy with a DIFFERENT admin password ──
    reboot(&server, &manifest("ensure"), "totally-different-pw-2", USER_PASSWORD).await;

    // The ORIGINAL password still works …
    assert_eq!(
        login(&server, "admin", ADMIN_PASSWORD).await.status(),
        200,
        "a re-deploy must NOT reset the admin password"
    );
    // … and the new one does not.
    assert_eq!(
        login(&server, "admin", "totally-different-pw-2").await.status(),
        401,
        "the re-deploy's password must never have been applied"
    );

    // Exactly one admin, still.
    let admins = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE is_admin = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(admins, 1);
}

// ───────────────────────────── TEST-9 ─────────────────────────────

/// TEST-9 — the regular user is seeded into the default group, is NOT an admin,
/// can log in, and is not duplicated by a re-deploy.
#[tokio::test]
async fn test_regular_user_is_seeded_in_the_default_group() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let pool = pool_of(&server).await;

    let user = sqlx::query!("SELECT id, is_admin, is_active FROM users WHERE username = 'user'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!user.is_admin, "the seeded user must NOT be an admin");
    assert!(user.is_active);

    let groups: Vec<String> = sqlx::query_scalar!(
        "SELECT g.name FROM groups g
         JOIN user_groups ug ON ug.group_id = g.id
         WHERE ug.user_id = $1",
        user.id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(groups, vec!["Users".to_string()]);

    assert_eq!(login(&server, "user", USER_PASSWORD).await.status(), 200);

    reboot(&server, &manifest("ensure"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE username = 'user'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "the re-deploy duplicated the seeded user");
    assert_eq!(
        login(&server, "user", USER_PASSWORD).await.status(),
        200,
        "the re-deploy must not have re-hashed / broken the user's password"
    );
}

// ───────────────────────────── TEST-10 ─────────────────────────────

/// TEST-10 — the default group loses exactly the hidden features' permissions,
/// and keeps everything else.
#[tokio::test]
async fn test_users_group_permissions_are_trimmed() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let pool = pool_of(&server).await;

    let perms: Vec<String> = sqlx::query_scalar!(
        "SELECT permissions FROM groups WHERE name = 'Users' AND is_system = true AND is_default = true"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    for hidden in ["assistants::", "hub::", "projects::"] {
        assert!(
            !perms.iter().any(|p| p.starts_with(hidden)),
            "the Users group still holds a `{hidden}*` permission: {perms:?}"
        );
    }

    // The KEEP set — General is ungated by design; these are the rest.
    for kept in [
        "profile::read",
        "profile::edit",
        "chat::read",
        "conversations::create",
        "messages::create",
        "files::read",
        "mcp_servers::read",
        "user_llm_providers::read",
    ] {
        assert!(
            perms.iter().any(|p| p == kept),
            "the Users group LOST `{kept}`, which must be kept: {perms:?}"
        );
    }
}

// ───────────────────────────── TEST-11 ─────────────────────────────

/// TEST-11 — the backend deny half (A9): the seeded, non-admin user is refused
/// by the hidden features' APIs, and still served by the kept ones.
#[tokio::test]
async fn test_restricted_user_is_denied_hidden_features() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let token = login_token(&server, "user", USER_PASSWORD).await;
    let client = reqwest::Client::new();

    for denied in ["/assistants", "/hub/assistants", "/projects"] {
        let res = client
            .get(server.api_url(denied))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            403,
            "{denied} must be FORBIDDEN for a user whose group lost the permission"
        );
    }

    for allowed in ["/mcp/servers", "/auth/me"] {
        let res = client
            .get(server.api_url(allowed))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            200,
            "{allowed} must still work for the seeded user"
        );
    }
}

// ───────────────────────────── TEST-12 ─────────────────────────────

/// TEST-12 — migration 157 removed the three unused seeded system servers, and
/// left `fetch` (enabled + group-assigned) and the `files` built-in intact.
#[tokio::test]
async fn test_unused_builtin_servers_are_gone() {
    let server = TestServer::start().await;
    let pool = pool_of(&server).await;

    for removed in ["filesystem", "browser", "git"] {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = $1 AND is_system = true"#,
            removed
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "the `{removed}` system server should be deleted");
    }

    // `fetch` survives, still enabled, still assigned to the default group.
    let fetch = sqlx::query!(
        "SELECT id, enabled FROM mcp_servers WHERE name = 'fetch' AND is_system = true"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(fetch.enabled, "`fetch` must remain enabled");

    let assigned = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM user_group_mcp_servers ugms
           JOIN groups g ON g.id = ugms.group_id
           WHERE ugms.mcp_server_id = $1 AND g.is_default = true"#,
        fetch.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(assigned, 1, "`fetch` must remain assigned to the default group");

    // The load-bearing `files` built-in is a DIFFERENT row and is untouched.
    let files = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'files' AND is_built_in = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(files, 1, "the `files` built-in MCP server must NOT be affected");
}

// ───────────────────────────── no-op path ─────────────────────────────

/// The reconciler is inert unless it is asked for: no env var → nothing happens
/// (this is what keeps dev, desktop and the rest of the suite unaffected).
#[tokio::test]
async fn test_no_env_var_means_no_reconcile() {
    let server = TestServer::start().await;
    let pool = pool_of(&server).await;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name IN ('rcpa','dscc','biognosia')"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);

    let admins = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE is_admin = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(admins, 0, "no desired-state file → no admin is seeded");

    // The stock Users group still has its assistants/hub permissions.
    let perms: Vec<String> = sqlx::query_scalar!(
        "SELECT permissions FROM groups WHERE name = 'Users' AND is_default = true"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        perms.iter().any(|p| p.starts_with("assistants::")),
        "without a desired-state file the default permissions must be untouched"
    );
}
