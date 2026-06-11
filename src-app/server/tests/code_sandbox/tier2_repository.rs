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

// ─── project_refs UNION path ─────────────────────────────────────────
//
// `get_conversation_files` was extended (commit 09b81114, the
// `project_refs` CTE + `UNION`) so that the sandbox sees the same
// effective file set as the chat: a conversation's PROJECT knowledge
// files now surface alongside its message attachments. These tests pin
// that path — the positive surface, the DISTINCT/ORDER BY behavior, and
// the deliberate absence of a `user_id` predicate.

/// A project knowledge file (attached via `project_files`, linked to a
/// conversation via `project_conversations`) surfaces in the sandbox
/// file set even though it is NOT a chat-message attachment.
#[tokio::test]
async fn get_conversation_files_surfaces_project_knowledge_file() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let pool = repo.pool();

    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let conv_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

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
    sqlx::query(
        r#"INSERT INTO projects (id, user_id, name)
           VALUES ($1, $2, 'P')"#,
    )
    .bind(project_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    // The conversation does NOT need an active_branch_id: project files
    // surface via project_conversations, independent of branch walking.
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
        r#"INSERT INTO files (id, user_id, filename, file_size, mime_type)
           VALUES ($1, $2, 'knowledge.csv', 20, 'text/csv')"#,
    )
    .bind(file_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO project_files (project_id, file_id)
           VALUES ($1, $2)"#,
    )
    .bind(project_id)
    .bind(file_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO project_conversations (conversation_id, project_id)
           VALUES ($1, $2)"#,
    )
    .bind(conv_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    let files = repo
        .get_conversation_files(conv_id)
        .await
        .expect("query ok");
    assert_eq!(files.len(), 1, "exactly the one project file, got: {files:?}");
    assert_eq!(files[0].file_id, file_id);
    assert_eq!(files[0].filename, "knowledge.csv");
}

/// The same file attached as BOTH a project knowledge file AND a chat
/// attachment must appear exactly ONCE — pins the `SELECT DISTINCT`
/// across the attachment_refs ∪ project_refs union.
#[tokio::test]
async fn get_conversation_files_dedups_project_and_attachment_overlap() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let pool = repo.pool();

    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    let msg_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

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
    sqlx::query("INSERT INTO projects (id, user_id, name) VALUES ($1, $2, 'P')")
        .bind(project_id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    // FK ordering: conversation (active_branch_id NULL) → branch →
    // UPDATE conversation to point active_branch_id at the branch.
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
        r#"INSERT INTO files (id, user_id, filename, file_size, mime_type)
           VALUES ($1, $2, 'shared.txt', 10, 'text/plain')"#,
    )
    .bind(file_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    // Path 1: project knowledge file.
    sqlx::query("INSERT INTO project_files (project_id, file_id) VALUES ($1, $2)")
        .bind(project_id)
        .bind(file_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO project_conversations (conversation_id, project_id) VALUES ($1, $2)",
    )
    .bind(conv_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();
    // Path 2: the SAME file, also a chat-message attachment.
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
    sqlx::query(
        r#"INSERT INTO message_contents (id, message_id, content_type, content, sequence_order, created_at, updated_at)
           VALUES (gen_random_uuid(), $1, 'file_attachment',
                   jsonb_build_object('file_id', $2::text),
                   0, NOW(), NOW())"#,
    )
    .bind(msg_id)
    .bind(file_id)
    .execute(pool)
    .await
    .unwrap();

    let files = repo
        .get_conversation_files(conv_id)
        .await
        .expect("query ok");
    assert_eq!(
        files.len(),
        1,
        "a file in BOTH the project and an attachment must dedup to one, got: {files:?}"
    );
    assert_eq!(files[0].file_id, file_id);
}

/// Two project files that share an identical `created_at` must come back
/// in a stable order — `ORDER BY f.created_at, f.id` breaks the tie on
/// `f.id`. This keeps the downstream collision-suffixing in
/// `build_bwrap_argv` deterministic across calls (bulk project uploads
/// frequently share a created_at).
#[tokio::test]
async fn get_conversation_files_orders_ties_by_file_id() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let pool = repo.pool();

    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let conv_id = Uuid::new_v4();
    // Two file ids with a deterministic ordering relationship: the
    // all-zeros / all-`a` ids sort unambiguously by uuid bytes.
    let lo = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let hi = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();

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
    sqlx::query("INSERT INTO projects (id, user_id, name) VALUES ($1, $2, 'P')")
        .bind(project_id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 't', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    // BOTH files pinned to the EXACT same created_at so only the
    // `f.id` tiebreaker decides order.
    sqlx::query(
        r#"INSERT INTO files (id, user_id, filename, file_size, mime_type, created_at)
           VALUES ($1, $3, 'lo.txt', 10, 'text/plain', TIMESTAMPTZ '2020-01-01 00:00:00+00'),
                  ($2, $3, 'hi.txt', 10, 'text/plain', TIMESTAMPTZ '2020-01-01 00:00:00+00')"#,
    )
    .bind(lo)
    .bind(hi)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO project_files (project_id, file_id)
           VALUES ($1, $2), ($1, $3)"#,
    )
    .bind(project_id)
    .bind(lo)
    .bind(hi)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO project_conversations (conversation_id, project_id) VALUES ($1, $2)",
    )
    .bind(conv_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    let files = repo
        .get_conversation_files(conv_id)
        .await
        .expect("query ok");
    let ids: Vec<Uuid> = files.iter().map(|f| f.file_id).collect();
    assert_eq!(
        ids,
        vec![lo, hi],
        "shared created_at must sort by f.id ascending, got: {ids:?}"
    );
}

/// Regression-pinning of the DOCUMENTED design: the `project_refs` CTE
/// has NO `f.user_id` predicate. If a future bug ever lets a foreign
/// file into `project_files` (the attach handler — NOT this query — is
/// the tenant guard, 404ing foreign files before insert), this query
/// STILL returns it. We assert that on purpose so any change to the
/// invariant trips a test: the protection lives at the handler
/// boundary, and a defense-in-depth `user_id` filter here would be a
/// deliberate design change, not a silent one.
#[tokio::test]
async fn get_conversation_files_project_path_has_no_user_id_predicate() {
    let server = TestServer::start().await;
    let repo = repo(&server).await;
    let pool = repo.pool();

    let user_a = Uuid::new_v4(); // project + conversation owner
    let user_b = Uuid::new_v4(); // foreign file owner
    let project_id = Uuid::new_v4();
    let conv_id = Uuid::new_v4();
    let foreign_file = Uuid::new_v4();

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
    sqlx::query("INSERT INTO projects (id, user_id, name) VALUES ($1, $2, 'P')")
        .bind(project_id)
        .bind(user_a)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 't', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(user_a)
    .execute(pool)
    .await
    .unwrap();
    // A file owned by user B, inserted directly via SQL — bypassing the
    // attach-handler 404 guard that would normally reject a foreign file.
    sqlx::query(
        r#"INSERT INTO files (id, user_id, filename, file_size, mime_type)
           VALUES ($1, $2, 'foreign.txt', 10, 'text/plain')"#,
    )
    .bind(foreign_file)
    .bind(user_b)
    .execute(pool)
    .await
    .unwrap();
    // Simulate a future bug breaking the handler invariant: B's file is
    // in A's project_files.
    sqlx::query("INSERT INTO project_files (project_id, file_id) VALUES ($1, $2)")
        .bind(project_id)
        .bind(foreign_file)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO project_conversations (conversation_id, project_id) VALUES ($1, $2)",
    )
    .bind(conv_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    // INTENTIONAL: the SQL has no user_id filter, so B's file STILL
    // surfaces. This documents that the cross-tenant guard lives at the
    // attach-handler boundary, not in this query. If this assertion ever
    // flips, someone added defense-in-depth here on purpose — update the
    // test to match the new (intended) contract.
    let files = repo
        .get_conversation_files(conv_id)
        .await
        .expect("query ok");
    assert_eq!(files.len(), 1, "got: {files:?}");
    assert_eq!(
        files[0].file_id, foreign_file,
        "project_refs has no user_id predicate; the foreign file surfaces \
         (handler boundary is the tenant guard, not this query)"
    );
}
