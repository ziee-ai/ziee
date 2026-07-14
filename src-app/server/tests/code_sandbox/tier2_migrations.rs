//! Tier 2 — Migrations 35 + 36 applied correctly.

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;

/// Migration 36's read-only-auto-approve backfill body, inlined here.
/// MIGRATE-squash (N8) squashed away the historical `00000000000036_seed_...`
/// file — it was a ONE-TIME backfill of existing users' `auto_approved_tools`
/// (new users inherit the table default `'[]'`), so a fresh squash baseline
/// drops it. This test still validates the backfill SQL's row-level idempotent
/// merge logic, so the body is inlined (as the original migration's comment
/// invited) rather than `include_str!`-ing the deleted file.
const MIGRATION_36_BODY: &str = r#"DO $$
DECLARE
    sandbox_id CONSTANT TEXT := 'b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd';
    read_only_tools CONSTANT JSONB := '["read_file", "list_files", "get_resource_link"]'::jsonb;
    rec RECORD;
    updated JSONB;
    found_idx INT;
    existing_tools JSONB;
    merged_tools JSONB;
BEGIN
    FOR rec IN SELECT id, auto_approved_tools FROM user_mcp_defaults LOOP
        found_idx := NULL;
        FOR i IN 0..jsonb_array_length(rec.auto_approved_tools) - 1 LOOP
            IF rec.auto_approved_tools -> i ->> 'server_id' = sandbox_id THEN
                found_idx := i;
                EXIT;
            END IF;
        END LOOP;

        IF found_idx IS NULL THEN
            updated := rec.auto_approved_tools
                     || jsonb_build_array(
                         jsonb_build_object('server_id', sandbox_id, 'tools', read_only_tools)
                     );
        ELSE
            existing_tools := COALESCE(rec.auto_approved_tools -> found_idx -> 'tools', '[]'::jsonb);
            merged_tools := existing_tools;
            FOR i IN 0..jsonb_array_length(read_only_tools) - 1 LOOP
                IF NOT merged_tools @> jsonb_build_array(read_only_tools -> i) THEN
                    merged_tools := merged_tools || jsonb_build_array(read_only_tools -> i);
                END IF;
            END LOOP;
            updated := jsonb_set(
                rec.auto_approved_tools,
                ARRAY[found_idx::text],
                jsonb_build_object('server_id', sandbox_id, 'tools', merged_tools)
            );
        END IF;

        UPDATE user_mcp_defaults
        SET auto_approved_tools = updated,
            updated_at = NOW()
        WHERE id = rec.id
          AND auto_approved_tools IS DISTINCT FROM updated;
    END LOOP;
END $$;"#;

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
        r#"INSERT INTO users (id, username, email, password_hash, is_active)
           VALUES ($1, $2, $3, 'x', true)"#,
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
    let body = MIGRATION_36_BODY;
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
        r#"INSERT INTO users (id, username, email, password_hash, is_active)
           VALUES ($1, $2, $3, 'x', true)"#,
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

    let body = MIGRATION_36_BODY;
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
