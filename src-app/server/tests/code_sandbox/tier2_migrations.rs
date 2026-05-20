//! Tier 2 — Migrations 35 + 36 applied correctly.

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;

async fn pool(server: &TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

// ─── Migration 35: code_sandbox::execute permission ─────────────────

#[tokio::test]
async fn migration_35_grants_code_sandbox_execute_to_default_group() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let (perms,): (Vec<String>,) = sqlx::query_as(
        r#"SELECT permissions FROM groups
           WHERE name = 'Users' AND is_system = TRUE AND is_default = TRUE"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        perms.iter().any(|p| p == "code_sandbox::execute"),
        "default Users group missing code_sandbox::execute permission; perms = {perms:?}"
    );
}

#[tokio::test]
async fn migration_35_is_idempotent() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    // Re-execute the migration's UPDATE manually; the WHERE clause's
    // NOT … = ANY guard means the second run is a no-op.
    let updated: sqlx::postgres::PgQueryResult = sqlx::query(
        r#"
        UPDATE groups
        SET permissions = array_append(permissions, 'code_sandbox::execute'),
            updated_at = NOW()
        WHERE name = 'Users'
          AND is_system = TRUE
          AND is_default = TRUE
          AND NOT ('code_sandbox::execute' = ANY(permissions))
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(updated.rows_affected(), 0, "rerun must be a no-op");
}

// ─── Migration 36: read-only auto-approve seed ──────────────────────

#[tokio::test]
async fn migration_36_seeds_read_only_tools_for_new_user_defaults() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // Create a user + user_mcp_defaults row. The migration runs at
    // build-time so existing rows are already seeded; we explicitly
    // insert a row then re-run the migration body to confirm it
    // updates the new row too.
    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash, is_active, is_protected, profile)
           VALUES ($1, $2, $3, 'x', true, false, '{}'::jsonb)"#,
    )
    .bind(user_id)
    .bind(format!("u-{user_id}"))
    .bind(format!("u-{user_id}@x.test"))
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO user_mcp_defaults (user_id, approval_mode, auto_approved_tools, disabled_servers)
           VALUES ($1, 'manual_approve', '[]'::jsonb, '[]'::jsonb)"#,
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Re-run the migration 36 body. Loading the migration file would
    // require sqlx::migrate machinery; the body is short enough to
    // inline as a single anonymous PL/pgSQL block here.
    let body = include_str!("../../migrations/00000000000036_seed_code_sandbox_read_only_auto_approve.sql");
    sqlx::raw_sql(body).execute(&pool).await.unwrap();

    let (autos,): (serde_json::Value,) = sqlx::query_as(
        "SELECT auto_approved_tools FROM user_mcp_defaults WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let arr = autos.as_array().expect("auto_approved_tools is array");
    let sandbox = arr
        .iter()
        .find(|entry| {
            entry["server_id"].as_str()
                == Some("b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd")
        })
        .expect("sandbox entry must be present after migration");

    let tools: Vec<String> = sandbox["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap().to_string())
        .collect();

    for must_have in &["read_file", "list_files", "get_resource_link"] {
        assert!(
            tools.iter().any(|t| t == must_have),
            "read-only tool {must_have} missing from auto-approve: {tools:?}"
        );
    }
    for must_not in &["execute_command", "write_file", "edit_file"] {
        assert!(
            !tools.iter().any(|t| t == must_not),
            "mutation tool {must_not} must NOT be auto-approved: {tools:?}"
        );
    }
}

#[tokio::test]
async fn migration_36_is_idempotent() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash, is_active, is_protected, profile)
           VALUES ($1, $2, $3, 'x', true, false, '{}'::jsonb)"#,
    )
    .bind(user_id)
    .bind(format!("u2-{user_id}"))
    .bind(format!("u2-{user_id}@x.test"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO user_mcp_defaults (user_id, approval_mode, auto_approved_tools, disabled_servers)
           VALUES ($1, 'manual_approve', '[]'::jsonb, '[]'::jsonb)"#,
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let body = include_str!("../../migrations/00000000000036_seed_code_sandbox_read_only_auto_approve.sql");
    sqlx::raw_sql(body).execute(&pool).await.unwrap();
    let (first,): (serde_json::Value,) = sqlx::query_as(
        "SELECT auto_approved_tools FROM user_mcp_defaults WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(body).execute(&pool).await.unwrap();
    let (second,): (serde_json::Value,) = sqlx::query_as(
        "SELECT auto_approved_tools FROM user_mcp_defaults WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        first, second,
        "rerunning migration 36 changed auto_approved_tools — not idempotent"
    );
}
