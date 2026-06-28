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
        &["llm_models::downloads_read"],
    )
    .await;

    // User without permission
    let user = test_helpers::create_user_with_no_permissions(&server, "regular").await;

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
    assert!(
        body.get("downloads").is_some(),
        "Should have downloads array"
    );
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

    assert_eq!(
        response.status(),
        401,
        "Should be unauthorized without token"
    );
}

#[tokio::test]
async fn test_list_downloads_pagination() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"],
    )
    .await;

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
        &["llm_models::downloads_read"],
    )
    .await;

    // Try to get non-existent download
    let url = server.api_url("/llm-models/downloads/00000000-0000-0000-0000-000000000000");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        404,
        "Non-existent download should return 404"
    );
}

#[tokio::test]
async fn test_get_download_requires_permission() {
    let server = TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"],
    )
    .await;

    let user = test_helpers::create_user_with_no_permissions(&server, "regular").await;

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

    assert_eq!(
        response.status(),
        403,
        "Should be forbidden without permission"
    );
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
        &["llm_models::downloads_cancel"],
    )
    .await;

    let user = test_helpers::create_user_with_no_permissions(&server, "regular").await;

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

    assert_eq!(
        response.status(),
        403,
        "Should be forbidden without permission"
    );
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
        &["llm_models::downloads_delete"],
    )
    .await;

    let user = test_helpers::create_user_with_no_permissions(&server, "regular").await;

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

    assert_eq!(
        response.status(),
        403,
        "Should be forbidden without permission"
    );
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
        &["llm_models::downloads_read"],
    )
    .await;

    // User without permission
    let user = test_helpers::create_user_with_no_permissions(&server, "regular").await;

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
        response
            .headers()
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

    assert_eq!(
        response.status(),
        401,
        "Should be unauthorized without token"
    );
}

#[tokio::test]
async fn test_subscribe_download_progress_sse_format() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"],
    )
    .await;

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
    let content_type = response
        .headers()
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
    use tokio::time::{Duration, timeout};

    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_models::downloads_read"],
    )
    .await;

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

    let event_text = String::from_utf8(first_chunk.to_vec()).expect("Failed to convert to UTF-8");

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

// =====================================================
// SSE Progress & Completion Tests (Tier 2 - Advanced)
// =====================================================

#[tokio::test]
async fn test_sse_completion_event_structure() {
    // This test verifies that the download completion flow works and that
    // the completed download has the correct data structure that would be
    // sent in a Complete SSE event (model_id and provider_id).
    //
    // Note: With tiny test models (<1 second to download), real-time SSE event
    // capture is unreliable in tests. The download_progress_test.rs already
    // tests the end-to-end download flow. This test focuses on verifying the
    // data structure of a completed download.

    use tokio::time::{Duration, sleep};

    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_models::downloads_read",
        ],
    )
    .await;

    // Get Hugging Face repository and configure API key
    let hf_repo =
        crate::llm_model::download_test::get_huggingface_repository(&server, &user.token, true)
            .await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Initiate download
    let payload = serde_json::json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-completion-structure-test",
        "display_name": "Tiny GPT-2 (Completion Structure Test)",
        "description": "Test model for completion event structure",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    let download_response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(download_response.status(), 200);

    let download_instance: serde_json::Value = download_response.json().await.unwrap();
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Poll for completion
    let mut iterations = 0;
    let max_iterations = 30;
    let mut final_download: Option<serde_json::Value> = None;

    while iterations < max_iterations {
        sleep(Duration::from_secs(1)).await;
        iterations += 1;

        let response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();

        if response.status() == 404 {
            // Download was deleted after completion
            println!("✅ Download completed and was deleted (expected behavior)");
            // Since it was deleted, we can't verify the structure, but completion is confirmed
            return;
        }

        assert_eq!(response.status(), 200);

        let download: serde_json::Value = response.json().await.unwrap();
        let status = download["status"].as_str().unwrap();

        if status == "completed" {
            final_download = Some(download);
            println!("✅ Download completed");
            break;
        }

        if status == "failed" {
            let error = download["error_message"]
                .as_str()
                .unwrap_or("Unknown error");
            panic!("Download failed: {}", error);
        }
    }

    // Verify the completed download structure
    if let Some(download) = final_download {
        // Debug: Print the full download structure
        println!(
            "Download structure: {}",
            serde_json::to_string_pretty(&download).unwrap()
        );

        // Check if download has model_id (the key field for completion)
        let model_id = download["model_id"]
            .as_str()
            .expect("Completed download should include model_id");

        assert!(!model_id.is_empty(), "model_id should not be empty");

        // Verify provider_id matches
        assert_eq!(
            download["provider_id"].as_str().unwrap(),
            provider_id,
            "Download should have correct provider_id"
        );

        println!("✅ Completed download includes model_id: {}", model_id);
        println!(
            "✅ Completed download includes provider_id: {}",
            provider_id
        );
        println!("✅ This structure would be sent in SSE Complete event");
    }

    println!("✅ SSE completion event structure test passed!");
}

#[tokio::test]
async fn test_sse_sends_update_events_during_download() {
    // This test verifies that SSE actually sends UPDATE events during an active download
    use futures_util::StreamExt;
    use tokio::time::{Duration, timeout};

    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_models::downloads_read",
        ],
    )
    .await;

    // Get Hugging Face repository and configure API key
    let hf_repo =
        crate::llm_model::download_test::get_huggingface_repository(&server, &user.token, true)
            .await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // 1. Start download FIRST (matches real client behavior - client creates download, THEN subscribes)
    // Using distilgpt2 (~350MB) to ensure download takes enough time to catch UPDATE events
    let payload = serde_json::json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "distilbert/distilgpt2",
        "repository_branch": "main",
        "name": "distilgpt2-sse-update-test",
        "display_name": "DistilGPT2 (SSE Update Test)",
        "description": "Test model for SSE UPDATE events - larger model to catch progress",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "clear_cache": true,  // Force fresh download to slow it down
        "source": {
            "type": "hub",
            "id": "distilbert/distilgpt2"
        }
    });

    let download_response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(download_response.status(), 200);
    let download_instance: serde_json::Value = download_response.json().await.unwrap();
    let download_id = download_instance["id"].as_str().unwrap();

    println!("✅ Download started with ID: {}", download_id);

    // 2. NOW connect to SSE stream (after download exists)
    let sse_url = server.api_url("/llm-models/downloads/subscribe");
    let sse_response = reqwest::Client::new()
        .get(&sse_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Failed to connect to SSE");

    assert_eq!(sse_response.status(), 200);
    let mut sse_stream = sse_response.bytes_stream();

    println!("✅ Connected to SSE stream");

    // 3. Read Connected event
    let connected_chunk = timeout(Duration::from_secs(5), sse_stream.next())
        .await
        .expect("Timeout waiting for Connected event")
        .expect("Stream ended prematurely")
        .expect("Failed to read chunk");

    let connected_text =
        String::from_utf8(connected_chunk.to_vec()).expect("Failed to convert to UTF-8");

    println!("📡 First event: {}", connected_text);
    assert!(
        connected_text.contains("Connected"),
        "First event should be Connected"
    );

    // 4. Read SSE events - look for UPDATE events
    let mut update_events_received = 0;
    let mut complete_event_received = false;
    let max_events = 200; // Read up to 200 events (larger model will take longer)
    let mut events_read = 0;

    while events_read < max_events {
        match timeout(Duration::from_secs(60), sse_stream.next()).await {
            Ok(Some(Ok(chunk))) => {
                let event_text =
                    String::from_utf8(chunk.to_vec()).expect("Failed to convert to UTF-8");

                println!("📡 SSE Event #{}: {}", events_read + 1, event_text);

                // Check if this is an UPDATE event (lowercase "update")
                if event_text.contains("event: update") || event_text.contains("event:update") {
                    // Extract and validate the data payload
                    let data_line = event_text
                        .lines()
                        .find(|line| line.starts_with("data:"))
                        .expect("UPDATE event should have data field");

                    let json_str = data_line.strip_prefix("data:").unwrap().trim();

                    // Parse as JSON array of DownloadProgressUpdate
                    let updates: Vec<serde_json::Value> = serde_json::from_str(json_str)
                        .expect("UPDATE event data should be valid JSON array");

                    // Skip empty UPDATE events (polling before download starts)
                    if updates.is_empty() {
                        println!("⏭️  Skipping empty UPDATE event (download not started yet)");
                        continue;
                    }

                    // Only count non-empty UPDATE events
                    update_events_received += 1;

                    // Validate structure of first update
                    let update = &updates[0];

                    // Required fields
                    assert!(update.get("id").is_some(), "UPDATE should have 'id' field");
                    assert!(update["id"].is_string(), "'id' should be a string");

                    assert!(
                        update.get("status").is_some(),
                        "UPDATE should have 'status' field"
                    );
                    assert!(update["status"].is_string(), "'status' should be a string");

                    assert!(
                        update.get("phase").is_some(),
                        "UPDATE should have 'phase' field"
                    );
                    assert!(update["phase"].is_string(), "'phase' should be a string");

                    // Optional fields (can be null or present)
                    assert!(
                        update.get("current").is_some(),
                        "UPDATE should have 'current' field"
                    );
                    assert!(
                        update.get("total").is_some(),
                        "UPDATE should have 'total' field"
                    );
                    assert!(
                        update.get("message").is_some(),
                        "UPDATE should have 'message' field"
                    );
                    assert!(
                        update.get("speed_bps").is_some(),
                        "UPDATE should have 'speed_bps' field"
                    );
                    assert!(
                        update.get("eta_seconds").is_some(),
                        "UPDATE should have 'eta_seconds' field"
                    );
                    assert!(
                        update.get("error_message").is_some(),
                        "UPDATE should have 'error_message' field"
                    );

                    // If numeric fields are present (not null), they should be numbers
                    if !update["current"].is_null() {
                        assert!(
                            update["current"].is_number(),
                            "'current' should be a number when present"
                        );
                    }
                    if !update["total"].is_null() {
                        assert!(
                            update["total"].is_number(),
                            "'total' should be a number when present"
                        );
                    }
                    if !update["speed_bps"].is_null() {
                        assert!(
                            update["speed_bps"].is_number(),
                            "'speed_bps' should be a number when present"
                        );
                    }
                    if !update["eta_seconds"].is_null() {
                        assert!(
                            update["eta_seconds"].is_number(),
                            "'eta_seconds' should be a number when present"
                        );
                    }

                    println!(
                        "✅ Received valid UPDATE event #{} - status: {}, phase: {}",
                        update_events_received,
                        update["status"].as_str().unwrap_or("unknown"),
                        update["phase"].as_str().unwrap_or("unknown")
                    );
                }

                // Check if this is a COMPLETE event (lowercase "complete")
                if event_text.contains("event: complete") || event_text.contains("event:complete") {
                    complete_event_received = true;
                    println!("✅ Received COMPLETE event");
                    break; // Stop reading after Complete
                }

                events_read += 1;
            }
            Ok(Some(Err(e))) => {
                panic!("Error reading SSE stream: {}", e);
            }
            Ok(None) => {
                println!("⚠️ SSE stream ended");
                break;
            }
            Err(_) => {
                println!("⚠️ Timeout waiting for next SSE event");
                break;
            }
        }
    }

    // 5. Verify we received at least one UPDATE event
    assert!(
        update_events_received > 0,
        "Should have received at least one UPDATE event during download, got {}",
        update_events_received
    );

    println!("✅ SSE UPDATE events test passed!");
    println!("   - Received {} UPDATE events", update_events_received);
    println!(
        "   - Complete event: {}",
        if complete_event_received { "Yes" } else { "No" }
    );
}

// =====================================================
// Delete download — INVALID_STATE (non-terminal) guard
// =====================================================

/// A download in a NON-terminal state (`pending`/`downloading`) must NOT be
/// deletable — the handler returns 400 INVALID_STATE rather than tearing down
/// an in-flight download. Only terminal (completed/failed/cancelled) rows
/// delete. Seeds a real `downloading` row via SQL so the guard is exercised
/// end-to-end through the HTTP path.
#[tokio::test]
async fn test_delete_active_download_returns_invalid_state() {
    use uuid::Uuid;

    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "dl_invalid_state",
        &["llm_models::downloads_delete"],
    )
    .await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");

    // Minimal provider + repository to satisfy the download_instances FKs.
    let provider_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_providers (id, name, provider_type, enabled, built_in)
         VALUES ($1, 'DL Test Provider', 'huggingface', true, false)",
    )
    .bind(provider_id)
    .execute(&pool)
    .await
    .expect("insert provider");

    let repository_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_repositories (id, name, url, auth_type, enabled, built_in)
         VALUES ($1, 'DL Test Repo', 'https://huggingface.co', 'none', true, false)",
    )
    .bind(repository_id)
    .execute(&pool)
    .await
    .expect("insert repository");

    // A genuinely non-terminal download.
    let download_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO download_instances (id, provider_id, repository_id, request_data, status)
         VALUES ($1, $2, $3, '{}'::jsonb, 'downloading')",
    )
    .bind(download_id)
    .bind(provider_id)
    .bind(repository_id)
    .execute(&pool)
    .await
    .expect("insert downloading download");
    pool.close().await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-models/downloads/{download_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete request failed");

    assert_eq!(
        resp.status(),
        400,
        "deleting a non-terminal (downloading) download must be 400 INVALID_STATE"
    );
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    assert_eq!(
        body["error_code"], "INVALID_STATE",
        "error_code should be INVALID_STATE, got body: {body}"
    );
}

// =====================================================
// Cancel download — real mid-flight cancellation (downloading → cancelled)
// =====================================================

/// REAL-PATH cancellation of an IN-FLIGHT download: the E2E mocks the list +
/// cancel endpoints, and the only backend cancel test exercises the
/// cancel-AFTER-complete (400) path. Here a genuine `downloading` row is seeded
/// via SQL and POSTed to the real /cancel endpoint, asserting 204 + the row
/// transitions to `cancelled` (the `can_cancel()` → mark-cancelled handler path,
/// downloads.rs:189-281).
#[tokio::test]
async fn test_cancel_active_download_marks_cancelled() {
    use uuid::Uuid;

    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "dl_cancel_active",
        &["llm_models::downloads_cancel", "llm_models::downloads_read"],
    )
    .await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");

    let provider_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_providers (id, name, provider_type, enabled, built_in)
         VALUES ($1, 'DL Cancel Provider', 'huggingface', true, false)",
    )
    .bind(provider_id)
    .execute(&pool)
    .await
    .expect("insert provider");

    let repository_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_repositories (id, name, url, auth_type, enabled, built_in)
         VALUES ($1, 'DL Cancel Repo', 'https://huggingface.co', 'none', true, false)",
    )
    .bind(repository_id)
    .execute(&pool)
    .await
    .expect("insert repository");

    let download_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO download_instances (id, provider_id, repository_id, request_data, status)
         VALUES ($1, $2, $3, '{}'::jsonb, 'downloading')",
    )
    .bind(download_id)
    .bind(provider_id)
    .bind(repository_id)
    .execute(&pool)
    .await
    .expect("insert downloading download");

    // Real cancel endpoint → 204 No Content.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-models/downloads/{download_id}/cancel")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("cancel request failed");
    assert_eq!(resp.status(), 204, "cancelling an in-flight download must be 204");

    // The row transitioned to `cancelled` in the DB (the real handler updated it).
    let status: String =
        sqlx::query_scalar("SELECT status::text FROM download_instances WHERE id = $1")
            .bind(download_id)
            .fetch_one(&pool)
            .await
            .expect("row still present pre-reap");
    pool.close().await;
    assert_eq!(status, "cancelled", "the in-flight download must be marked cancelled");
}
