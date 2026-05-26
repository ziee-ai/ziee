// ============================================================================
// Onboarding-driven admin settings initialization (plan §9 Phase 1).
//
// The MemorySetupStep posts the admin's choice to
// `PUT /api/admin/memory-settings`. This test asserts the happy path:
// an admin user sends the request; the row is updated; subsequent
// GETs reflect the new state.
// ============================================================================

use serde_json::Value;

#[tokio::test]
async fn test_admin_can_set_enabled_via_onboarding_flow() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "onb_admin",
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await;

    let client = reqwest::Client::new();
    let token = &admin.token;

    // Sanity: starts disabled.
    let res = client
        .get(server.api_url("/admin/memory-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], false);

    // Apply onboarding choice (Skip → still disabled).
    let res = client
        .put(server.api_url("/admin/memory-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], false);

    // Apply onboarding choice (Enable → now enabled).
    let res = client
        .put(server.api_url("/admin/memory-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], true);
}

#[tokio::test]
async fn test_non_admin_cannot_change_admin_settings() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "onb_user",
        &["memory::read", "memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/admin/memory-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}
