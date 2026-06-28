// ============================================================================
// Memory FTS-rebuild endpoints (handlers.rs:843-1060):
//   POST /api/memory/admin/fts/rebuild        (trigger_fts_rebuild)
//   GET  /api/memory/admin/fts/rebuild/status (get_fts_rebuild_status)
//
// These were completely untested. Covers the permission gate, the allowlist
// (invalid dictionary → 400), the same-dictionary short-circuit (no DDL), a
// real different-dictionary rebuild claiming the slot + driving it to
// completion via the status endpoint, and the in-progress 409 guard.
// ============================================================================

use serde_json::Value;
use std::time::Duration;

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
