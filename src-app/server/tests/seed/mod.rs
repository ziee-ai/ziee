//! Integration tests for the ziee-on-SDK declarative seed (the `ziee-seed` engine
//! wired into ziee's boot). Each test spawns a real server (which runs the seed at
//! boot in its own process) and then connects a pool to that server's DB to assert
//! the seed's effects.
//!
//! Coverage:
//! - (a) after boot, the `seed_ledger` OWNS the migration-baked `llm_providers`
//!   rows (adopt-in-place).
//! - (b) the overlay-only `SeedDemo` provider (absent from every migration) was
//!   CREATEd, ledgered, and its nested group-assign converged.
//! - (c) re-running the full seed on the same DB is idempotent (no duplicates).
//! - (d) reconciling a settings-singleton (web_search_settings) updates its columns.

use sqlx::postgres::PgPoolOptions;

use crate::common::TestServer;

/// The 8 built-in providers baked (built_in = true) by the llm_provider migration.
const BAKED_PROVIDERS: &[&str] = &[
    "OpenAI",
    "Anthropic",
    "Groq",
    "Google Gemini",
    "Mistral AI",
    "DeepSeek",
    "Local",
    "OpenRouter",
];

async fn connect(server: &TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .expect("connect to the spawned server's DB")
}

// (a) The seed ADOPTED the baked built_in llm_providers into the ledger at boot.
#[tokio::test]
async fn seed_ledger_owns_adopted_baked_llm_providers() {
    let server = TestServer::start().await;
    let pool = connect(&server).await;

    for name in BAKED_PROVIDERS {
        let row = sqlx::query!(
            "SELECT p.id AS prov_id, p.built_in \
             FROM seed_ledger l JOIN llm_providers p ON p.id = l.entity_id \
             WHERE l.section = 'llm_providers' AND l.natural_key = $1",
            name
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        let row = row.unwrap_or_else(|| panic!("baked provider {name} was not adopted into seed_ledger"));
        assert!(row.built_in, "adopted provider {name} should be a built_in baked row");
    }

    let n: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM seed_ledger WHERE section = 'llm_providers'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
    // 8 baked default providers are adopted at boot (the shipped default is adopt-only).
    assert!(n >= 8, "expected >= 8 ledgered llm_providers, got {n}");
}

// (b) An overlay-only provider (absent from the baselines AND the shipped default) is
// CREATEd + ledgered + group-assigned — the operator-overlay CREATE path. Applied via
// run_from_yaml so the shipped default.yaml stays adopt-only (no cross-test blast radius).
#[tokio::test]
async fn seed_creates_and_ledgers_the_overlay_only_demo_provider() {
    let server = TestServer::start().await;
    let pool = connect(&server).await;

    // Multi-row providers write through the global `Repos`; point it at THIS test's DB
    // before the overlay run (the codebase norm for Repos-using integration tests).
    ziee::init_repositories(pool.clone());
    let overlay = "llm_providers:\n  items:\n    - name: SeedDemo\n      provider_type: openai\n      base_url: \"https://demo.seed.ziee.invalid/v1\"\n      enabled: false\n      assign_groups: [Users]\n";
    ziee::ziee_seed::run_from_yaml(&pool, Some(overlay), false, "")
        .await
        .expect("overlay seed apply succeeds");

    let row = sqlx::query!("SELECT id, built_in FROM llm_providers WHERE name = 'SeedDemo'")
        .fetch_optional(&pool)
        .await
        .unwrap()
        .expect("SeedDemo provider was CREATEd from the overlay");
    assert!(!row.built_in, "a seed-created provider is not a built_in baked row");

    let led = sqlx::query!(
        "SELECT entity_id FROM seed_ledger WHERE section = 'llm_providers' AND natural_key = 'SeedDemo'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap()
    .expect("SeedDemo is ledgered");
    assert_eq!(led.entity_id, Some(row.id), "ledger entity_id points at the created row");

    // Nested subtree: the SeedDemo → Users group-assign converged AND is ledgered.
    let assigned: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM user_group_llm_providers ugp \
         JOIN groups g ON g.id = ugp.group_id \
         WHERE ugp.provider_id = $1 AND g.name = 'Users'",
        row.id
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(assigned, 1, "SeedDemo is assigned to the Users group");

    let led_assign: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM seed_ledger WHERE section = 'user_group_llm_providers' AND natural_key = 'SeedDemo:Users'"
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(led_assign, 1, "the SeedDemo:Users assignment is ledgered");
}

// (c) Re-running the full seed on the same DB creates no duplicates.
#[tokio::test]
async fn seed_rerun_is_idempotent() {
    let server = TestServer::start().await;
    let pool = connect(&server).await;

    let prov_before: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM llm_providers")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);
    let led_before: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM seed_ledger")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);
    let assign_before: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM user_group_llm_providers")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);

    // The multi-row providers write through the global `Repos`; point it at THIS
    // test's DB immediately before the run (the codebase norm for Repos-using
    // integration tests). The count assertions read the local pool, so they stay
    // correct even under the small cross-test Repos window.
    ziee::init_repositories(pool.clone());
    ziee::run_seed(&pool).await.expect("seed re-run succeeds");

    let prov_after: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM llm_providers")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);
    let led_after: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM seed_ledger")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);
    let assign_after: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM user_group_llm_providers")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);

    assert_eq!(prov_before, prov_after, "no duplicate llm_providers on re-run");
    assert_eq!(led_before, led_after, "no duplicate seed_ledger rows on re-run");
    assert_eq!(assign_before, assign_after, "no duplicate group assignments on re-run");
}

// (d) A per-section reconcile of a settings singleton updates its columns.
#[tokio::test]
async fn settings_singleton_reconcile_updates_columns() {
    let server = TestServer::start().await;
    let pool = connect(&server).await;

    // Migration default: enabled = true, max_results = 5. Boot's seed-if-empty
    // adopts the row but never changes it.
    let before = sqlx::query!("SELECT enabled, max_results FROM web_search_settings WHERE id = TRUE")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(before.enabled, "baseline web_search_settings.enabled is the migration default (true)");

    // Reconcile ONLY web_search_settings (per-section mode) against an empty base,
    // so no multi-row provider (global Repos) is engaged — the GenericSingleton
    // writes through ctx.pool, making this fully race-free.
    let overlay = "web_search_settings:\n  mode: reconcile\n  items:\n    - { enabled: false, max_results: 7 }\n";
    ziee::ziee_seed::run_from_yaml(&pool, Some(overlay), false, "{}")
        .await
        .expect("singleton reconcile succeeds");

    let after = sqlx::query!("SELECT enabled, max_results FROM web_search_settings WHERE id = TRUE")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!after.enabled, "reconcile set enabled = false");
    assert_eq!(after.max_results, 7, "reconcile set max_results = 7");

    let led: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM seed_ledger WHERE section = 'web_search_settings' AND natural_key = 'web_search_settings'"
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(led, 1, "the singleton is ledger-owned");
}
