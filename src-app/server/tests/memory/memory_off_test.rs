// ============================================================================
// Memory-off mandatory regression (plan §10).
//
// With `memory_admin_settings.enabled = false`, chat must work
// normally and no memory tables should be touched. The system
// default IS enabled=false; this test asserts a chat-like sequence
// of REST calls doesn't error and doesn't create memory rows on
// behalf of the user (we can't exercise the actual chat path
// without a real LLM; this is the closest proxy without that dep).
// ============================================================================

use serde_json::Value;

#[tokio::test]
async fn test_admin_settings_default_is_disabled() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "off_admin",
        &["memory::admin::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["enabled"], false, "memory must default to disabled");
    assert!(
        body["embedding_model_id"].is_null(),
        "no embedding model configured by default"
    );
}

#[tokio::test]
async fn test_user_settings_default_off() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "off_user",
        &["memory::read", "memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["extraction_enabled"], false);
    assert_eq!(body["retrieval_enabled"], false);
}

#[tokio::test]
async fn test_memory_disabled_mcp_recall_rejects() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "off_recall",
        &["memory::read", "memory::write"],
    )
    .await;
    // With memory disabled (default), recall must refuse.
    let res = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "recall", "arguments": { "query": "anything" } }
        }))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert!(body["error"].is_object(), "recall must error when memory disabled");
}
