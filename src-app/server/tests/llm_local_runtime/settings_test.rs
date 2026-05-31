//! Tier 2 — runtime-settings singleton (GET/PUT, validation, gating).

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;

async fn get_settings(server: &TestServer, token: &str) -> serde_json::Value {
    let resp = reqwest::Client::new()
        .get(server.api_url("/local-runtime/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.unwrap()
}

#[tokio::test]
async fn get_returns_defaults() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let s = get_settings(&server, &admin.token).await;
    assert_eq!(s["idle_unload_secs"].as_i64(), Some(1800));
    assert_eq!(s["auto_start_timeout_secs"].as_i64(), Some(30));
    assert_eq!(s["drain_timeout_secs"].as_i64(), Some(30));
}

#[tokio::test]
async fn put_updates_all_fields() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let resp = lrt::update_runtime_settings(
        &server,
        &admin.token,
        json!({
            "idle_unload_secs": 120,
            "auto_start_timeout_secs": 20,
            "drain_timeout_secs": 15
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let s = get_settings(&server, &admin.token).await;
    assert_eq!(s["idle_unload_secs"].as_i64(), Some(120));
    assert_eq!(s["auto_start_timeout_secs"].as_i64(), Some(20));
    assert_eq!(s["drain_timeout_secs"].as_i64(), Some(15));
}

#[tokio::test]
async fn partial_patch_preserves_other_fields() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    // Change only idle_unload_secs.
    let resp = lrt::update_runtime_settings(&server, &admin.token, json!({ "idle_unload_secs": 60 })).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let s = get_settings(&server, &admin.token).await;
    assert_eq!(s["idle_unload_secs"].as_i64(), Some(60));
    // Defaults preserved.
    assert_eq!(s["auto_start_timeout_secs"].as_i64(), Some(30));
    assert_eq!(s["drain_timeout_secs"].as_i64(), Some(30));
}

#[tokio::test]
async fn out_of_range_values_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let bad_idle = lrt::update_runtime_settings(&server, &admin.token, json!({ "idle_unload_secs": -5 })).await;
    assert_eq!(bad_idle.status(), StatusCode::BAD_REQUEST, "negative idle rejected");

    let bad_timeout = lrt::update_runtime_settings(&server, &admin.token, json!({ "auto_start_timeout_secs": 0 })).await;
    assert_eq!(bad_timeout.status(), StatusCode::BAD_REQUEST, "zero auto-start timeout rejected");

    let bad_drain = lrt::update_runtime_settings(&server, &admin.token, json!({ "drain_timeout_secs": 9999 })).await;
    assert_eq!(bad_drain.status(), StatusCode::BAD_REQUEST, "drain > 600 rejected");
}

#[tokio::test]
async fn settings_permission_gating() {
    let server = TestServer::start().await;
    // Instance-read only — lacks settings_read + settings_manage.
    let user =
        create_user_with_only_permissions(&server, "reader", &["llm_local_runtime::read"]).await;

    let get = reqwest::Client::new()
        .get(server.api_url("/local-runtime/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::FORBIDDEN, "GET needs settings_read");

    let put = lrt::update_runtime_settings(&server, &user.token, json!({ "idle_unload_secs": 10 })).await;
    assert_eq!(put.status(), StatusCode::FORBIDDEN, "PUT needs settings_manage");
}
