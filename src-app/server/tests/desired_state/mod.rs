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

const RCPA_URL: &str = "http://host.docker.internal:18120/mcp";
const DSCC_URL: &str = "http://host.docker.internal:18122/mcp";
const BIOGNOSIA_URL: &str = "http://host.docker.internal:18100/mcp";
const ADMIN_PASSWORD: &str = "ds-admin-pw-1";
const USER_PASSWORD: &str = "ds-user-pw-1";
// Dummy Google OIDC creds for the auth_providers reconcile tests (not real).
const GOOGLE_CLIENT_ID: &str = "ds-test-google-id.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "ds-test-google-secret-xyz";
/// Must match the harness's `secrets.storage_key` (common/harness_inner.rs) so
/// the reboot process — whose config we build in `reboot()` — encrypts with the
/// SAME key the main `TestServer` used; otherwise the second boot would run
/// keyless and write the secret in plaintext.
const STORAGE_KEY: &str = "test-storage-key-for-pgcrypto-min-32-chars-long";

/// A manifest carrying ONLY the `google` auth-provider entry (the org MCP
/// servers are covered by `manifest()`); `mode` is templated so a test can flip
/// ensure/enforce.
fn google_manifest(mode: &str) -> String {
    format!(
        r#"
auth_providers:
  - name: google
    enabled: true
    client_id: ${{GOOGLE_CLIENT_ID}}
    client_secret: ${{GOOGLE_CLIENT_SECRET}}
    mode: {mode}
"#
    )
}

/// The shape of the file we ship, kept in one place so each test can tweak it.
fn manifest(mode: &str) -> String {
    format!(
        r#"
mcp_servers:
  - name: rcpa-user
    display_name: RCPA
    description: RCPA analysis tools
    url: ${{RCPA_MCP_URL}}
    enabled: true
    supports_sampling: false
    timeout_seconds: 300
    groups: [Users]
    mode: {mode}
  - name: dscc-user
    display_name: DSCC
    url: ${{DSCC_MCP_URL}}
    enabled: true
    supports_sampling: false
    timeout_seconds: 300
    groups: [Users]
    mode: {mode}
  - name: biognosia-user
    display_name: Biognosia
    url: ${{BIOGNOSIA_MCP_URL}}
    enabled: true
    supports_sampling: true
    groups: [Users]
    mode: {mode}


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
      - knowledge_base::*
      - scheduler::*
      - assistants::*
      - web_search::*
      - lit_search::*
      - workflows::*
      - memory::*
      - citations::*
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
        // The DEPLOY SIGNAL. Without it the reconciler is inert (that is the
        // local-dev default) — `test_file_present_but_flag_unset_is_a_no_op`
        // pins exactly that.
        ("ZIEE_APPLY_DESIRED_STATE".to_string(), "1".to_string()),
        (
            "ZIEE_DESIRED_STATE_FILE".to_string(),
            path.to_string_lossy().to_string(),
        ),
        // The boot health check probes every enabled, non-built-in MCP server and
        // AUTO-DISABLES the unreachable ones (mcp/connection_health.rs). Our
        // fixture URLs are deliberately unreachable, so without this debug-only
        // seam the probe would flip `enabled` to false underneath every
        // assertion here — a race, not a real defect. (Production handles this by
        // declaring the servers `enforce`, which re-asserts `enabled` each boot.)
        ("ZIEE_DISABLE_MCP_HEALTH_CHECK".to_string(), "1".to_string()),
        ("RCPA_MCP_URL".to_string(), RCPA_URL.to_string()),
        ("DSCC_MCP_URL".to_string(), DSCC_URL.to_string()),
        ("BIOGNOSIA_MCP_URL".to_string(), BIOGNOSIA_URL.to_string()),
        ("ZIEE_ADMIN_USERNAME".to_string(), "admin".to_string()),
        ("ZIEE_ADMIN_EMAIL".to_string(), "admin@tinnguyen-lab.com".to_string()),
        ("ZIEE_ADMIN_PASSWORD".to_string(), admin_pw.to_string()),
        ("ZIEE_DEFAULT_USER_PASSWORD".to_string(), user_pw.to_string()),
        // The google auth_providers entry resolves these; harmless to the
        // MCP-only manifests (which carry no `auth_providers` block).
        ("GOOGLE_CLIENT_ID".to_string(), GOOGLE_CLIENT_ID.to_string()),
        ("GOOGLE_CLIENT_SECRET".to_string(), GOOGLE_CLIENT_SECRET.to_string()),
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
# Same at-rest key the main TestServer harness uses, so the re-deploy process
# encrypts auth-provider secrets (not keyless/plaintext) — see STORAGE_KEY.
secrets:
  storage_key: "{storage_key}"
update_check:
  enabled: false
"#,
        data = data_dir.path().display(),
        storage_key = STORAGE_KEY,
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
    // `AuthResponse.tokens` is `#[serde(flatten)]`, so the token pair is at the
    // TOP level of the body, not nested under "tokens".
    body["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("no access_token in the login response: {body}"))
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
           WHERE name IN ('rcpa-user', 'dscc-user', 'biognosia-user')
           ORDER BY name"#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 3, "expected exactly the 3 declared servers");

    let bio = &rows[0];
    assert_eq!(bio.name, "biognosia-user");
    assert_eq!(bio.url.as_deref(), Some(BIOGNOSIA_URL));
    assert!(bio.supports_sampling, "biognosia declares sampling support");

    let dscc = &rows[1];
    assert_eq!(dscc.name, "dscc-user");
    assert_eq!(dscc.url.as_deref(), Some(DSCC_URL));
    assert_eq!(dscc.timeout_seconds, 300);
    assert!(!dscc.supports_sampling);

    let rcpa = &rows[2];
    assert_eq!(rcpa.name, "rcpa-user");
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
           WHERE g.name = 'Users' AND s.name IN ('rcpa-user', 'dscc-user', 'biognosia-user')"#
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
        "UPDATE mcp_servers SET enabled = false, display_name = 'RCPA (edited)' WHERE name = 'rcpa-user'"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Positive control: delete one declared server outright. If the second boot
    // really reconciles, it comes back. Without this, TEST-6/8/9 would all be
    // "assert nothing changed" — which stays green even if the env plumbing to
    // the second process silently broke and it reconciled NOTHING.
    sqlx::query!("DELETE FROM mcp_servers WHERE name = 'dscc-user'")
        .execute(&pool)
        .await
        .unwrap();

    // ── second deploy, ensure mode ──
    reboot(&server, &manifest("ensure"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let dscc_back = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'dscc-user'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        dscc_back, 1,
        "the second boot did not reconcile at all (the deleted server was not re-created) \
         — every 'nothing changed' assertion below would be vacuous"
    );

    let counts = sqlx::query!(
        r#"SELECT name, COUNT(*) as "count!" FROM mcp_servers
           WHERE name IN ('rcpa-user', 'dscc-user', 'biognosia-user') GROUP BY name"#
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(counts.len(), 3);
    for row in &counts {
        assert_eq!(row.count, 1, "{} was duplicated by the re-deploy", row.name);
    }

    // Still exactly one Users-group assignment per server. (The join table's PK
    // makes literal duplicates impossible, so what this really pins is that the
    // re-deploy neither dropped an assignment nor added a spurious extra group.)
    let assignments = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM user_group_mcp_servers ugms
           JOIN mcp_servers s ON s.id = ugms.mcp_server_id
           JOIN groups g ON g.id = ugms.group_id
           WHERE s.name IN ('rcpa-user', 'dscc-user', 'biognosia-user') AND g.name = 'Users'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        assignments, 3,
        "each server must still be assigned to the Users group after a re-deploy"
    );

    let rcpa = sqlx::query!("SELECT enabled, display_name FROM mcp_servers WHERE name = 'rcpa-user'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !rcpa.enabled && rcpa.display_name == "RCPA (edited)",
        "ensure mode must NOT clobber a later admin edit"
    );

    // ── third deploy, enforce mode → the file wins ──
    reboot(&server, &manifest("enforce"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let rcpa = sqlx::query!("SELECT enabled, display_name, url FROM mcp_servers WHERE name = 'rcpa-user'")
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
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name IN ('rcpa-user','dscc-user','biognosia-user')"#
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
  - name: rcpa-user
    display_name: RCPA
    url: ${RCPA_MCP_URL}
    groups: [Users]
  - name: dscc-user
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
            ("ZIEE_APPLY_DESIRED_STATE".to_string(), "1".to_string()),
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
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'rcpa-user'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rcpa, 1, "the resolvable server must still be created");

    let dscc = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'dscc-user'"#
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

/// TEST-8 — the admin is bootstrapped from the ENV on a virgin database, and a
/// later deploy NEVER reverts it. Specifically: the operator changes the admin
/// password in the running app, the container is redeployed (with the ORIGINAL
/// env password still set), and the CHANGED password must still be the one that
/// works — the env triple is a first-deploy seed, nothing more.
#[tokio::test]
async fn test_admin_is_bootstrapped_from_env_and_never_reverted() {
    let (server, _dir) = server_with(&manifest("ensure")).await;
    let pool = pool_of(&server).await;

    let admin = sqlx::query!(
        "SELECT id, email, is_admin, is_active FROM users WHERE username = 'admin'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(admin.is_admin, "the env-seeded admin must be the root admin");
    assert!(admin.is_active);
    assert_eq!(
        admin.email, "admin@tinnguyen-lab.com",
        "username/email come from the env, not the file"
    );

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
    assert_eq!(login(&server, "admin", ADMIN_PASSWORD).await.status(), 200);

    // ── the operator CHANGES the admin password in the running app ──
    const ROTATED: &str = "operator-rotated-pw-9";
    let token = login_token(&server, "admin", ADMIN_PASSWORD).await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "current_password": ADMIN_PASSWORD,
            "new_password": ROTATED,
        }))
        .send()
        .await
        .expect("change-password request");
    assert!(
        res.status().is_success(),
        "the admin must be able to rotate their own password: {}",
        res.status()
    );
    assert_eq!(login(&server, "admin", ROTATED).await.status(), 200);

    // ── redeploy: same image, same env (ORIGINAL password still in the env) ──
    reboot(&server, &manifest("ensure"), ADMIN_PASSWORD, USER_PASSWORD).await;

    // The ROTATED password still works …
    assert_eq!(
        login(&server, "admin", ROTATED).await.status(),
        200,
        "a redeploy must NOT revert an admin password the operator changed"
    );
    // … and the env's original password does NOT come back.
    assert_eq!(
        login(&server, "admin", ADMIN_PASSWORD).await.status(),
        401,
        "the env password must not be re-applied on a redeploy"
    );

    // Still exactly one admin, and no second account was minted.
    let admins = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE is_admin = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(admins, 1);
}

/// The virgin-DB rule: if the database already has ANY account (even a non-admin
/// one), the env admin bootstrap does nothing at all.
#[tokio::test]
async fn test_admin_bootstrap_skipped_when_any_account_exists() {
    // A server with NO admin env — so the DB comes up empty of accounts …
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desired-state.yaml");
    std::fs::write(&path, manifest("ensure")).unwrap();

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("ZIEE_APPLY_DESIRED_STATE".to_string(), "1".to_string()),
            ("ZIEE_DISABLE_MCP_HEALTH_CHECK".to_string(), "1".to_string()),
            (
                "ZIEE_DESIRED_STATE_FILE".to_string(),
                path.to_string_lossy().to_string(),
            ),
            // No ZIEE_ADMIN_* triple → no admin. The manifest's `users:` entry
            // still runs, so the table ends up NON-empty.
            (
                "ZIEE_DEFAULT_USER_PASSWORD".to_string(),
                USER_PASSWORD.to_string(),
            ),
        ],
        ..Default::default()
    })
    .await;
    let pool = pool_of(&server).await;

    let admins = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE is_admin = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(admins, 0, "no admin env ⇒ no admin");
    let seeded = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE username = 'user'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(seeded, 1, "the non-admin user still got seeded");

    // ── now redeploy WITH the admin env: the table is no longer empty, so the
    //    bootstrap must stay a no-op (it may only ever run on a virgin DB) ──
    reboot(&server, &manifest("ensure"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let admins = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE is_admin = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        admins, 0,
        "an existing account must block the admin bootstrap entirely"
    );
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

    // Positive control: a declared server deleted here must come back, proving the
    // second process really reconciled (so "the user was not duplicated" is a
    // real assertion, not an artifact of a no-op boot).
    sqlx::query!("DELETE FROM mcp_servers WHERE name = 'rcpa-user'")
        .execute(&pool)
        .await
        .unwrap();

    reboot(&server, &manifest("ensure"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let rcpa_back = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name = 'rcpa-user'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rcpa_back, 1, "the second boot did not reconcile at all");

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

    for hidden in [
        "assistants::",
        "hub::",
        "projects::",
        "knowledge_base::",
        "scheduler::",
        "web_search::",
        "lit_search::",
        "workflows::",
        "memory::",
        "citations::",
    ] {
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
        // Granted alongside scheduler by migration 142 — users still need their
        // notifications, so the `scheduler::*` removal must not take it.
        "notifications::read",
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

    for denied in [
        "/assistants",
        "/hub/assistants",
        "/projects",
        "/knowledge-bases",
        "/scheduled-tasks",
        "/citations",
        "/workflows",
    ] {
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
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers WHERE name IN ('rcpa-user','dscc-user','biognosia-user')"#
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

// ───────────────────────── the deploy gate ─────────────────────────

/// The reconciler is DEPLOY-ONLY: a desired-state file that is present and fully
/// resolvable must still do NOTHING unless `ZIEE_APPLY_DESIRED_STATE` says so.
/// This is what protects a local developer's hand-configured MCP servers, admin
/// and permissions from being seeded/enforced/duplicated behind their back.
#[tokio::test]
async fn test_file_present_but_flag_unset_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desired-state.yaml");
    std::fs::write(&path, manifest("enforce")).unwrap();

    // Everything the reconciler would need — EXCEPT the deploy signal.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            (
                "ZIEE_DESIRED_STATE_FILE".to_string(),
                path.to_string_lossy().to_string(),
            ),
            ("RCPA_MCP_URL".to_string(), RCPA_URL.to_string()),
            ("DSCC_MCP_URL".to_string(), DSCC_URL.to_string()),
            ("BIOGNOSIA_MCP_URL".to_string(), BIOGNOSIA_URL.to_string()),
            ("ZIEE_ADMIN_USERNAME".to_string(), "admin".to_string()),
            ("ZIEE_ADMIN_EMAIL".to_string(), "admin@tinnguyen-lab.com".to_string()),
            ("ZIEE_ADMIN_PASSWORD".to_string(), ADMIN_PASSWORD.to_string()),
            (
                "ZIEE_DEFAULT_USER_PASSWORD".to_string(),
                USER_PASSWORD.to_string(),
            ),
        ],
        ..Default::default()
    })
    .await;
    let pool = pool_of(&server).await;

    // No servers …
    let servers = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM mcp_servers
           WHERE name IN ('rcpa-user','dscc-user','biognosia-user')"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(servers, 0, "no MCP server may be created without the deploy flag");

    // … no accounts …
    let admins = sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM users WHERE is_admin"#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(admins, 0, "no admin may be seeded without the deploy flag");
    let users = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM users WHERE username = 'user'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(users, 0, "no user may be seeded without the deploy flag");

    // … and the developer's permissions are untouched.
    let perms: Vec<String> = sqlx::query_scalar!(
        "SELECT permissions FROM groups WHERE name = 'Users' AND is_default = true"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        perms.iter().any(|p| p.starts_with("assistants::"))
            && perms.iter().any(|p| p.starts_with("hub::")),
        "the default group's permissions must NOT be trimmed without the deploy flag: {perms:?}"
    );
}

// ───────────────────────── Auth providers (Google OIDC) ─────────────────────

/// Helper: read the `google` auth_providers row's key columns.
async fn google_row(
    pool: &sqlx::PgPool,
) -> (bool, Option<String>, Option<String>, bool) {
    // enabled, config->>'client_id', decrypted client_secret, has_encrypted_blob
    let row = sqlx::query!(
        r#"SELECT enabled,
                  config->>'client_id'      as client_id,
                  config->>'client_secret'  as config_secret,
                  client_secret_encrypted   as enc,
                  pgp_sym_decrypt(client_secret_encrypted, $1) as decrypted
           FROM auth_providers WHERE name = 'google'"#,
        STORAGE_KEY
    )
    .fetch_one(pool)
    .await
    .unwrap();
    // config plaintext must be blanked once encrypted at rest.
    assert!(
        row.config_secret.as_deref().unwrap_or("").is_empty(),
        "config->>'client_secret' must be BLANK when encrypted at rest, got {:?}",
        row.config_secret
    );
    (row.enabled, row.client_id, row.decrypted, row.enc.is_some())
}

/// TEST-5 — with the two env vars set, boot stamps client_id + an ENCRYPTED
/// client_secret onto the pre-seeded `google` row and enables it. The plaintext
/// copy in `config` is blanked; the ciphertext decrypts to the real secret.
#[tokio::test]
async fn test_google_provider_configured_and_enabled_from_env() {
    let (server, _dir) = server_with(&google_manifest("enforce")).await;
    let pool = pool_of(&server).await;

    let (enabled, client_id, decrypted, has_blob) = google_row(&pool).await;
    assert!(enabled, "google must be ENABLED after reconcile");
    assert_eq!(
        client_id.as_deref(),
        Some(GOOGLE_CLIENT_ID),
        "client_id must be stamped from GOOGLE_CLIENT_ID"
    );
    assert!(has_blob, "client_secret_encrypted must be populated");
    assert_eq!(
        decrypted.as_deref(),
        Some(GOOGLE_CLIENT_SECRET),
        "the encrypted secret must decrypt to GOOGLE_CLIENT_SECRET"
    );

    // The seeded fields survive the merge (issuer / scopes / mapping).
    let issuer = sqlx::query_scalar!(
        r#"SELECT config->>'issuer_url' FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(issuer.as_deref(), Some("https://accounts.google.com"));

    // Exactly one google row — never duplicated.
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

/// TEST-6 — with the env vars UNSET, the google entry is skipped: the seeded row
/// stays disabled with empty client_id and NO encrypted secret. Google stays off
/// (and the server still boots — a skipped entry never crashes the boot).
#[tokio::test]
async fn test_google_provider_skipped_when_env_unset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desired-state.yaml");
    std::fs::write(&path, google_manifest("enforce")).unwrap();

    // Deploy flag ON, storage key present, but the two GOOGLE_* vars ABSENT.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("ZIEE_APPLY_DESIRED_STATE".to_string(), "1".to_string()),
            (
                "ZIEE_DESIRED_STATE_FILE".to_string(),
                path.to_string_lossy().to_string(),
            ),
        ],
        ..Default::default()
    })
    .await;
    let pool = pool_of(&server).await;

    let row = sqlx::query!(
        r#"SELECT enabled, config->>'client_id' as client_id, client_secret_encrypted as enc
           FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!row.enabled, "google must stay DISABLED when creds are unset");
    assert_eq!(
        row.client_id.as_deref().unwrap_or(""),
        "",
        "client_id must stay blank when the entry is skipped"
    );
    assert!(
        row.enc.is_none(),
        "no secret may be written when the entry is skipped"
    );
}

/// TEST-7 — a second deploy (same DB) is idempotent: still exactly one google
/// row, still enabled, client_id stable, and the secret still decrypts (no
/// duplicate, no plaintext leak on the re-stamp).
#[tokio::test]
async fn test_google_provider_reconcile_is_idempotent() {
    let (server, _dir) = server_with(&google_manifest("enforce")).await;
    let pool = pool_of(&server).await;

    // Positive control: an admin disables it after the first deploy. `enforce`
    // must re-enable it on the next boot — proving the second reconcile ran.
    sqlx::query!("UPDATE auth_providers SET enabled = false WHERE name = 'google'")
        .execute(&pool)
        .await
        .unwrap();

    reboot(&server, &google_manifest("enforce"), ADMIN_PASSWORD, USER_PASSWORD).await;

    let (enabled, client_id, decrypted, has_blob) = google_row(&pool).await;
    assert!(enabled, "enforce must re-enable google on the re-deploy");
    assert_eq!(client_id.as_deref(), Some(GOOGLE_CLIENT_ID), "client_id stable");
    assert!(has_blob && decrypted.as_deref() == Some(GOOGLE_CLIENT_SECRET));

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "the re-deploy must not duplicate the google row");
}
