//! ziee's database bootstrap — a thin orchestration shim over the generic
//! embedded/external Postgres bring-up in `ziee_framework::embedded_pg`
//! (relocated there in Chunk BG-3).
//!
//! The framework owns the app-agnostic lifecycle (embedded-PG setup/start/stop,
//! pool connect + retry, the process-wide pool + instance statics, cleanup on
//! panic/drop). ziee retains ONLY the two schema-/app-bound pieces the framework
//! is parameterized over:
//!   - `MERGED_MIGRATOR` — the app's merged (`migrations-merged`) schema-bound
//!     `sqlx::migrate!` set, with `set_ignore_missing(true)` (desktop + server
//!     share `_sqlx_migrations`).
//!   - the pgvector install + `CREATE EXTENSION` smoke-test hooks (the memory
//!     module's Postgres extension), which stay app-side because the artifacts
//!     are baked in by ziee's `build.rs` (`pgvector_install`).
//!
//! The public API (`initialize_database` / `get_database_pool` /
//! `cleanup_database`) is byte-signature-identical to before, so every consumer
//! (`main.rs`, `lib.rs`, `file::geometry_backfill`) is unchanged.

use sqlx::PgPool;
use std::sync::Arc;
use ziee_framework::embedded_pg::{self, EmbeddedPgHooks};

pub mod pgvector_install;

/// The Postgres binary version (theseus-rs), from `.cargo/config.toml`'s
/// `[env]`. Passed to the framework's `pg_ctl` stop path — the SDK workspace
/// has no such env, so the version can't be `env!`'d framework-side.
const POSTGRES_VERSION: &str = env!("ZIEE_POSTGRES_VERSION");

/// The app's merged migration set (`migrations-merged` = the UNION of every
/// module-owned `src/modules/*/migrations/` ∪ SDK `sdk/crates/*/migrations/`,
/// composed by build.rs, version-sorted — see MIGRATE-squash / N3.1 / N7).
/// `set_ignore_missing(true)` because desktop +
/// server share `_sqlx_migrations` and each binary owns its own subset — the
/// supported sqlx pattern. A `LazyLock` so the framework receives a `&'static`.
static MERGED_MIGRATOR: std::sync::LazyLock<sqlx::migrate::Migrator> =
    std::sync::LazyLock::new(|| {
        let mut m = sqlx::migrate!("./migrations-merged");
        m.set_ignore_missing(true);
        m
    });

/// The app's Postgres-extension install hooks, threaded into the generic
/// embedded bring-up. Fail-soft: pgvector-less installs (zero-byte build stubs)
/// leave the DB usable; the memory module self-disables via
/// `pgvector_install::is_available()`.
const PGVECTOR_HOOKS: EmbeddedPgHooks = EmbeddedPgHooks {
    after_setup: pgvector_after_setup,
    smoke_test: pgvector_smoke,
};

/// Install pgvector into the embedded-PG installation dir BEFORE start() —
/// Postgres only scans `share/extension/` at boot for CREATE EXTENSION lookups.
/// Fail-soft: if the build embedded zero-byte stubs (pgvector make failed at
/// compile time), log and continue; the memory module checks
/// `pgvector_install::is_available()` before touching vector(N) tables.
fn pgvector_after_setup(install_dir: &std::path::Path) {
    if pgvector_install::has_real_artifacts() {
        match pgvector_install::install_into(install_dir) {
            Ok(()) => println!("pgvector: installed into embedded PG"),
            Err(e) => eprintln!(
                "WARN: pgvector install failed; memory features will be disabled: {}",
                e
            ),
        }
    } else {
        eprintln!(
            "WARN: pgvector artifacts not built into binary (build_helper/pgvector.rs::build_pgvector failed at compile time); memory features will be disabled"
        );
    }
}

/// Smoke-test: CREATE EXTENSION vector. On success, mark available so the memory
/// module knows it can use vector(N).
fn pgvector_smoke(
    smoke_url: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        if let Ok(probe_pool) = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&smoke_url)
            .await
        {
            match sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
                .execute(&probe_pool)
                .await
            {
                Ok(_) => {
                    pgvector_install::mark_available();
                    println!("pgvector: CREATE EXTENSION smoke-test passed");
                }
                Err(e) => {
                    eprintln!(
                        "WARN: CREATE EXTENSION vector failed; memory features will be disabled: {}",
                        e
                    );
                }
            }
            probe_pool.close().await;
        }
    })
}

/// Bring up the database (embedded or external), run migrations, and cache the
/// process-wide pool. Delegates to `ziee_framework::embedded_pg`, passing ziee's
/// merged migrator + pgvector hooks + Postgres binary version.
pub async fn initialize_database(
    config: &crate::core::config::Config,
) -> Result<Arc<PgPool>, Box<dyn std::error::Error + Send + Sync>> {
    embedded_pg::initialize_database(
        config.postgresql.clone(),
        config.database_url(),
        POSTGRES_VERSION.to_string(),
        std::sync::LazyLock::force(&MERGED_MIGRATOR),
        PGVECTOR_HOOKS,
    )
    .await
}

/// The cached process-wide pool. Errors with `PoolTimedOut` before
/// `initialize_database` has run.
pub fn get_database_pool() -> Result<Arc<PgPool>, sqlx::Error> {
    embedded_pg::get_database_pool()
}

/// Close the pool + stop the embedded Postgres instance. Idempotent.
pub async fn cleanup_database() {
    embedded_pg::cleanup_database().await
}
