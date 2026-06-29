use serde_json::Value;
use std::time::Duration;

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

async fn admin(server: &crate::common::TestServer, name: &str) -> crate::common::test_helpers::TestUser {
    crate::common::test_helpers::create_user_with_permissions(
        server,
        name,
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await
}

#[tokio::test]
async fn fts_rebuild_requires_manage_permission() {
    let server = crate::common::TestServer::start().await;
    // A user with only read (not manage) must be refused on the trigger.
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
    assert_eq!(res.status(), 403, "trigger requires memory::admin::manage");
}

#[tokio::test]
async fn fts_rebuild_rejects_dictionary_not_in_allowlist() {
    let server = crate::common::TestServer::start().await;
    let admin = admin(&server, "fts_badword").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "dictionary": "klingon" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error_code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn fts_rebuild_same_dictionary_short_circuits() {
    let server = crate::common::TestServer::start().await;
    let admin = admin(&server, "fts_same").await;

    // The seeded default dictionary is 'simple' (migration 89).
    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "dictionary": "simple" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["started"], false, "same-dictionary must not start a rebuild: {body}");
    assert_eq!(body["reason"], "dictionary unchanged");
}

#[tokio::test]
async fn fts_rebuild_status_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    // A user with neither memory admin perm.
    let nobody =
        crate::common::test_helpers::create_user_with_permissions(&server, "fts_nobody", &[]).await;

    let res = reqwest::Client::new()
        .get(server.api_url("/memory/admin/fts/rebuild/status"))
        .header("Authorization", format!("Bearer {}", nobody.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "status requires memory::admin::read");
}

#[tokio::test]
async fn fts_rebuild_different_dictionary_runs_and_status_reaches_completion() {
    let server = crate::common::TestServer::start().await;
    let admin = admin(&server, "fts_run").await;
    let client = reqwest::Client::new();

    // Status starts idle (no rebuild has ever run).
    let status: Value = client
        .get(server.api_url("/memory/admin/fts/rebuild/status"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["in_progress"], false, "idle before any rebuild: {status}");

    // Trigger a real rebuild to a DIFFERENT valid dictionary (simple → english).
    // The user_memories table is empty for a fresh deployment, so the GENERATED
    // column rebuild + index recreate complete quickly.
    let res = client
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "dictionary": "english" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["started"], true, "different dictionary must start a rebuild: {body}");

    // Poll the status endpoint until the rebuild completes.
    let mut completed = false;
    for _ in 0..100 {
        let s: Value = client
            .get(server.api_url("/memory/admin/fts/rebuild/status"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if s["in_progress"] == false && !s["completed_at"].is_null() {
            completed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(completed, "FTS rebuild must reach completion (in_progress=false + completed_at set)");

    // The active dictionary is now 'english'.
    let settings: Value = client
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["fts_dictionary"], "english", "rebuild must switch the active dictionary");
}

#[tokio::test]
async fn test_fts_rebuild_trigger_and_status() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "fts_admin",
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin.token);

    // Initial status: not in progress.
    let s0: Value = client
        .get(server.api_url("/memory/admin/fts/rebuild/status"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(s0["in_progress"], false, "no rebuild running initially: {s0}");

    // Trigger the rebuild.
    let trigger = client
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert!(
        trigger.status().is_success(),
        "trigger should succeed: {} {}",
        trigger.status(),
        trigger.text().await.unwrap_or_default()
    );

    // After a synchronous rebuild (no memories), status reports completion
    // (not in progress, completed_at set).
    let s1: Value = client
        .get(server.api_url("/memory/admin/fts/rebuild/status"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(s1["in_progress"], false, "rebuild finished: {s1}");
    assert!(!s1["completed_at"].is_null(), "completed_at must be set after a rebuild: {s1}");
}

#[tokio::test]
async fn test_fts_rebuild_requires_manage_permission_v2() {
    let server = crate::common::TestServer::start().await;
    // Read-only memory admin can read status but NOT trigger a rebuild.
    let reader = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "fts_reader",
        &["memory::admin::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin/fts/rebuild"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "rebuild trigger must require memory::admin::manage");
}

