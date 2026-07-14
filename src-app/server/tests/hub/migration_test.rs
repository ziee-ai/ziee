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

// Migrations 92 + 131 were one-time DATA rewrites of existing `hub_entities`
// rows; MIGRATE-squash (N8) folded their end-state into the squashed hub
// baseline and dropped the standalone files. Their rewrite SQL is inlined here
// (from history) so this test still exercises the rewrite/idempotency logic —
// same approach as `tests/code_sandbox/tier2_migrations.rs`. Migration 92
// legitimately still emits the `io.github.phibya/*` form; 131 rebrands it.
const MIGRATION_SQL: &str = r#"
UPDATE hub_entities AS h
SET hub_id = m.new_hub_id
FROM (VALUES
    ('filesystem-mcp',          'io.github.modelcontextprotocol/filesystem'),
    ('memory-mcp',              'io.github.modelcontextprotocol/memory'),
    ('postgres-mcp',            'io.github.modelcontextprotocol/postgres'),
    ('github-mcp',              'io.github.github/mcp'),
    ('brave-search-mcp',        'com.brave/search-mcp'),
    ('linear-mcp',              'app.linear/mcp'),
    ('llama-3-1-8b-instruct',           'io.github.phibya/llama-3-1-8b-instruct'),
    ('llama-3-2-3b-instruct-gguf',      'io.github.phibya/llama-3-2-3b-instruct-gguf'),
    ('qwen2.5-coder-7b-instruct',       'io.github.phibya/qwen2.5-coder-7b-instruct'),
    ('qwen2.5-vl-3b-instruct',          'io.github.phibya/qwen2.5-vl-3b-instruct'),
    ('phi-3-mini-4k-instruct',          'io.github.phibya/phi-3-mini-4k-instruct'),
    ('nomic-embed-text-v1-5-gguf',      'io.github.phibya/nomic-embed-text-v1-5-gguf'),
    ('deepseek-r1-70b',                 'io.github.phibya/deepseek-r1-70b'),
    ('code-reviewer',   'io.github.phibya/code-reviewer'),
    ('creative-writer', 'io.github.phibya/creative-writer'),
    ('deep-researcher', 'io.github.phibya/deep-researcher'),
    ('sql-helper',      'io.github.phibya/sql-helper'),
    ('vision-analyst',  'io.github.phibya/vision-analyst')
) AS m(old_slug, new_hub_id)
WHERE h.hub_id = m.old_slug
  AND h.hub_id NOT LIKE '%/%';

DO $$
DECLARE
    orphan_count int;
BEGIN
    SELECT COUNT(*) INTO orphan_count
    FROM hub_entities
    WHERE hub_id NOT LIKE '%/%';
    IF orphan_count > 0 THEN
        RAISE NOTICE 'hub_entities migration: % row(s) have unrecognized slug-style hub_id (left untouched; reinstall to re-track)', orphan_count;
    END IF;
END $$;
"#;

const MIGRATION_131_SQL: &str = r#"
DO $$
DECLARE
    rewritten_count int;
BEGIN
    UPDATE hub_entities
    SET hub_id = 'io.github.ziee-ai/' || substring(hub_id from length('io.github.phibya/') + 1)
    WHERE hub_id LIKE 'io.github.phibya/%';

    GET DIAGNOSTICS rewritten_count = ROW_COUNT;
    IF rewritten_count > 0 THEN
        RAISE NOTICE 'hub_entities org migration: rewrote % row(s) io.github.phibya/* -> io.github.ziee-ai/*', rewritten_count;
    END IF;
END $$;
"#;

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
