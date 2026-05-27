//! Tier 2 — Repository SQL bodies against a real Postgres.
//!
//! Validates the three SQL contracts in `code_sandbox::repository`:
//!   1. `get_conversation_user_id` / `get_conversation_files`
//!   2. `get_file_by_id` denies foreign-user access
//!   3. `upsert_builtin_server` is idempotent AND does NOT overwrite
//!      `enabled` on conflict (the admin-disable-survives-restart guarantee)

use uuid::Uuid;

use crate::common::TestServer;
use ziee::code_sandbox::{code_sandbox_server_id, CodeSandboxRepository};

async fn repo(server: &TestServer) -> CodeSandboxRepository {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    CodeSandboxRepository::new(pool)
}

// ─── upsert_builtin_server ──────────────────────────────────────────

#[tokio::test]
async fn upsert_builtin_server_is_idempotent() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let id = code_sandbox_server_id();

    repo.upsert_builtin_server(id, "http://127.0.0.1:9999/api/code-sandbox")
        .await
        .expect("first upsert");
    repo.upsert_builtin_server(id, "http://127.0.0.1:9999/api/code-sandbox")
        .await
        .expect("second upsert");

    // Both calls must leave exactly one row.
    let pool = repo.pool();
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_builtin_server_preserves_enabled_on_conflict() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let id = code_sandbox_server_id();

    // Insert.
    repo.upsert_builtin_server(id, "http://127.0.0.1:9999/api/code-sandbox")
        .await
        .expect("first upsert");

    // Admin disables via UI (simulated as direct UPDATE).
    let pool = repo.pool();
    sqlx::query("UPDATE mcp_servers SET enabled = false WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();

    // Restart-equivalent upsert.
    repo.upsert_builtin_server(id, "http://127.0.0.1:9999/api/code-sandbox")
        .await
        .expect("second upsert");

    // The contract: enabled must STILL be false.
    let (enabled,): (bool,) = sqlx::query_as("SELECT enabled FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(
        !enabled,
        "admin-disable was overwritten by boot-time upsert (the bug the contract prevents)"
    );
}

#[tokio::test]
async fn upsert_builtin_server_attaches_to_default_group() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let id = code_sandbox_server_id();

    repo.upsert_builtin_server(id, "http://127.0.0.1:9999/api/code-sandbox")
        .await
        .expect("upsert");

    let pool = repo.pool();
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM user_group_mcp_servers ug
        JOIN groups g ON g.id = ug.group_id
        WHERE ug.mcp_server_id = $1 AND g.is_default = TRUE
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "sandbox row must attach to the default group");
}

#[tokio::test]
async fn upsert_builtin_server_sets_expected_columns() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let id = code_sandbox_server_id();
    let url = "http://127.0.0.1:9999/api/code-sandbox";

    repo.upsert_builtin_server(id, url).await.expect("upsert");

    let pool = repo.pool();
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        transport_type: String,
        is_built_in: bool,
        is_system: bool,
        url: Option<String>,
        timeout_seconds: i32,
        supports_sampling: bool,
        usage_mode: String,
        max_concurrent_sessions: Option<i32>,
    }
    let row: Row = sqlx::query_as("SELECT name, transport_type, is_built_in, is_system, url, timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(row.name, "code_sandbox");
    assert_eq!(row.transport_type, "http");
    assert!(row.is_built_in);
    assert!(row.is_system);
    assert_eq!(row.url.as_deref(), Some(url));
    assert_eq!(row.timeout_seconds, 620);
    assert!(!row.supports_sampling);
    assert_eq!(row.usage_mode, "auto");
    assert_eq!(row.max_concurrent_sessions, Some(1));
}

// ─── get_conversation_files / get_conversation_user_id ──────────────

#[tokio::test]
async fn get_conversation_files_returns_empty_for_nonexistent_conv() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let bogus = Uuid::new_v4();
    let files = repo
        .get_conversation_files(bogus)
        .await
        .expect("query ok");
    assert!(files.is_empty());
}

#[tokio::test]
async fn get_conversation_user_id_returns_none_for_missing() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let bogus = Uuid::new_v4();
    let uid = repo.get_conversation_user_id(bogus).await.expect("query ok");
    assert!(uid.is_none());
}

#[tokio::test]
async fn get_file_by_id_denies_foreign_user() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;

    // Insert a file owned by user A.
    let pool = repo.pool();
    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash, is_active)
           VALUES ($1, $2, $3, 'x', true),
                  ($4, $5, $6, 'x', true)"#,
    )
    .bind(user_a)
    .bind(format!("a-{}", user_a))
    .bind(format!("a-{}@x.test", user_a))
    .bind(user_b)
    .bind(format!("b-{}", user_b))
    .bind(format!("b-{}@x.test", user_b))
    .execute(pool)
    .await
    .unwrap();

    let file_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO files (id, user_id, filename, file_size, mime_type)
           VALUES ($1, $2, 'a.txt', 10, 'text/plain')"#,
    )
    .bind(file_id)
    .bind(user_a)
    .execute(pool)
    .await
    .unwrap();

    // Owner can fetch.
    let got_a = repo
        .get_file_by_id(file_id, user_a)
        .await
        .expect("query ok");
    assert!(got_a.is_some(), "owner must be able to fetch their file");

    // Foreign user is denied (returns None — not even an error to
    // distinguish existence).
    let got_b = repo
        .get_file_by_id(file_id, user_b)
        .await
        .expect("query ok");
    assert!(got_b.is_none(), "foreign user must NOT see the file");
}

// ─── JSONB defense regressions ───────────────────────────────────────

/// Regression for the JSONB-UUID-cast hardening in commit a3fc827.
/// Previously `(content ->> 'file_id')::uuid` would raise a query-level
/// "invalid input syntax for type uuid" if any message in the
/// conversation had a malformed file_id string — breaking
/// build_context for the owning conversation. The regex filter in
/// the SQL CTE means malformed entries are silently dropped (they
/// couldn't have resolved to a real `files` row anyway).
#[tokio::test]
async fn get_conversation_files_filters_malformed_uuid_in_jsonb() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let pool = repo.pool();

    // Set up a user + conversation + branch + message + a message_content
    // whose content.file_id is NOT a valid UUID.
    let user_id = Uuid::new_v4();
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash, is_active)
           VALUES ($1, $2, $3, 'x', true)"#,
    )
    .bind(user_id)
    .bind(format!("u_{}", &user_id.to_string()[..8]))
    .bind(format!("u_{}@example.com", &user_id.to_string()[..8]))
    .execute(pool)
    .await
    .unwrap();
    // FK ordering: conversation row first (with active_branch_id NULL),
    // then branch (FK back to conversation), then UPDATE conversation
    // to point active_branch_id at the new branch.
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 't', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(branch_id)
    .bind(conv_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE conversations SET active_branch_id = $1 WHERE id = $2")
        .bind(branch_id)
        .bind(conv_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO messages (id, role, originated_from_id, created_at)
           VALUES ($1, 'user', $1, NOW())"#,
    )
    .bind(msg_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branch_messages (branch_id, message_id, created_at)
           VALUES ($1, $2, NOW())"#,
    )
    .bind(branch_id)
    .bind(msg_id)
    .execute(pool)
    .await
    .unwrap();
    // The poisoned message_content: content.file_id is a garbage string.
    sqlx::query(
        r#"INSERT INTO message_contents (id, message_id, content_type, content, sequence_order, created_at, updated_at)
           VALUES (gen_random_uuid(), $1, 'file_attachment',
                   '{"file_id":"not-a-uuid"}'::jsonb,
                   0, NOW(), NOW())"#,
    )
    .bind(msg_id)
    .execute(pool)
    .await
    .unwrap();

    // Without the regex filter, this would raise an "invalid input
    // syntax for type uuid" error. With the filter, the malformed
    // entry is silently dropped and we get an empty Vec.
    let files = repo
        .get_conversation_files(conv_id)
        .await
        .expect("query must succeed despite malformed file_id JSON");
    assert!(
        files.is_empty(),
        "malformed file_id entries must be silently filtered, got: {files:?}"
    );
}
