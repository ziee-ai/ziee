//! Desktop auth bootstrap + auto-login tests
//!
//! These exercise `ensure_desktop_admin` and the JWT-minting half of
//! `auto_login` (`mint_admin_login`) against a real Postgres. They are
//! `#[ignore]`-gated because they need the dockerized build database
//! that the server's integration tests share (port 54321 by default
//! per CLAUDE.md). Bring it up with:
//!
//!   cd src-app && docker compose up -d
//!
//! Then run with:
//!
//!   cd src-app/desktop/tauri && cargo test -- --ignored --test-threads=1
//!
//! `--test-threads=1` is required because all tests share a single
//! `init_repositories` OnceCell and truncate the `users` table between
//! tests.
//!
//! Skipped in the default `cargo test` run so dev cycles stay fast and
//! DB-free.

use serial_test::serial;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use std::sync::Once;
use tokio::sync::OnceCell;
use ziee_desktop::modules::auth::commands::mint_admin_login;
use ziee_desktop::modules::auth::ensure_desktop_admin;

fn database_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string())
}

static REPOS_INIT: Once = Once::new();
static POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn shared_pool() -> &'static PgPool {
    POOL.get_or_init(|| async {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url())
            .await
            .expect(
                "Failed to connect to test Postgres at 54321. Run \
                 `cd src-app && docker compose up -d` to start it.",
            );
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

#[tokio::test]
#[ignore]
#[serial]
async fn ensure_desktop_admin_creates_admin_on_first_run() {
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
}

#[tokio::test]
#[ignore]
#[serial]
async fn ensure_desktop_admin_is_idempotent() {
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
}

#[tokio::test]
#[ignore]
#[serial]
async fn mint_admin_login_returns_valid_jwt_for_bootstrapped_admin() {
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
}

#[tokio::test]
#[ignore]
#[serial]
async fn mint_admin_login_errors_when_admin_missing() {
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
}
