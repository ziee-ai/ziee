//! Tier 2 — session settings (`/api/auth/session-settings`): CRUD,
//! permission gating, range validation, sync emission, config seed-once,
//! and the mint-time DB read (fresh logins honor the admin-configured
//! lifetimes).

use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

fn admin_perms() -> &'static [&'static str] {
    &[
        "auth::session_settings::read",
        "auth::session_settings::manage",
    ]
}

/// The JwtService configured EXACTLY like the test harness's server
/// (same secret/issuer/audience — see harness_inner.rs's jwt block), so
/// tests can validate + decode tokens the server minted.
fn harness_jwt_service() -> ziee::JwtService {
    ziee::JwtService::try_new(ziee::JwtConfig {
        secret: "test-secret-key-for-jwt-tokens-min-32-chars-long".to_string(),
        issuer: "ziee".to_string(),
        audience: "ziee-api".to_string(),
        access_token_expiry_hours: 24,
        refresh_token_expiry_days: 30,
        access_token_expiry_seconds: None,
    })
    .expect("harness jwt config is valid")
}

#[tokio::test]
async fn test_get_put_roundtrip() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ss_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    // GET returns the seeded singleton (config defaults 24h / 30d).
    let res = client
        .get(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["access_token_expiry_hours"], 24);
    assert_eq!(row["refresh_token_expiry_days"], 30);

    // PUT both fields.
    let res = client
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "access_token_expiry_hours": 2, "refresh_token_expiry_days": 7 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["access_token_expiry_hours"], 2);
    assert_eq!(row["refresh_token_expiry_days"], 7);

    // Partial PUT leaves the other field untouched (COALESCE).
    let res = client
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "refresh_token_expiry_days": 14 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["access_token_expiry_hours"], 2);
    assert_eq!(row["refresh_token_expiry_days"], 14);

    // GET reflects the persisted values.
    let res = client
        .get(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["access_token_expiry_hours"], 2);
    assert_eq!(row["refresh_token_expiry_days"], 14);
}

/// Both gates per endpoint: no token → 401; a user WITHOUT the admin
/// perms → 403 (guidelines §12).
#[tokio::test]
async fn test_non_admin_403_unauth_401() {
    let server = TestServer::start().await;
    let plain = create_user_with_permissions(&server, "ss_plain", &[]).await;
    let client = reqwest::Client::new();

    // Unauthenticated → 401.
    let res = client
        .get(server.api_url("/auth/session-settings"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    let res = client
        .put(server.api_url("/auth/session-settings"))
        .json(&json!({ "access_token_expiry_hours": 3 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);

    // Authenticated but unprivileged → 403.
    let res = client
        .get(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", plain.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
    let res = client
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", plain.token))
        .json(&json!({ "access_token_expiry_hours": 3 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // Read-only admin can GET but not PUT.
    let reader = create_user_with_permissions(
        &server,
        "ss_reader",
        &["auth::session_settings::read"],
    )
    .await;
    let res = client
        .get(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let res = client
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .json(&json!({ "access_token_expiry_hours": 3 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_validation_400_out_of_range() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ss_range", admin_perms()).await;
    let client = reqwest::Client::new();

    for body in [
        json!({ "access_token_expiry_hours": 0 }),
        json!({ "access_token_expiry_hours": 9000 }),
        json!({ "refresh_token_expiry_days": 0 }),
        json!({ "refresh_token_expiry_days": 4000 }),
    ] {
        let res = client
            .put(server.api_url("/auth/session-settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "out-of-range body {body} must 400");
        let err: Value = res.json().await.unwrap();
        assert_eq!(err["error_code"], "VALIDATION_ERROR");
    }
}

/// The DB setting is honored AT MINT TIME: after an admin shortens the
/// lifetimes, a fresh login's tokens decode with the new exp values.
#[tokio::test]
async fn test_db_expiry_honored_in_minted_tokens() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ss_mint", admin_perms()).await;
    let client = reqwest::Client::new();

    let res = client
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "access_token_expiry_hours": 1, "refresh_token_expiry_days": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Fresh registration mints with the new lifetimes.
    let register: Value = client
        .post(server.api_url("/auth/register"))
        .json(&json!({
            "username": "ss_mint_user",
            "email": "ss_mint_user@example.com",
            "password": "testpass123"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(register["expires_in"], 3600, "expires_in reflects 1h");

    let svc = harness_jwt_service();
    let now = chrono::Utc::now().timestamp();

    let access = svc
        .validate_access_token(register["access_token"].as_str().unwrap())
        .unwrap();
    let access_ttl = access.exp - now;
    assert!(
        (3600 - 120..=3600 + 120).contains(&access_ttl),
        "access exp ≈ now+1h, got ttl {access_ttl}s"
    );

    let refresh = svc
        .validate_refresh_token(register["refresh_token"].as_str().unwrap())
        .unwrap();
    let refresh_ttl = refresh.exp - now;
    let one_day = 24 * 3600;
    assert!(
        (one_day - 120..=one_day + 120).contains(&refresh_ttl),
        "refresh exp ≈ now+1d, got ttl {refresh_ttl}s"
    );
}

/// A settings update publishes a `SessionSettings`/`update` sync frame to
/// holders of `auth::session_settings::read` and NOT to plain users
/// (audience = `Audience::perm::<SessionSettingsRead>()`, singleton id =
/// `Uuid::nil`).
#[tokio::test]
async fn test_sync_emit_on_update() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ss_sync_admin", admin_perms()).await;
    let plain = create_user_with_permissions(&server, "ss_sync_plain", &[]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut plain_probe = SyncProbe::open(&server, &plain.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "refresh_token_expiry_days": 21 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let frame = admin_probe
        .expect_event("session_settings", "update", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string());

    plain_probe.expect_silence(Duration::from_secs(1)).await;
}

/// The YAML jwt values are copied into the singleton exactly ONCE
/// (`seeded_from_config` guard): the first boot seeds them; a simulated
/// second boot (re-running the seed against the same DB) must NOT
/// overwrite an admin's subsequent edit.
#[tokio::test]
async fn test_config_seed_applied_once() {
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        refresh_token_expiry_days: Some(14),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ss_seed", admin_perms()).await;
    let client = reqwest::Client::new();

    // Boot seed carried the YAML value (14, not the migration default 30).
    let row: Value = client
        .get(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(row["refresh_token_expiry_days"], 14);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let seeded: bool =
        sqlx::query_scalar("SELECT seeded_from_config FROM session_settings WHERE id = TRUE")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(seeded, "boot must set the seeded_from_config flag");

    // Admin edits the value…
    let res = client
        .put(server.api_url("/auth/session-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "refresh_token_expiry_days": 21 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // …and a second boot's seed attempt (the exact repo call the module
    // init makes) is a no-op because the flag is already set.
    let repo = ziee::SessionSettingsRepository::new(pool.clone());
    repo.seed_from_config_once(14, 14).await.unwrap();

    let days: i32 =
        sqlx::query_scalar("SELECT refresh_token_expiry_days FROM session_settings WHERE id = TRUE")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(days, 21, "re-seed must not clobber the admin's edit");
    pool.close().await;
}
