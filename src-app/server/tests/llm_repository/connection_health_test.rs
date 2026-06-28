// ============================================================================
// LLM Repository Connection-Health — Tier 2 Integration Tests
// ============================================================================
//
// Covers the three enforcement entry points (create / update-transition /
// boot) end-to-end against a real Postgres + a `wiremock` HTTP server that
// stands in for the upstream auth-test endpoint. Mirrors the MCP module's
// connection-health test shape (no shared fixture — each spec spins its own
// mock so failures are isolated).

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use reqwest::StatusCode;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const REPO_ADMIN_PERMS: &[&str] = &[
    "llm_repositories::read",
    "llm_repositories::create",
    "llm_repositories::edit",
    "llm_repositories::delete",
];

async fn pool_for(server: &TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test pool")
}

/// Stand up a mock that returns 200 OK on any GET. The probe accepts
/// only 200 as success (per `test_repository_connectivity`).
async fn mock_ok() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&mock)
        .await;
    mock
}

/// Stand up a mock that returns 401 on any GET, simulating a stale
/// or missing credential.
async fn mock_401() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock)
        .await;
    mock
}

/// Helper — read a repository row + assert its health-column shape.
async fn read_repo_row(
    pool: &sqlx::PgPool,
    repo_id: Uuid,
) -> (bool, String, Option<String>, Option<chrono::DateTime<chrono::Utc>>) {
    let row = sqlx::query!(
        r#"SELECT enabled, last_health_check_status, last_health_check_reason, last_health_check_at
           FROM llm_repositories WHERE id = $1"#,
        repo_id,
    )
    .fetch_one(pool)
    .await
    .expect("read repo row");
    let at = row
        .last_health_check_at
        .and_then(|t| chrono::DateTime::<chrono::Utc>::from_timestamp(t.unix_timestamp(), 0));
    (
        row.enabled,
        row.last_health_check_status,
        row.last_health_check_reason,
        at,
    )
}

// ─── create flow ────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_with_enabled_true_against_working_mock_persists_healthy() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_ok().await;

    let body: Value = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "ok-repo",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Response shape is the new `LlmRepositoryWithHealthWarning` wrapper.
    assert!(
        body.get("connection_warning").is_none()
            || body["connection_warning"].is_null(),
        "happy path must omit connection_warning: {body}"
    );
    let repo = &body;
    let repo_id: Uuid =
        Uuid::parse_str(repo["id"].as_str().expect("id")).expect("uuid");

    assert_eq!(repo["enabled"], true, "row stays enabled on a passing probe");
    assert_eq!(
        repo["last_health_check_status"], "healthy",
        "status recorded as healthy: {body}"
    );

    // DB-level cross-check (the response was assembled from the
    // refetched row, but verify directly so a future serde tweak
    // doesn't silently mask a missing UPDATE).
    let pool = pool_for(&server).await;
    let (enabled, status, reason, at) = read_repo_row(&pool, repo_id).await;
    assert!(enabled);
    assert_eq!(status, "healthy");
    assert_eq!(reason, None);
    assert!(at.is_some(), "last_health_check_at must be stamped");
}

#[tokio::test]
async fn create_with_enabled_true_against_401_mock_persists_disabled_with_warning() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_401().await;

    let body: Value = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "bad-repo",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let repo = &body;
    let repo_id: Uuid =
        Uuid::parse_str(repo["id"].as_str().expect("id")).expect("uuid");

    // Wrapper carries the reason verbatim.
    let warning = &body["connection_warning"];
    assert!(
        warning["reason"].as_str().unwrap_or("").contains("401")
            || warning["reason"].as_str().unwrap_or("").contains("HTTP"),
        "warning must surface the 401 / HTTP failure: {body}"
    );

    // Row was downgraded.
    assert_eq!(
        repo["enabled"], false,
        "row auto-downgrades to disabled on a failing probe: {body}"
    );
    assert_eq!(repo["last_health_check_status"], "unhealthy");
    assert!(repo["last_health_check_reason"].is_string());

    let pool = pool_for(&server).await;
    let (enabled, status, reason, _at) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled);
    assert_eq!(status, "unhealthy");
    assert!(reason.is_some_and(|r| !r.is_empty()));
}

#[tokio::test]
async fn create_with_enabled_false_skips_probe_entirely() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    // Point at a mock that would 401 — we expect it NOT to be reached.
    let upstream = mock_401().await;

    let body: Value = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "off-repo",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let repo = &body;
    let repo_id: Uuid =
        Uuid::parse_str(repo["id"].as_str().expect("id")).expect("uuid");

    assert!(
        body.get("connection_warning").is_none()
            || body["connection_warning"].is_null(),
        "disabled rows skip the probe: {body}"
    );
    assert_eq!(repo["enabled"], false);
    assert_eq!(
        repo["last_health_check_status"], "untested",
        "disabled creates leave status at the default 'untested': {body}"
    );

    let pool = pool_for(&server).await;
    let (enabled, status, reason, at) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled);
    assert_eq!(status, "untested");
    assert_eq!(reason, None);
    assert!(at.is_none(), "no probe ran, so last_health_check_at stays NULL");
}

// ─── update-transition flow ──────────────────────────────────────────────────

#[tokio::test]
async fn update_flipping_false_to_true_against_failing_mock_returns_400() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_401().await;
    let client = reqwest::Client::new();

    // Create disabled (probe skipped).
    let created: Value = client
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "to-enable",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_id: Uuid =
        Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    // PUT { enabled: true } — backend probes, fails, reverts.
    let resp = client
        .post(server.api_url(&format!("/llm-repositories/{repo_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "failing probe must 400: {}",
        resp.status()
    );
    let body: Value = resp.json().await.unwrap();
    assert!(
        body["error_code"]
            .as_str()
            .unwrap_or("")
            .contains("LLM_REPOSITORY_ENABLE_FAILED_HEALTH_CHECK"),
        "error_code identifies the enable-transition failure: {body}"
    );

    // Row stays at enabled=false, recorded unhealthy.
    let pool = pool_for(&server).await;
    let (enabled, status, reason, _) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled, "row stays disabled after failing probe");
    assert_eq!(status, "unhealthy");
    assert!(reason.is_some_and(|r| !r.is_empty()));
}

#[tokio::test]
async fn update_flipping_false_to_true_against_working_mock_persists_healthy() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_ok().await;
    let client = reqwest::Client::new();

    let created: Value = client
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "good-to-enable",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_id: Uuid =
        Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let resp = client
        .post(server.api_url(&format!("/llm-repositories/{repo_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["enabled"], true);
    assert_eq!(body["last_health_check_status"], "healthy");

    let pool = pool_for(&server).await;
    let (enabled, status, _, at) = read_repo_row(&pool, repo_id).await;
    assert!(enabled);
    assert_eq!(status, "healthy");
    assert!(at.is_some());
}

#[tokio::test]
async fn update_with_no_enabled_transition_skips_probe() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    // 401 mock — we expect it NOT to be touched.
    let upstream = mock_401().await;
    let client = reqwest::Client::new();

    // Create disabled, then PUT with a name change but `enabled`
    // omitted entirely. No probe should run; status stays 'untested'.
    let created: Value = client
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "rename-me",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_id: Uuid =
        Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let resp = client
        .post(server.api_url(&format!("/llm-repositories/{repo_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "name": "renamed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "non-transition update succeeds even with a failing mock");

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "renamed");
    assert_eq!(
        body["last_health_check_status"], "untested",
        "no probe ran, status stays 'untested': {body}"
    );

    let pool = pool_for(&server).await;
    let (_enabled, status, _reason, at) = read_repo_row(&pool, repo_id).await;
    assert_eq!(status, "untested");
    assert!(at.is_none());
}

// ─── boot-time path ──────────────────────────────────────────────────────────

#[tokio::test]
async fn run_startup_health_check_disables_only_failing_rows() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let healthy_upstream = mock_ok().await;
    let failing_upstream = mock_401().await;
    let client = reqwest::Client::new();

    // Two enabled rows pointing at different mocks.
    let r1: Value = client
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "boot-healthy",
            "url": healthy_upstream.uri(),
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r2: Value = client
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "boot-failing",
            "url": failing_upstream.uri(),
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // The "failing" row was already auto-disabled by the create-flow
    // probe — re-enable it directly via SQL so we can observe the
    // boot-path doing its OWN downgrade (otherwise we'd be testing
    // create-flow twice).
    let pool = pool_for(&server).await;
    let r1_id: Uuid =
        Uuid::parse_str(r1["id"].as_str().unwrap()).unwrap();
    let r2_id: Uuid =
        Uuid::parse_str(r2["id"].as_str().unwrap()).unwrap();
    sqlx::query!(
        "UPDATE llm_repositories SET enabled = TRUE, last_health_check_status = 'untested', last_health_check_reason = NULL, last_health_check_at = NULL WHERE id = $1",
        r2_id,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Drive the boot probe directly.
    ziee::llm_repository_health::run_startup_health_check(pool.clone()).await;

    // Healthy row stayed enabled, status flipped to healthy.
    let (h_enabled, h_status, h_reason, h_at) = read_repo_row(&pool, r1_id).await;
    assert!(h_enabled);
    assert_eq!(h_status, "healthy");
    assert_eq!(h_reason, None);
    assert!(h_at.is_some());

    // Failing row was auto-disabled with a populated reason.
    let (f_enabled, f_status, f_reason, f_at) = read_repo_row(&pool, r2_id).await;
    assert!(!f_enabled, "boot probe must auto-disable a failing row");
    assert_eq!(f_status, "unhealthy");
    assert!(f_reason.is_some_and(|r| !r.is_empty()));
    assert!(f_at.is_some());
}

#[tokio::test]
async fn run_startup_health_check_records_healthy_on_pass() {
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_ok().await;
    let client = reqwest::Client::new();

    let created: Value = client
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "boot-solo-healthy",
            "url": upstream.uri(),
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_id: Uuid =
        Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let pool = pool_for(&server).await;
    // Wipe health-record columns so we can observe the boot path
    // writing them ON ITS OWN (not the create-flow probe leftover).
    sqlx::query!(
        "UPDATE llm_repositories SET last_health_check_status = 'untested', last_health_check_reason = NULL, last_health_check_at = NULL WHERE id = $1",
        repo_id,
    )
    .execute(&pool)
    .await
    .unwrap();

    ziee::llm_repository_health::run_startup_health_check(pool.clone()).await;

    let (enabled, status, reason, at) = read_repo_row(&pool, repo_id).await;
    assert!(enabled, "healthy row stays enabled");
    assert_eq!(status, "healthy");
    assert_eq!(reason, None, "no reason on healthy");
    assert!(at.is_some(), "last_health_check_at stamped on every pass");
}

// ─── test-by-id endpoint — POST /llm-repositories/{id}/test ─────────────────

/// Helper — create a disabled row pointing at `mock_url` via the API.
/// Used by the test-by-id integration tests to set the row state
/// without going through the create-flow probe (which would itself
/// probe the mock, racing the per-test mock state changes).
async fn seed_disabled_row(
    server: &TestServer,
    admin_token: &str,
    name: &str,
    mock_url: &str,
) -> Uuid {
    let body: Value = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&json!({
            "name": name,
            "url": mock_url,
            "auth_type": "none",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn test_by_id_with_persisted_config_passes_against_working_mock() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_ok().await;
    let repo_id = seed_disabled_row(&server, &admin.token, "byid-ok", &upstream.uri()).await;

    let resp: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["success"], true, "passing probe returns success: {resp}");

    // Row stays disabled (the test button doesn't enable; that's the
    // Switch's job). Status flips to healthy.
    let pool = pool_for(&server).await;
    let (enabled, status, reason, at) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled, "test-by-id does not change `enabled`");
    assert_eq!(status, "healthy");
    assert_eq!(reason, None);
    assert!(at.is_some(), "last_health_check_at stamped");
}

#[tokio::test]
async fn test_by_id_with_persisted_config_fails_records_unhealthy_disabled_row() {
    // Plan spec: failure on a CURRENTLY-DISABLED row records
    // unhealthy + reason but does NOT call disable + does NOT emit
    // auto_disabled (flipping enabled would spam listeners for no
    // UI benefit). The row's `enabled` stays at its prior value.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_401().await;
    let repo_id = seed_disabled_row(&server, &admin.token, "byid-fail-off", &upstream.uri()).await;

    let resp: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["success"], false);
    assert!(
        resp["message"]
            .as_str()
            .unwrap_or("")
            .contains("failed"),
        "message surfaces the failure: {resp}"
    );

    let pool = pool_for(&server).await;
    let (enabled, status, reason, _) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled, "disabled row stays disabled after test failure");
    assert_eq!(status, "unhealthy");
    assert!(reason.is_some_and(|r| !r.is_empty()));
}

#[tokio::test]
async fn test_by_id_with_persisted_config_fails_on_enabled_row_auto_disables() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let upstream = mock_ok().await;
    // Create the row enabled by pointing at a passing mock first;
    // the create-flow probe will pass, leaving the row enabled +
    // healthy. Then swap the mock to 401 and click Test.
    let repo_id = {
        let body: Value = reqwest::Client::new()
            .post(server.api_url("/llm-repositories"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({
                "name": "byid-fail-on-enabled",
                "url": upstream.uri(),
                "auth_type": "none",
                "enabled": true,
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
    };
    // Confirm the row really is enabled before flipping mock.
    let pool = pool_for(&server).await;
    let (enabled, _, _, _) = read_repo_row(&pool, repo_id).await;
    assert!(enabled, "fixture must start enabled");

    // Flip the mock to 401 and click Test.
    drop(upstream);
    let bad = mock_401().await;
    // Override the row's URL to the new failing mock via the test
    // request's override body (mirrors a user editing the URL field
    // then clicking Test).
    let resp: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "url": bad.uri() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["success"], false);

    let (enabled, status, reason, _) = read_repo_row(&pool, repo_id).await;
    assert!(
        !enabled,
        "enabled-row test failure must auto-disable (matches boot-probe semantics)",
    );
    assert_eq!(status, "unhealthy");
    assert!(reason.is_some_and(|r| !r.is_empty()));
}

#[tokio::test]
async fn test_by_id_with_form_url_override_uses_form_url_not_persisted() {
    // Cross-URL secret guard: the test endpoint accepts a form
    // `url` override; when supplied, the probe targets that URL
    // (not the persisted one). Verifies that the override actually
    // makes it to the upstream by pointing the override at a DIFFERENT
    // mock than the row was seeded with.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let persisted_upstream = mock_401().await;
    let override_upstream = mock_ok().await;
    let repo_id = seed_disabled_row(
        &server,
        &admin.token,
        "byid-override-url",
        &persisted_upstream.uri(),
    )
    .await;

    // Form override targets the OK mock; result should be success
    // even though the persisted row points at the 401 mock.
    let resp: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "url": override_upstream.uri() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        resp["success"], true,
        "form url override must drive the probe target, not the persisted URL: {resp}",
    );
}

#[tokio::test]
async fn test_by_id_404_for_nonexistent_repository() {
    // Defense-in-depth: a stale UI clicking Test on a row that was
    // deleted concurrently should get a clean 404, not a 500.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}/test", Uuid::new_v4())))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_by_id_concurrent_probes_same_repo_are_race_safe() {
    // Concurrency / TOCTOU: the test-by-id handler probes the upstream
    // and then writes the outcome to the row's `last_health_check_*`
    // columns (via `record_test_outcome`) + refetches the row to emit
    // an `updated` event. Firing many probes for the SAME repository
    // at once exercises that shared per-row write path under contention.
    // A race that corrupted the shared state would surface as a 5xx /
    // panic / deadlock, or as a final row left in an inconsistent
    // (non-healthy / unstamped) state. None of the existing test-by-id
    // specs fire concurrently.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    // All probes hit a working mock, so every individual probe MUST
    // succeed — any failure is a genuine race artifact, not an
    // upstream flip.
    let upstream = mock_ok().await;
    let repo_id =
        seed_disabled_row(&server, &admin.token, "byid-concurrent", &upstream.uri()).await;

    // Fire N genuinely-parallel probes (each its own spawned task +
    // its own reqwest client → independent connections racing at the
    // DB), then join them all.
    const N: usize = 8;
    let url = server.api_url(&format!("/llm-repositories/{repo_id}/test"));
    let token = admin.token.clone();
    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        let url = url.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            let resp = reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({}))
                .send()
                .await
                .expect("probe request failed");
            let status = resp.status();
            let body: Value = resp.json().await.expect("probe body json");
            (status, body)
        }));
    }

    // Every concurrent probe completes with 200 + success:true — no
    // 5xx, no panic, no deadlock under contention on the shared row.
    for h in handles {
        let (status, body) = h.await.expect("probe task panicked");
        assert_eq!(
            status,
            StatusCode::OK,
            "every concurrent probe must return 200 (no race-induced 5xx): {body}"
        );
        assert_eq!(
            body["success"], true,
            "every concurrent probe against the OK mock must succeed: {body}"
        );
    }

    // The shared per-row state converged to a single consistent
    // outcome: healthy + stamped, and the test button never flipped
    // `enabled` (it stays at the seeded `false`). A lost-update /
    // torn-write race would leave a stale or missing health record.
    let pool = pool_for(&server).await;
    let (enabled, status, reason, at) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled, "test-by-id never mutates `enabled`, even under concurrency");
    assert_eq!(status, "healthy", "concurrent probes converge to healthy");
    assert_eq!(reason, None, "healthy outcome carries no reason");
    assert!(at.is_some(), "last_health_check_at stamped by the winning write");
}

/// Tier-3 LIVE-credential connectivity test (the existing suite uses wiremock).
/// Exercises the real `test_repository_connectivity` path against Hugging Face's
/// `whoami-v2` endpoint with the real `HUGGINGFACE_API_KEY` (shipped in
/// `tests/.env.test`). Soft-skips when the key is unset (mirrors the other
/// real-credential tests' pattern).
#[tokio::test]
async fn live_huggingface_connection_test_with_real_credentials() {
    let hf_key = match std::env::var("HUGGINGFACE_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            eprintln!(
                "Skipping live_huggingface_connection_test_with_real_credentials — HUGGINGFACE_API_KEY unset"
            );
            return;
        }
    };

    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "repo_live", &["llm_repositories::read"]).await;

    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories/test"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "HF Live",
            "url": "https://huggingface.co",
            "auth_type": "api_key",
            "auth_config": {
                "api_key": hf_key,
                "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["success"], true,
        "live Hugging Face connection test with a real token must succeed: {body}"
    );
}

// ─── Tier-3 real-credential probe (HuggingFace, key-gated) ──────────────────

/// Tier-3: the test-connection-by-id probe SUCCEEDS against the LIVE
/// HuggingFace API with a real token — the path the wiremock tests
/// above can only simulate. Soft-skips when `HUGGINGFACE_API_KEY` is
/// unset (matching the suite's real-credential gating); `tests/.env.test`
/// ships a working key so this runs in the normal suite.
///
/// Shape mirrors the production HuggingFace repository: `auth_type:
/// "api_key"`, `url` containing `huggingface.co` (so the probe sends
/// `Authorization: Bearer <key>`), and `auth_test_api_endpoint` pointed
/// at the real `whoami-v2` endpoint, which returns HTTP 200 for a valid
/// token (the only status `test_repository_connectivity` treats as a pass).
#[tokio::test]
async fn test_by_id_real_huggingface_credentials_probe_succeeds() {
    let api_key = match std::env::var("HUGGINGFACE_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            eprintln!(
                "skipping test_by_id_real_huggingface_credentials_probe_succeeds: \
                 HUGGINGFACE_API_KEY not set"
            );
            return;
        }
    };

    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;

    // Create the repo DISABLED so the create-flow probe is skipped; the
    // real live probe is exercised explicitly via test-by-id below. The
    // api_key persists encrypted and is read back decrypted by the
    // by-id handler exactly as the runtime spawn path reads it.
    let created: Value = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "hf-live",
            "url": "https://huggingface.co",
            "auth_type": "api_key",
            "enabled": false,
            "auth_config": {
                "api_key": api_key,
                "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2",
            },
        }))
        .send()
        .await
        .expect("create request failed")
        .json()
        .await
        .expect("create response not json");
    let repo_id = Uuid::parse_str(created["id"].as_str().expect("id")).expect("uuid");

    // Probe the persisted config against the LIVE HuggingFace endpoint.
    let resp: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .expect("test-by-id request failed")
        .json()
        .await
        .expect("test-by-id response not json");

    assert_eq!(
        resp["success"], true,
        "live HuggingFace probe with a real token must succeed: {resp}"
    );

    // The live pass persists a `healthy` health record (and stamps the
    // probe time); the row stays disabled (the test button never enables).
    let pool = pool_for(&server).await;
    let (enabled, status, reason, at) = read_repo_row(&pool, repo_id).await;
    assert!(!enabled, "test-by-id does not change `enabled`");
    assert_eq!(status, "healthy", "live HF probe recorded as healthy");
    assert_eq!(reason, None);
    assert!(at.is_some(), "last_health_check_at stamped after the live probe");
}
