//! workflow_mcp `upsert_builtin_server` — Tier-2 repository idempotency +
//! enabled-preservation (audit gap M7).
//!
//! Mirrors `code_sandbox/tier2_repository.rs`'s `upsert_builtin_server_*` tests
//! against the REAL `WorkflowMcpRepository` (re-exported for tests as
//! `ziee::workflow_mcp::WorkflowMcpRepository`, paralleling
//! `ziee::code_sandbox::CodeSandboxRepository`). The contract under test:
//!
//!   1. Calling `upsert_builtin_server` twice leaves exactly ONE row (the
//!      `ON CONFLICT (id) DO UPDATE` is idempotent on the deterministic
//!      `workflow_mcp_server_id()`).
//!   2. An admin-set `enabled = false` SURVIVES a second upsert — the
//!      conflict-update touches only the identity columns (is_system /
//!      is_built_in / transport_type / url / updated_at), deliberately leaving
//!      `enabled` (and other admin-editable columns) untouched. This is the
//!      "admin-disable survives restart" guarantee: a boot-time re-registration
//!      must never silently re-enable a server the admin turned off.
//!   3. The row carries the expected identity columns the workflow upsert SQL
//!      asserts (name='workflow', built-in, system, http, the loopback url).
//!
//! NOTE: `TestServer::start()` boots the server, whose `WorkflowMcpModule::init`
//! spawns its OWN `upsert_builtin_server` for the loopback port. So the row may
//! already exist from boot before the test's explicit calls — which is exactly
//! the point: the explicit re-upserts must be idempotent on top of the boot one.
//! Each test polls for the boot row first, then drives the explicit sequence so
//! the `SET enabled = false` can't race ahead of a late boot upsert.

use uuid::Uuid;

use crate::common::TestServer;
use ziee::workflow_mcp::{workflow_mcp_server_id, WorkflowMcpRepository};

/// The loopback url shape the module uses (any valid `/api/workflows/mcp` url
/// works for the upsert; the conflict-update overwrites `url` each call).
const LOOPBACK_URL: &str = "http://127.0.0.1:9999/api/workflows/mcp";

/// Open a small pool on the per-test DB.
async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db")
}

/// Poll until the boot-spawned `upsert_builtin_server` has inserted the row, so
/// a subsequent explicit upsert + UPDATE sequence isn't raced by a late boot
/// upsert.
async fn wait_for_boot_row(pool: &sqlx::PgPool, id: Uuid) {
    for _ in 0..40 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("count workflow built-in row");
        if count >= 1 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("workflow_mcp built-in row never registered at boot within ~10s");
}

#[tokio::test]
async fn upsert_builtin_server_is_idempotent_no_duplicate_row() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let id = workflow_mcp_server_id();
    wait_for_boot_row(&pool, id).await;

    let repo = WorkflowMcpRepository::new(pool.clone());
    // Two explicit upserts on top of the boot one — all share the same id.
    repo.upsert_builtin_server(id, LOOPBACK_URL)
        .await
        .expect("first explicit upsert");
    repo.upsert_builtin_server(id, LOOPBACK_URL)
        .await
        .expect("second explicit upsert");

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "repeated upserts on the deterministic id must leave exactly one row"
    );
    pool.close().await;
}

#[tokio::test]
async fn upsert_builtin_server_preserves_admin_disabled_enabled() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let id = workflow_mcp_server_id();
    wait_for_boot_row(&pool, id).await;

    let repo = WorkflowMcpRepository::new(pool.clone());
    // Establish the row (idempotent on top of boot).
    repo.upsert_builtin_server(id, LOOPBACK_URL)
        .await
        .expect("establishing upsert");

    // Admin disables it via the UI (simulated as a direct UPDATE).
    sqlx::query("UPDATE mcp_servers SET enabled = false WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .expect("admin-disable update");

    // A restart-equivalent re-upsert must NOT re-enable it.
    repo.upsert_builtin_server(id, LOOPBACK_URL)
        .await
        .expect("restart-equivalent upsert");

    let (enabled,): (bool,) = sqlx::query_as("SELECT enabled FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !enabled,
        "admin-disable was overwritten by a re-upsert (the bug the conflict-update prevents)"
    );
    pool.close().await;
}

#[tokio::test]
async fn upsert_builtin_server_sets_expected_identity_columns() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let id = workflow_mcp_server_id();
    wait_for_boot_row(&pool, id).await;

    let repo = WorkflowMcpRepository::new(pool.clone());
    repo.upsert_builtin_server(id, LOOPBACK_URL)
        .await
        .expect("upsert");

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        display_name: Option<String>,
        transport_type: String,
        is_built_in: bool,
        is_system: bool,
        url: Option<String>,
        timeout_seconds: i32,
        supports_sampling: bool,
        usage_mode: String,
        max_concurrent_sessions: Option<i32>,
    }
    let row: Row = sqlx::query_as(
        "SELECT name, display_name, transport_type, is_built_in, is_system, url, \
         timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions \
         FROM mcp_servers WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.name, "workflow", "the built-in server is named 'workflow'");
    assert_eq!(row.display_name.as_deref(), Some("Workflows"));
    assert_eq!(row.transport_type, "http");
    assert!(row.is_built_in, "the row is a built-in server");
    assert!(row.is_system, "the row is a system server");
    // The explicit upsert's conflict-update overwrites `url` with our loopback.
    assert_eq!(
        row.url.as_deref(),
        Some(LOOPBACK_URL),
        "the conflict-update keeps the live loopback url current"
    );
    assert_eq!(row.timeout_seconds, 30);
    assert!(!row.supports_sampling);
    assert_eq!(row.usage_mode, "auto");
    assert_eq!(row.max_concurrent_sessions, Some(8));
    pool.close().await;
}
