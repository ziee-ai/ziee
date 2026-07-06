//! `hub_entities.hub_id` reverse-DNS rewrite migrations.
//!
//! Verifies migration 92 (rewrite_hub_entities_hub_id_to_reverse_dns):
//!
//! 1. Known slug → reverse-DNS mapping rewrites correctly.
//! 2. Idempotent for rows already in reverse-DNS form (and idempotent
//!    when re-run a second time on the same dataset).
//! 3. Unknown slug-shaped rows are left untouched (the operator
//!    reinstalls them to re-track).
//!
//! …and migration 131 (rewrite_hub_ids_phibya_to_ziee_ai), the org-migration
//! Phase-2 publisher rebrand `io.github.phibya/*` → `io.github.ziee-ai/*`:
//!
//! 4. A `io.github.phibya/*` `hub_id` is rewritten to `io.github.ziee-ai/*`;
//!    non-`phibya` reverse-DNS rows are untouched; idempotent on re-run.
//! 5. The 92→131 chain composes: a pre-§12 slug rewrites to the personal
//!    namespace under 92, then to the org namespace under 131.
//!
//! Each migration is already applied at test DB setup. To exercise a rewrite
//! in isolation, we insert rows that look pre-migration and re-execute that
//! migration's SQL via `sqlx::raw_sql` (migration 92 legitimately still emits
//! the `io.github.phibya/*` form — 131 is the step that moves it to the org
//! namespace). This mirrors the pattern in
//! `tests/code_sandbox/tier2_migrations.rs`.

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;

const MIGRATION_SQL: &str = include_str!(
    "../../migrations/00000000000092_rewrite_hub_entities_hub_id_to_reverse_dns.sql"
);

const MIGRATION_131_SQL: &str = include_str!(
    "../../migrations/00000000000131_rewrite_hub_ids_phibya_to_ziee_ai.sql"
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

// =====================================================================
// 4. Org-migration publisher rebrand (migration 131): phibya → ziee-ai
// =====================================================================

#[tokio::test]
async fn hub_ids_phibya_rewrite_migrates_and_is_idempotent() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // A first-party entity installed under the old personal namespace.
    let model_id = insert_hub_entity(
        &pool,
        "llm_model",
        "model",
        "io.github.phibya/deepseek-r1-70b",
    )
    .await;
    let assistant_id = insert_hub_entity(
        &pool,
        "assistant",
        "assistant",
        "io.github.phibya/code-reviewer",
    )
    .await;
    // A non-phibya reverse-DNS row + a row already on the new namespace:
    // both must be left untouched by the prefix-scoped rewrite.
    let untouched_mcp = insert_hub_entity(
        &pool,
        "mcp_server",
        "mcp_server",
        "io.github.modelcontextprotocol/filesystem",
    )
    .await;
    let already_new = insert_hub_entity(
        &pool,
        "assistant",
        "assistant",
        "io.github.ziee-ai/creative-writer",
    )
    .await;

    // First run rewrites the phibya rows.
    sqlx::raw_sql(MIGRATION_131_SQL)
        .execute(&pool)
        .await
        .expect("re-run migration 131");

    assert_eq!(
        fetch_hub_id(&pool, model_id).await,
        "io.github.ziee-ai/deepseek-r1-70b",
        "phibya model should be rebranded to ziee-ai"
    );
    assert_eq!(
        fetch_hub_id(&pool, assistant_id).await,
        "io.github.ziee-ai/code-reviewer",
        "phibya assistant should be rebranded to ziee-ai"
    );
    assert_eq!(
        fetch_hub_id(&pool, untouched_mcp).await,
        "io.github.modelcontextprotocol/filesystem",
        "non-phibya reverse-DNS hub_id must never change"
    );
    assert_eq!(
        fetch_hub_id(&pool, already_new).await,
        "io.github.ziee-ai/creative-writer",
        "already-ziee-ai hub_id must never change"
    );

    // Second run — prefix guard makes it a no-op (idempotent).
    sqlx::raw_sql(MIGRATION_131_SQL)
        .execute(&pool)
        .await
        .expect("second re-run of migration 131");
    assert_eq!(
        fetch_hub_id(&pool, model_id).await,
        "io.github.ziee-ai/deepseek-r1-70b",
        "rerunning migration 131 changed a rewritten row — not idempotent"
    );

    pool.close().await;
}

#[tokio::test]
async fn hub_id_slug_composes_through_92_then_131() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // A pre-§12 slug install. The real chain applies 92 (slug → personal
    // reverse-DNS) then 131 (personal → org namespace); assert both steps.
    let assistant_id = insert_hub_entity(
        &pool,
        "assistant",
        "assistant",
        "code-reviewer",
    )
    .await;

    sqlx::raw_sql(MIGRATION_SQL)
        .execute(&pool)
        .await
        .expect("re-run migration 92");
    assert_eq!(
        fetch_hub_id(&pool, assistant_id).await,
        "io.github.phibya/code-reviewer",
        "migration 92 maps the slug to the personal reverse-DNS namespace"
    );

    sqlx::raw_sql(MIGRATION_131_SQL)
        .execute(&pool)
        .await
        .expect("re-run migration 131");
    assert_eq!(
        fetch_hub_id(&pool, assistant_id).await,
        "io.github.ziee-ai/code-reviewer",
        "migration 131 rebrands the personal namespace to the org namespace"
    );

    pool.close().await;
}
