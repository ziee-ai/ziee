//! FTS rebuild admin endpoints:
//!   POST /memory/admin/fts/rebuild         (MemoryAdminManage)
//!   GET  /memory/admin/fts/rebuild/status  (MemoryAdminRead)
//!
//! Neither was exercised. We drive the trigger end-to-end (it rebuilds
//! `content_tsv` synchronously; with no memories it completes immediately) and
//! assert the status reflects completion, plus the manage-permission gate.

use serde_json::Value;

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
async fn test_fts_rebuild_requires_manage_permission() {
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
