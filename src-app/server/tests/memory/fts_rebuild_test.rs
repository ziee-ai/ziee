//! audit id all-8a7efaae6e4a — the FTS-rebuild endpoints (trigger_fts_rebuild +
//! get_fts_rebuild_status, handlers.rs:843-1060) were completely untested. These
//! cover the validation gate, the same-dictionary short-circuit, the status
//! read, and the permission gate — none of which spawn the real DDL rewrite.

use serde_json::Value;

fn admin_perms() -> &'static [&'static str] {
    &["memory::admin::read", "memory::admin::manage"]
}

#[tokio::test]
async fn test_fts_rebuild_rejects_dictionary_not_in_allowlist() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "fts_badword",
        admin_perms(),
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "dictionary": "klingon" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "non-allowlisted dictionary must be 400");
    let body: Value = res.json().await.unwrap_or_default();
    assert_eq!(body["error_code"], "VALIDATION_ERROR", "body: {body}");
}

#[tokio::test]
async fn test_fts_rebuild_same_dictionary_is_noop() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "fts_same",
        admin_perms(),
    )
    .await;

    // Read the current dictionary, then request a rebuild to that same value —
    // the handler short-circuits (no DDL) with started=false.
    let cur: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let dict = cur["fts_dictionary"].as_str().expect("fts_dictionary present").to_string();

    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "dictionary": dict }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["started"], false, "same-dictionary rebuild must short-circuit: {body}");
}

#[tokio::test]
async fn test_fts_rebuild_status_is_readable() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "fts_status",
        admin_perms(),
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/memory/admin/fts/rebuild/status"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    // Idle by default → in_progress=false.
    assert_eq!(body["in_progress"], false, "no rebuild running by default: {body}");
}

#[tokio::test]
async fn test_fts_rebuild_requires_manage_permission() {
    let server = crate::common::TestServer::start().await;
    // Read-only admin (no manage) must be forbidden from triggering.
    let reader = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "fts_reader",
        &["memory::admin::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .json(&serde_json::json!({ "dictionary": "english" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "trigger must require memory::admin::manage");
}
