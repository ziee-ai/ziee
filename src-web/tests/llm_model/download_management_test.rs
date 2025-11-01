// LLM Model Download Management Integration Tests
// Following Tier 1 & 2 SSE testing strategy from .plans/sse-testing-strategy.md

use crate::common::{TestServer, test_helpers};

// =====================================================
// List Downloads Tests
// =====================================================

#[tokio::test]
async fn test_list_downloads_requires_permission() {
    let server = TestServer::start().await;

    // User with correct permission
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    // User without permission
    let user = test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    let url = server.api_url("/llm-models/downloads");

    // Admin should be able to list downloads
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Admin should list downloads");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("downloads").is_some(), "Should have downloads array");
    assert!(body.get("total").is_some(), "Should have total count");
    assert!(body.get("page").is_some(), "Should have page number");
    assert!(body.get("per_page").is_some(), "Should have per_page");

    // Regular user without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Regular user should be forbidden");
}

#[tokio::test]
async fn test_list_downloads_unauthorized() {
    let server = TestServer::start().await;

    // No auth token should get 401
    let url = server.api_url("/llm-models/downloads");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Should be unauthorized without token");
}

#[tokio::test]
async fn test_list_downloads_pagination() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    // Test with pagination parameters
    let url = server.api_url("/llm-models/downloads?page=1&per_page=10");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body.get("page").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(body.get("per_page").and_then(|v| v.as_i64()), Some(10));
}

// =====================================================
// Get Download Tests
// =====================================================

#[tokio::test]
async fn test_get_download_not_found() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    // Try to get non-existent download
    let url = server.api_url("/llm-models/downloads/00000000-0000-0000-0000-000000000000");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404, "Non-existent download should return 404");
}

#[tokio::test]
async fn test_get_download_requires_permission() {
    let server = TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    let user = test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    let url = server.api_url("/llm-models/downloads/00000000-0000-0000-0000-000000000000");

    // Admin can access (will get 404 since download doesn't exist, but permission check passes)
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404); // 404 because download doesn't exist, not 403

    // Regular user should get 403 (permission denied before 404 check)
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should be forbidden without permission");
}

// =====================================================
// Cancel Download Tests
// =====================================================

#[tokio::test]
async fn test_cancel_download_requires_permission() {
    let server = TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_cancel"]
    ).await;

    let user = test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    let url = server.api_url("/llm-models/downloads/00000000-0000-0000-0000-000000000000/cancel");

    // Admin can access (will get 404)
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);

    // Regular user should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should be forbidden without permission");
}

// =====================================================
// Delete Download Tests
// =====================================================

#[tokio::test]
async fn test_delete_download_requires_permission() {
    let server = TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_delete"]
    ).await;

    let user = test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    let url = server.api_url("/llm-models/downloads/00000000-0000-0000-0000-000000000000");

    // Admin can access (will get 404)
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);

    // Regular user should get 403
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should be forbidden without permission");
}

// =====================================================
// SSE Subscription Tests (Tier 1 - Connection & Headers)
// =====================================================

#[tokio::test]
async fn test_subscribe_download_progress_requires_permission() {
    let server = TestServer::start().await;

    // User with correct permission
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    // User without permission
    let user = test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    let url = server.api_url("/llm-models/downloads/subscribe");

    // ✅ Admin should connect
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Admin should connect to SSE");
    assert_eq!(
        response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "Should return SSE content type"
    );

    // Don't read body to avoid hanging the test

    // ❌ Regular user should be denied
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Regular user should be forbidden");
}

#[tokio::test]
async fn test_subscribe_download_progress_unauthorized() {
    let server = TestServer::start().await;

    // No auth token → 401
    let url = server.api_url("/llm-models/downloads/subscribe");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Should be unauthorized without token");
}

#[tokio::test]
async fn test_subscribe_download_progress_sse_format() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    let url = server.api_url("/llm-models/downloads/subscribe");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    // Verify SSE content type
    let content_type = response.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .expect("Should have content-type header");

    assert!(
        content_type.contains("text/event-stream"),
        "Content type should be text/event-stream, got: {}",
        content_type
    );

    // Note: We don't read the response body because SSE streams are endless
    // and would cause the test to hang. The content-type verification is sufficient
    // for Tier 1 testing. The actual "Connected" event will be sent immediately,
    // followed by either "Complete" (no active downloads) or "Update" events.
}

// =====================================================
// SSE Event Format Tests (Tier 2 - Optional)
// =====================================================

#[tokio::test]
async fn test_subscribe_download_progress_connected_event() {
    use futures_util::StreamExt;
    use tokio::time::{timeout, Duration};

    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"]
    ).await;

    let url = server.api_url("/llm-models/downloads/subscribe");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    // Read stream with 5-second timeout
    let mut stream = response.bytes_stream();

    // Read first chunk (should be Connected event)
    let first_chunk = timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("Timeout waiting for first event")
        .expect("Stream ended prematurely")
        .expect("Failed to read chunk");

    let event_text = String::from_utf8(first_chunk.to_vec())
        .expect("Failed to convert to UTF-8");

    // Verify SSE format
    assert!(event_text.contains("event:"), "Should have event type");
    assert!(event_text.contains("data:"), "Should have data field");

    // Verify this is the Connected event
    assert!(
        event_text.contains("Connected") || event_text.contains("Complete"),
        "First event should be Connected or Complete (if no downloads), got: {}",
        event_text
    );

    // Don't read more - drop the connection
}
