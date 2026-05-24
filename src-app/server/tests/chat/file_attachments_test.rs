//! Integration tests for chat file attachments (file extension)

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

// Helper to upload a test file and return file ID
async fn upload_test_file(
    server: &crate::common::TestServer,
    token: &str,
    filename: &str,
    content: &[u8],
    mime_type: &str,
) -> Uuid {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content.to_vec())
            .file_name(filename.to_string())
            .mime_str(mime_type)
            .unwrap(),
    );

    let response = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    super::helpers::parse_uuid(&body["id"])
}

// =====================================================
// Basic File Attachment Tests
// =====================================================

#[tokio::test]
async fn test_send_message_with_single_file() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "files::upload",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Upload a test file
    let file_content = b"This is a test file for chat attachment.";
    let file_id = upload_test_file(
        &server,
        &user.token,
        "test.txt",
        file_content,
        "text/plain",
    )
    .await;

    // Create conversation
    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Get test model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with file attachment
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Please analyze this file",
        "file_ids": [file_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Verify SSE response
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/event-stream"));
}

#[tokio::test]
async fn test_send_message_with_multiple_files() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "files::upload",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Upload multiple test files
    let file1_id = upload_test_file(
        &server,
        &user.token,
        "file1.txt",
        b"First file content",
        "text/plain",
    )
    .await;

    let file2_id = upload_test_file(
        &server,
        &user.token,
        "file2.txt",
        b"Second file content",
        "text/plain",
    )
    .await;

    // Create conversation
    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Get test model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with multiple file attachments
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Please compare these files",
        "file_ids": [file1_id, file2_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Verify SSE response
    assert_eq!(response.status(), StatusCode::OK);
}

// =====================================================
// File Ownership and Security Tests
// =====================================================

#[tokio::test]
async fn test_cannot_attach_other_users_file() {
    let server = crate::common::TestServer::start().await;

    // Create two users
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["files::upload"],
    )
    .await;

    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // User1 uploads a file
    let file_id = upload_test_file(
        &server,
        &user1.token,
        "private.txt",
        b"Private content",
        "text/plain",
    )
    .await;

    // User2 tries to use user1's file in a message
    let conversation =
        super::helpers::create_conversation(&server, &user2.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user2.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Analyze this file",
        "file_ids": [file_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // File ownership validation happens in extension hook before streaming starts
    // Access denied returns HTTP 403 Forbidden
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body_text = response.text().await.unwrap();
    // Error body should indicate access denied
    assert!(body_text.contains("access") || body_text.contains("forbidden") || body_text.contains("FILE_ACCESS_DENIED"),
        "Expected access denied error, got: {}", body_text);
}

#[tokio::test]
async fn test_send_message_with_nonexistent_file() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Use a random UUID that doesn't exist
    let fake_file_id = Uuid::new_v4();

    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Analyze this file",
        "file_ids": [fake_file_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // File validation happens in extension hook before streaming starts
    // Invalid files return HTTP 404 (file not found)
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body_text = response.text().await.unwrap();
    // Error body should indicate file not found
    assert!(body_text.contains("not found") || body_text.contains("FILE_NOT_FOUND") || body_text.contains("File"),
        "Expected file not found error, got: {}", body_text);
}

#[tokio::test]
async fn test_send_message_with_empty_file_list() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with empty file_ids array
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Hello",
        "file_ids": []
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should succeed - empty array is valid
    assert_eq!(response.status(), StatusCode::OK);
}

// =====================================================
// Extension Content Storage Tests
// =====================================================

#[tokio::test]
async fn test_file_extension_stores_content_as_extension() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "messages::read",
            "files::upload",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Upload a test file
    let file_content = b"Test file content for storage verification";
    let file_id = upload_test_file(
        &server,
        &user.token,
        "test.txt",
        file_content,
        "text/plain",
    )
    .await;

    // Create conversation
    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Get test model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with file attachment
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Please analyze this file",
        "file_ids": [file_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Parse SSE stream to get user message ID
    let events = super::helpers::parse_sse_events(response).await;
    let user_message_id = events
        .iter()
        .find(|e| e.event == "started")
        .and_then(|e| e.data.get("user_message_id"))
        .and_then(|v| v.as_str())
        .expect("Expected user_message_id in started event");

    let user_message_id = super::helpers::parse_uuid(&serde_json::Value::String(user_message_id.to_string()));

    // Retrieve message via API to verify content blocks
    let message = super::helpers::get_message(&server, &user.token, user_message_id).await;
    let content_blocks = message["contents"].as_array().expect("Expected 'contents' array in message");

    // Should have 2 content blocks: text (0) and extension (1)
    assert_eq!(content_blocks.len(), 2, "Expected 2 content blocks (text + file)");

    // Verify text content block at position 0
    assert_eq!(content_blocks[0]["content_type"], "text");
    assert_eq!(content_blocks[0]["sequence_order"], 0);
    assert_eq!(content_blocks[0]["content"]["type"], "text");
    assert_eq!(content_blocks[0]["content"]["text"], "Please analyze this file");

    // Verify file attachment content block at position 1 (flattened structure)
    assert_eq!(content_blocks[1]["content_type"], "file_attachment");
    assert_eq!(content_blocks[1]["sequence_order"], 1);
    assert_eq!(content_blocks[1]["content"]["type"], "file_attachment");
    assert_eq!(content_blocks[1]["content"]["file_id"], file_id.to_string());
    assert_eq!(content_blocks[1]["content"]["filename"], "test.txt");
    assert_eq!(content_blocks[1]["content"]["mime_type"], "text/plain");
    assert!(content_blocks[1]["content"]["file_size"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_file_content_in_conversation_history() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "messages::read",
            "files::upload",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Upload a test file
    let file_id = upload_test_file(
        &server,
        &user.token,
        "document.pdf",
        b"Fake PDF content",
        "application/pdf",
    )
    .await;

    // Create conversation
    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Get test model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with file attachment
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Summarize this document",
        "file_ids": [file_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Wait for stream to complete
    let _ = response.text().await.unwrap();

    // Retrieve conversation history (returns array of messages directly)
    let history = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;
    let messages = history.as_array().expect("Expected history to be an array of messages");

    // Find user message
    let user_message = messages
        .iter()
        .find(|m| m["role"] == "user")
        .expect("Expected user message in history");

    // Verify content blocks (conversation history uses 'contents' plural)
    let content_blocks = user_message["contents"].as_array().unwrap();
    assert_eq!(content_blocks.len(), 2, "Expected 2 content blocks (text + file)");

    // Verify text content
    assert_eq!(content_blocks[0]["content_type"], "text");
    assert_eq!(content_blocks[0]["content"]["type"], "text");
    assert_eq!(content_blocks[0]["content"]["text"], "Summarize this document");

    // Verify file attachment content (flattened structure)
    assert_eq!(content_blocks[1]["content_type"], "file_attachment");
    assert_eq!(content_blocks[1]["content"]["type"], "file_attachment");
    assert_eq!(content_blocks[1]["content"]["file_id"], file_id.to_string());
    assert_eq!(content_blocks[1]["content"]["filename"], "document.pdf");
    assert_eq!(content_blocks[1]["content"]["mime_type"], "application/pdf");
}

#[tokio::test]
async fn test_multiple_files_content_ordering() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "messages::read",
            "files::upload",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Upload multiple test files with different types
    let file1_id = upload_test_file(
        &server,
        &user.token,
        "image.jpg",
        b"JPEG image data",
        "image/jpeg",
    )
    .await;

    let file2_id = upload_test_file(
        &server,
        &user.token,
        "document.pdf",
        b"PDF document data",
        "application/pdf",
    )
    .await;

    let file3_id = upload_test_file(
        &server,
        &user.token,
        "data.txt",
        b"Text file data",
        "text/plain",
    )
    .await;

    // Create conversation
    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Get test model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with all three files in specific order
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Analyze these files",
        "file_ids": [file1_id, file2_id, file3_id]
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Parse SSE stream to get user message ID
    let events = super::helpers::parse_sse_events(response).await;
    let user_message_id = events
        .iter()
        .find(|e| e.event == "started")
        .and_then(|e| e.data.get("user_message_id"))
        .and_then(|v| v.as_str())
        .expect("Expected user_message_id in started event");

    let user_message_id = super::helpers::parse_uuid(&serde_json::Value::String(user_message_id.to_string()));

    // Retrieve message via API to verify content blocks ordering
    let message = super::helpers::get_message(&server, &user.token, user_message_id).await;
    let content_blocks = message["contents"].as_array().unwrap();

    // Should have 4 content blocks: text (0) + 3 files (1, 2, 3)
    assert_eq!(content_blocks.len(), 4, "Expected 4 content blocks (text + 3 files)");

    // Verify text at position 0
    assert_eq!(content_blocks[0]["content_type"], "text");
    assert_eq!(content_blocks[0]["sequence_order"], 0);
    assert_eq!(content_blocks[0]["content"]["type"], "text");
    assert_eq!(content_blocks[0]["content"]["text"], "Analyze these files");

    // Verify file1 (image.jpg) at position 1
    assert_eq!(content_blocks[1]["content_type"], "file_attachment");
    assert_eq!(content_blocks[1]["sequence_order"], 1);
    assert_eq!(content_blocks[1]["content"]["type"], "file_attachment");
    assert_eq!(content_blocks[1]["content"]["filename"], "image.jpg");
    assert_eq!(content_blocks[1]["content"]["mime_type"], "image/jpeg");

    // Verify file2 (document.pdf) at position 2
    assert_eq!(content_blocks[2]["content_type"], "file_attachment");
    assert_eq!(content_blocks[2]["sequence_order"], 2);
    assert_eq!(content_blocks[2]["content"]["type"], "file_attachment");
    assert_eq!(content_blocks[2]["content"]["filename"], "document.pdf");
    assert_eq!(content_blocks[2]["content"]["mime_type"], "application/pdf");

    // Verify file3 (data.txt) at position 3
    assert_eq!(content_blocks[3]["content_type"], "file_attachment");
    assert_eq!(content_blocks[3]["sequence_order"], 3);
    assert_eq!(content_blocks[3]["content"]["type"], "file_attachment");
    assert_eq!(content_blocks[3]["content"]["filename"], "data.txt");
    assert_eq!(content_blocks[3]["content"]["mime_type"], "text/plain");

    // Retrieve via conversation history API and verify order preserved
    let history = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;
    let messages = history.as_array().expect("Expected history to be an array of messages");
    let user_message = messages
        .iter()
        .find(|m| m["role"] == "user")
        .expect("Expected user message");

    let api_content = user_message["contents"].as_array().unwrap();
    assert_eq!(api_content.len(), 4);
    assert_eq!(api_content[1]["content"]["filename"], "image.jpg");
    assert_eq!(api_content[2]["content"]["filename"], "document.pdf");
    assert_eq!(api_content[3]["content"]["filename"], "data.txt");
}
