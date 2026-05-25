//! Admin CRUD for /api/admin/auth-providers — covers:
//!   - Permission gating (member → 403, admin → 200)
//!   - Secret masking on GET
//!   - Secret preservation on PUT with empty client_secret
//!   - Delete returns cascade count
//!   - Public /api/auth/providers excludes secrets + disabled rows
//!
//! All endpoints use `RequirePermissions<…>` so the gating boundary
//! is the typed extractor, not anything bespoke per handler.

use serde_json::json;

/// Seed an admin user via setup, return its access token.
async fn make_admin(test_server: &crate::common::TestServer) -> String {
    let client = reqwest::Client::new();
    let r = client
        .post(test_server.api_url("/app/setup/admin"))
        .json(&json!({
            "username": "rootadmin",
            "email": "root@example.com",
            "password": "ComplexPass1!"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let body: serde_json::Value = r.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

/// Register a regular non-admin user, return its access token.
async fn make_member(test_server: &crate::common::TestServer, username: &str) -> String {
    let client = reqwest::Client::new();
    let r = client
        .post(test_server.api_url("/auth/register"))
        .json(&json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": "ComplexPass1!"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let body: serde_json::Value = r.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

/// Member (no auth_providers permission) must get 403 on every
/// admin auth-providers endpoint.
#[tokio::test]
async fn test_admin_providers_member_blocked_403() {
    let test_server = crate::common::TestServer::start().await;
    let _admin_token = make_admin(&test_server).await; // setup must run first
    let member_token = make_member(&test_server, "alice").await;
    let client = reqwest::Client::new();

    let bearer = format!("Bearer {}", member_token);

    let r = client
        .get(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "GET /admin/auth-providers must 403 for member");

    let r = client
        .post(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .json(&json!({
            "name": "x",
            "provider_type": "oidc",
            "config": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "POST must 403 for member");

    // Use a placeholder id for PUT/DELETE/test — perm check happens
    // before any row lookup, so the bogus id is fine for asserting 403.
    let bogus_id = "00000000-0000-0000-0000-000000000000";
    let r = client
        .put(test_server.api_url(&format!("/admin/auth-providers/{}", bogus_id)))
        .header("Authorization", &bearer)
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);

    let r = client
        .delete(test_server.api_url(&format!("/admin/auth-providers/{}", bogus_id)))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);

    let r = client
        .post(test_server.api_url(&format!(
            "/admin/auth-providers/{}/test",
            bogus_id
        )))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}

/// Root admin can CRUD providers. Asserts:
///   - create returns 201 with body matching what we sent (secret masked)
///   - GET returns the row with secret masked, NOT the real value
///   - PUT with empty client_secret preserves the existing secret
///   - DELETE returns deleted=true + cascade count
#[tokio::test]
async fn test_admin_providers_crud_happy_path() {
    let test_server = crate::common::TestServer::start().await;
    let admin_token = make_admin(&test_server).await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin_token);

    // 1. Create.
    let create_body = json!({
        "name": "google",
        "provider_type": "oidc",
        "enabled": true,
        "config": {
            "client_id": "google-client-id",
            "client_secret": "INITIAL-SECRET-VALUE",
            "issuer_url": "https://accounts.google.com",
            "scopes": ["openid","email","profile"]
        }
    });
    let r = client
        .post(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .json(&create_body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let created: serde_json::Value = r.json().await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["config"]["client_secret"], json!("••••••"));

    // 2. List — secret still masked.
    let r = client
        .get(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let list: Vec<serde_json::Value> = r.json().await.unwrap();
    let row = list.iter().find(|p| p["name"] == "google").unwrap();
    assert_eq!(row["config"]["client_secret"], json!("••••••"));
    assert_ne!(row["config"]["client_secret"], json!("INITIAL-SECRET-VALUE"));

    // 3. PUT with empty client_secret preserves the existing one.
    //    Confirm by reading the raw DB value before + after.
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let secret_before: Option<String> = sqlx::query_scalar!(
        r#"SELECT config->>'client_secret' FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(secret_before.as_deref(), Some("INITIAL-SECRET-VALUE"));

    let r = client
        .put(test_server.api_url(&format!("/admin/auth-providers/{}", id)))
        .header("Authorization", &bearer)
        .json(&json!({
            "config": {
                "client_id": "google-client-id",
                "client_secret": "",                 // empty → preserve
                "issuer_url": "https://accounts.google.com",
                "scopes": ["openid","email","profile","extra"]
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let secret_after: Option<String> = sqlx::query_scalar!(
        r#"SELECT config->>'client_secret' FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        secret_after.as_deref(),
        Some("INITIAL-SECRET-VALUE"),
        "Empty client_secret in PUT must preserve existing value"
    );
    let scopes_after: Option<Vec<String>> = sqlx::query_scalar!(
        r#"SELECT ARRAY(SELECT jsonb_array_elements_text(config->'scopes'))::text[]
           FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        scopes_after.unwrap().contains(&"extra".to_string()),
        "Non-secret config edits should still apply"
    );

    // 4. PUT with NEW client_secret replaces the existing value.
    let r = client
        .put(test_server.api_url(&format!("/admin/auth-providers/{}", id)))
        .header("Authorization", &bearer)
        .json(&json!({
            "config": {
                "client_id": "google-client-id",
                "client_secret": "REPLACED-SECRET",
                "issuer_url": "https://accounts.google.com",
                "scopes": ["openid","email"]
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let secret_after2: Option<String> = sqlx::query_scalar!(
        r#"SELECT config->>'client_secret' FROM auth_providers WHERE name = 'google'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(secret_after2.as_deref(), Some("REPLACED-SECRET"));

    // 5. Delete + cascade count.
    let r = client
        .delete(test_server.api_url(&format!("/admin/auth-providers/{}", id)))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["deleted"], json!(true));
    assert_eq!(body["affected_user_links"], json!(0));
}

/// `GET /api/auth/providers` (PUBLIC, no auth) returns the enabled
/// providers with display fields only — never secrets.
#[tokio::test]
async fn test_public_providers_list_no_secrets() {
    let test_server = crate::common::TestServer::start().await;
    let admin_token = make_admin(&test_server).await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin_token);

    // Seed two providers: one enabled, one disabled.
    let _r1 = client
        .post(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .json(&json!({
            "name": "google",
            "provider_type": "oidc",
            "enabled": true,
            "config": {
                "client_id": "id",
                "client_secret": "should-not-appear",
                "issuer_url": "https://accounts.google.com",
                "scopes": ["openid"],
                "display_name": "Sign in with Google"
            }
        }))
        .send()
        .await
        .unwrap();

    let _r2 = client
        .post(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .json(&json!({
            "name": "disabled-okta",
            "provider_type": "oidc",
            "enabled": false,
            "config": {
                "client_id": "id",
                "client_secret": "should-not-appear",
                "issuer_url": "https://okta.example.com"
            }
        }))
        .send()
        .await
        .unwrap();

    // Public endpoint — no auth.
    let pub_client = reqwest::Client::new();
    let r = pub_client
        .get(test_server.api_url("/auth/providers"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();

    // Only the enabled non-local provider shows up.
    let names: Vec<&str> = providers
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"google"), "Enabled provider must appear");
    assert!(
        !names.contains(&"disabled-okta"),
        "Disabled provider must NOT appear in public list"
    );

    // Sanity: serialized JSON must not include the secret value anywhere.
    let raw = serde_json::to_string(&body).unwrap();
    assert!(
        !raw.contains("should-not-appear"),
        "Secret must never appear in public providers response: {}",
        raw
    );

    // Display field present.
    let g = providers.iter().find(|p| p["name"] == "google").unwrap();
    assert_eq!(g["display_name"], json!("Sign in with Google"));
}

/// PUT against a non-existent provider id should 404 (not 500).
#[tokio::test]
async fn test_admin_providers_update_missing_returns_404() {
    let test_server = crate::common::TestServer::start().await;
    let admin_token = make_admin(&test_server).await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin_token);

    let bogus_id = "00000000-0000-0000-0000-000000000000";
    let r = client
        .put(test_server.api_url(&format!("/admin/auth-providers/{}", bogus_id)))
        .header("Authorization", &bearer)
        .json(&json!({ "config": { "client_id": "x" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

/// DELETE against a non-existent provider id should 404.
#[tokio::test]
async fn test_admin_providers_delete_missing_returns_404() {
    let test_server = crate::common::TestServer::start().await;
    let admin_token = make_admin(&test_server).await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin_token);

    let bogus_id = "00000000-0000-0000-0000-000000000000";
    let r = client
        .delete(test_server.api_url(&format!("/admin/auth-providers/{}", bogus_id)))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

/// CREATE rejects an invalid provider_type.
#[tokio::test]
async fn test_admin_providers_create_rejects_bad_type() {
    let test_server = crate::common::TestServer::start().await;
    let admin_token = make_admin(&test_server).await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin_token);

    let r = client
        .post(test_server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .json(&json!({
            "name": "garbage",
            "provider_type": "facebook-2007",
            "config": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error_code"], json!("INVALID_PROVIDER_TYPE"));
}
