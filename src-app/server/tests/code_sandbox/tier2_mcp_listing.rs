//! Tier 2 — Sandbox row appears in McpRepository::list_accessible for
//! users in the default group.

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;
use ziee::code_sandbox::{code_sandbox_server_id, CodeSandboxRepository};

#[tokio::test]
async fn sandbox_row_is_accessible_to_default_group_user() {
    let server = TestServer::start().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    // Seed the sandbox row + group attachment.
    let repo = CodeSandboxRepository::new(pool.clone());
    let sandbox_id = code_sandbox_server_id();
    repo.upsert_builtin_server(sandbox_id, "http://127.0.0.1:9999/api/code-sandbox")
        .await
        .unwrap();

    // Create a user and add them to the default group.
    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash, is_active)
           VALUES ($1, $2, $3, 'x', true)"#,
    )
    .bind(user_id)
    .bind(format!("listing-{user_id}"))
    .bind(format!("listing-{user_id}@x.test"))
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO user_groups (user_id, group_id, assigned_by)
           SELECT $1, g.id, $1 FROM groups g
           WHERE g.is_default = TRUE AND g.is_system = TRUE
           ON CONFLICT DO NOTHING"#,
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Hit the mcp repository directly to confirm the sandbox shows up
    // for this user.
    use ziee::mcp::McpRepository;
    let mcp_repo = McpRepository::new(pool.clone());
    let resp = mcp_repo
        .list_accessible(user_id, 1, 100)
        .await
        .expect("list_accessible");

    let sandbox = resp
        .servers
        .iter()
        .find(|s| s.id == sandbox_id)
        .expect("sandbox row missing from list_accessible for default-group user");

    assert!(sandbox.is_built_in, "sandbox row must have is_built_in=true");
    assert_eq!(
        sandbox.transport_type.to_string(),
        "http",
        "sandbox must be http transport"
    );
}

#[tokio::test]
async fn sandbox_row_is_marked_built_in_protected() {
    // The built-in delete protection lives in mcp/repository.rs:920
    // (`delete_system_mcp_server` rejects with BUILT_IN_SERVER when
    // is_built_in=true). We verify by reading the flag back; the actual
    // route-level rejection is covered by mcp tests.
    let server = TestServer::start().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let repo = CodeSandboxRepository::new(pool.clone());
    repo.upsert_builtin_server(
        code_sandbox_server_id(),
        "http://127.0.0.1:9999/api/code-sandbox",
    )
    .await
    .unwrap();

    let (is_built_in,): (bool,) =
        sqlx::query_as("SELECT is_built_in FROM mcp_servers WHERE id = $1")
            .bind(code_sandbox_server_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(is_built_in);
}
