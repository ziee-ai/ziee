//! Desktop auth bootstrap + auto-login tests
//!
//! These exercise `ensure_desktop_admin` and the JWT-minting half of
//! `auto_login` (`mint_admin_login`) against a real Postgres. They run by
//! default (no `#[ignore]`): they need a Postgres server at port 54321 —
//! the same dockerized build database the rest of the default-run desktop
//! integration suite (`TestServer`) already requires (`docker compose up
//! -d` per CLAUDE.md). Rather than mutate that shared `postgres` database
//! (the old version `TRUNCATE`d its `users` table, which is why these were
//! `#[ignore]`-gated), `shared_pool()` provisions its OWN isolated database
//! on that server and runs the server+desktop migrations into it in-process,
//! so it never clobbers the build DB.
//!
//! Still `#[serial]` + a single shared pool: the four tests share one
//! `init_repositories` OnceCell + isolated DB and `clean_users` between
//! tests, so they must not interleave.

use serial_test::serial;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Once;
use std::sync::OnceLock;
use tokio::sync::OnceCell;
use ziee_desktop::modules::auth::commands::mint_admin_login;
use ziee_desktop::modules::auth::ensure_desktop_admin;

/// Admin connection URL (points at the server's default `postgres`
/// database on :54321). We only use it to CREATE/DROP our own isolated
/// test database — never to run the tests against directly.
fn admin_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string())
}

/// Server-then-desktop migration dirs, resolved relative to this crate's
/// manifest (`desktop/tauri`). Mirrors the real desktop boot order +
/// `harness_inner.rs::template_migration_dirs`.
fn migration_dirs() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    vec![
        manifest.join("../../server/migrations"),
        manifest.join("migrations"),
    ]
}

static REPOS_INIT: Once = Once::new();
static POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn shared_pool() -> &'static PgPool {
    POOL.get_or_init(|| async {
        let admin = admin_url();
        // Our own isolated database on the same server. Fixed name +
        // DROP-IF-EXISTS (FORCE) at setup so a leftover from a crashed
        // prior run is reclaimed rather than leaking forever.
        let db_name = "ziee_desktop_auth_tests";
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&admin)
            .await
            .expect(
                "Failed to connect to Postgres at 54321. Run \
                 `cd src-app && docker compose up -d` to start it.",
            );
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
            .execute(&admin_pool)
            .await;
        sqlx::query(&format!("CREATE DATABASE {db_name}"))
            .execute(&admin_pool)
            .await
            .expect("create isolated auth-tests database");
        admin_pool.close().await;

        // Swap the database segment of the admin URL for our isolated DB
        // (string-only, to avoid pulling in the `url` crate here).
        let base = admin
            .rsplit_once('/')
            .map(|(b, _)| b)
            .expect("admin URL has a /database segment");
        let db_url = format!("{base}/{db_name}");

        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&db_url)
            .await
            .expect("connect isolated auth-tests database");

        // Apply server-then-desktop migrations in-process. ignore_missing
        // lets each migrator run against a DB that already carries the
        // other set (desktop versions sit far above the server's).
        for dir in migration_dirs() {
            let mut migrator = sqlx::migrate::Migrator::new(dir.clone())
                .await
                .unwrap_or_else(|e| panic!("create migrator for {}: {e}", dir.display()));
            migrator.set_ignore_missing(true);
            migrator
                .run(&pool)
                .await
                .unwrap_or_else(|e| panic!("migrate auth-tests DB from {}: {e}", dir.display()));
        }

        REPOS_INIT.call_once(|| {
            ziee::init_repositories(pool.clone());
        });
        pool
    })
    .await
}

/// Wipe the `users` table so each test starts from a known empty state.
/// CASCADE picks up the FKs from `refresh_tokens`, `user_groups`, etc.
async fn clean_users(pool: &PgPool) {
    sqlx::query("TRUNCATE TABLE users RESTART IDENTITY CASCADE")
        .execute(pool)
        .await
        .expect("truncate users");
}

fn test_jwt_service() -> Arc<ziee::JwtService> {
    // Long-enough + unique secret to pass JwtService::try_new's
    // banned-placeholder guard (modules/auth/jwt.rs::BANNED_JWT_SECRETS).
    let config = ziee::JwtConfig {
        secret: "desktop_auth_tests_secret_must_be_at_least_32_bytes_long".to_string(),
        issuer: "ziee".to_string(),
        audience: "ziee-api".to_string(),
        access_token_expiry_hours: 24,
        refresh_token_expiry_days: 30,
    };
    Arc::new(ziee::JwtService::new(config))
}

/// A single multi-thread runtime shared by ALL tests in this file.
/// `#[tokio::test]` builds a fresh runtime per test, but the shared sqlx
/// `POOL` (and the global `ziee::Repos` pool init'd from it) is created
/// lazily on whichever runtime runs first — and a sqlx pool's background
/// connection management is bound to the runtime that created it. Reusing
/// that pool from a *different* per-test runtime makes `acquire` hang
/// (`PoolTimedOut`). Running every test on ONE shared runtime keeps the
/// pool and Repos on the reactor that owns them. (`#[serial]` still applies
/// because the tests share that pool + the truncated `users` table.)
fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build shared test runtime")
    })
}

#[test]
#[serial]
fn ensure_desktop_admin_creates_admin_on_first_run() {
    rt().block_on(async {
        let pool = shared_pool().await;
        clean_users(pool).await;

        ensure_desktop_admin()
            .await
            .expect("ensure_desktop_admin should succeed on empty DB");

        let admin = ziee::Repos
            .user
            .get_by_username("admin")
            .await
            .expect("get_by_username should not error")
            .expect("admin row should exist after bootstrap");

        assert!(admin.is_admin, "bootstrapped admin should have is_admin=true");
        assert_eq!(admin.email, "admin@localhost");
    });
}

#[test]
#[serial]
fn ensure_desktop_admin_is_idempotent() {
    rt().block_on(async {
        let pool = shared_pool().await;
        clean_users(pool).await;

        ensure_desktop_admin().await.expect("first call should succeed");
        ensure_desktop_admin()
            .await
            .expect("second call should be a no-op (no error, no duplicate)");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = true")
            .fetch_one(pool)
            .await
            .expect("count admins");
        assert_eq!(count, 1, "exactly one admin should exist after two calls");
    });
}

#[test]
#[serial]
fn mint_admin_login_returns_valid_jwt_for_bootstrapped_admin() {
    rt().block_on(async {
        let pool = shared_pool().await;
        clean_users(pool).await;
        ensure_desktop_admin().await.expect("bootstrap admin");

        let jwt = test_jwt_service();
        let response = mint_admin_login(&jwt)
            .await
            .expect("mint_admin_login should succeed when admin exists");

        assert_eq!(response.user.username, "admin");
        assert!(response.user.is_admin);
        assert!(
            !response.access_token.is_empty(),
            "access_token must be populated"
        );
        assert!(
            !response.refresh_token.is_empty(),
            "refresh_token must be populated"
        );
        assert!(response.expires_in > 0, "expires_in must be positive");
    });
}

#[test]
#[serial]
fn mint_admin_login_errors_when_admin_missing() {
    rt().block_on(async {
        let pool = shared_pool().await;
        clean_users(pool).await;

        let jwt = test_jwt_service();
        let err = mint_admin_login(&jwt)
            .await
            .expect_err("should error when no admin exists");

        // The UI's retry loop matches on this prefix; keep the contract
        // stable so the spinner caption stays meaningful.
        assert!(
            err.contains("Admin not found"),
            "error string must contain 'Admin not found', got: {err}"
        );
    });
}
