//! Phase 7 / §13.6 — `hub_entities.hub_id` reverse-DNS rewrite migration.
//!
//! Verifies migration 89 (rewrite_hub_entities_hub_id_to_reverse_dns):
//!
//! 1. Known slug → reverse-DNS mapping rewrites correctly.
//! 2. Idempotent for rows already in reverse-DNS form (and idempotent
//!    when re-run a second time on the same dataset).
//! 3. Unknown slug-shaped rows are left untouched (the operator
//!    reinstalls them to re-track).
//!
//! The migration is already applied at test DB setup. To exercise the
//! rewrite logic, we insert post-migration rows that look pre-migration
//! (slug-shaped) and re-execute the migration SQL via `sqlx::raw_sql`.
//! This mirrors the pattern in `tests/code_sandbox/tier2_migrations.rs`.

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;

const MIGRATION_SQL: &str = include_str!(
    "../../migrations/00000000000089_rewrite_hub_entities_hub_id_to_reverse_dns.sql"
);

async fn pool(server: &TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test database")
}

async fn insert_hub_entity(
    pool: &sqlx::PgPool,
    entity_type: &str,
    hub_category: &str,
    hub_id: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO hub_entities (id, entity_type, entity_id, hub_id, hub_category)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(hub_id)
    .bind(hub_category)
    .execute(pool)
    .await
    .expect("insert hub_entities row");
    id
}

async fn fetch_hub_id(pool: &sqlx::PgPool, id: Uuid) -> String {
    let (hub_id,): (String,) =
        sqlx::query_as("SELECT hub_id FROM hub_entities WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("fetch hub_id");
    hub_id
}

// =====================================================================
// 1. Known slug → reverse-DNS rewrite
// =====================================================================

#[tokio::test]
async fn hub_entities_hub_id_rewrite_migrates_known_slugs() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // Insert one row per category, each using a known slug.
    let mcp_id = insert_hub_entity(
        &pool,
        "mcp_server",
        "mcp_server",
        "filesystem-mcp",
    )
    .await;
    let model_id = insert_hub_entity(
        &pool,
        "llm_model",
        "model",
        "llama-3-1-8b-instruct",
    )
    .await;
    let assistant_id = insert_hub_entity(
        &pool,
        "assistant",
        "assistant",
        "code-reviewer",
    )
    .await;

    // Re-run the migration against the test DB.
    sqlx::raw_sql(MIGRATION_SQL)
        .execute(&pool)
        .await
        .expect("re-run migration");

    assert_eq!(
        fetch_hub_id(&pool, mcp_id).await,
        "io.github.modelcontextprotocol/filesystem",
        "filesystem-mcp slug should be rewritten"
    );
    assert_eq!(
        fetch_hub_id(&pool, model_id).await,
        "io.github.phibya/llama-3-1-8b-instruct",
        "llama-3-1-8b-instruct slug should be rewritten"
    );
    assert_eq!(
        fetch_hub_id(&pool, assistant_id).await,
        "io.github.phibya/code-reviewer",
        "code-reviewer slug should be rewritten"
    );

    pool.close().await;
}

// =====================================================================
// 2. Idempotent for reverse-DNS rows + re-run safety
// =====================================================================

#[tokio::test]
async fn hub_entities_hub_id_rewrite_is_idempotent_for_reverse_dns() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // Insert rows ALREADY in reverse-DNS form. The migration must skip
    // these (they contain `/`, so the `NOT LIKE '%/%'` guard fires).
    let preserved_a = insert_hub_entity(
        &pool,
        "mcp_server",
        "mcp_server",
        "io.github.modelcontextprotocol/filesystem",
    )
    .await;
    let preserved_b = insert_hub_entity(
        &pool,
        "assistant",
        "assistant",
        "io.github.phibya/code-reviewer",
    )
    .await;

    // Also insert one rewriteable slug so we can verify the migration
    // produces the same end-state across two runs.
    let rewriteable = insert_hub_entity(
        &pool,
        "mcp_server",
        "mcp_server",
        "memory-mcp",
    )
    .await;

    // First run.
    sqlx::raw_sql(MIGRATION_SQL)
        .execute(&pool)
        .await
        .expect("first re-run of migration");
    let after_first_a = fetch_hub_id(&pool, preserved_a).await;
    let after_first_b = fetch_hub_id(&pool, preserved_b).await;
    let after_first_rewritten = fetch_hub_id(&pool, rewriteable).await;

    // Second run — must produce identical end-state.
    sqlx::raw_sql(MIGRATION_SQL)
        .execute(&pool)
        .await
        .expect("second re-run of migration");
    let after_second_a = fetch_hub_id(&pool, preserved_a).await;
    let after_second_b = fetch_hub_id(&pool, preserved_b).await;
    let after_second_rewritten = fetch_hub_id(&pool, rewriteable).await;

    assert_eq!(
        after_first_a, "io.github.modelcontextprotocol/filesystem",
        "reverse-DNS hub_id should never change"
    );
    assert_eq!(
        after_first_b, "io.github.phibya/code-reviewer",
        "reverse-DNS hub_id should never change"
    );
    assert_eq!(
        after_first_rewritten, "io.github.modelcontextprotocol/memory",
        "memory-mcp slug should be rewritten on the first run"
    );

    assert_eq!(
        after_first_a, after_second_a,
        "rerunning migration changed a reverse-DNS row — not idempotent"
    );
    assert_eq!(
        after_first_b, after_second_b,
        "rerunning migration changed a reverse-DNS row — not idempotent"
    );
    assert_eq!(
        after_first_rewritten, after_second_rewritten,
        "rerunning migration changed a previously rewritten row — not idempotent"
    );

    pool.close().await;
}

// =====================================================================
// 3. Unknown slugs survive untouched
// =====================================================================

#[tokio::test]
async fn hub_entities_hub_id_rewrite_leaves_unknown_slugs_alone() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // Insert a slug NOT in the lookup table. The migration's RAISE
    // NOTICE warns the operator, but the row's hub_id stays as-is so
    // the row survives (visible in the orphan list) rather than getting
    // wiped or corrupted.
    let unknown_mcp = insert_hub_entity(
        &pool,
        "mcp_server",
        "mcp_server",
        "totally-made-up-server",
    )
    .await;
    let unknown_model = insert_hub_entity(
        &pool,
        "llm_model",
        "model",
        "never-released-model",
    )
    .await;

    sqlx::raw_sql(MIGRATION_SQL)
        .execute(&pool)
        .await
        .expect("re-run migration");

    assert_eq!(
        fetch_hub_id(&pool, unknown_mcp).await,
        "totally-made-up-server",
        "unknown slug should be left untouched"
    );
    assert_eq!(
        fetch_hub_id(&pool, unknown_model).await,
        "never-released-model",
        "unknown slug should be left untouched"
    );

    pool.close().await;
}
