//! Magic-link issue + exchange integration tests.

use serde_json::{Value, json};

#[tokio::test]
async fn magic_link_issue_requires_admin() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server, "ml_nonadmin",
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/issue"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn magic_link_exchange_invalid_token_returns_401() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/exchange"))
        .json(&json!({ "token": "this-token-was-never-issued" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    let body: Value = res.json().await.unwrap();
    // The shared AppError serializes as { error: "<msg>", error_code: "<code>", details }
    // — see common/type.rs::ApiError.
    let code = body
        .get("error_code")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    assert!(
        code.contains("MAGIC_LINK_INVALID"),
        "expected MAGIC_LINK_INVALID code, got '{}' (full body: {})",
        code,
        body
    );
}

#[tokio::test]
async fn magic_link_exchange_empty_token_returns_401() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/exchange"))
        .json(&json!({ "token": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn magic_link_issue_exchange_roundtrip_as_admin() {
    // Happy-path E2E: provision a user with username "admin", grant
    // remote_access::manage, issue a magic-link, exchange it, assert
    // we get back a JWT for that user.
    let server = crate::common::TestServer::start_desktop().await;

    // The default helper appends a UUID suffix to the username for
    // uniqueness, but `issue` hard-codes `get_by_username("admin")`.
    // Rename the row after creation so the lookup finds our user.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ml_happy",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let user_uuid = uuid::Uuid::parse_str(&admin.user_id).unwrap();
    sqlx::query("UPDATE users SET username = 'admin' WHERE id = $1")
        .bind(user_uuid)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    // Issue.
    let issue_res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/issue"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(issue_res.status(), 200);
    let issue_body: Value = issue_res.json().await.unwrap();
    let token = issue_body["token"].as_str().expect("token field");
    assert!(!token.is_empty(), "issued token should be non-empty");

    // Exchange (unauthenticated — that's the whole point).
    let xchg_res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/exchange"))
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();
    assert_eq!(xchg_res.status(), 200);
    let xchg_body: Value = xchg_res.json().await.unwrap();
    assert_eq!(xchg_body["user"]["username"], "admin");
    assert!(
        xchg_body["access_token"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "exchange should return an access_token"
    );

    // Second exchange with the same token → 401 (single-use).
    let replay_res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/exchange"))
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();
    assert_eq!(replay_res.status(), 401, "replay must be rejected");
}
