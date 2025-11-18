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
        .post(&server.api_url("/files/upload"))
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
    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with file attachment
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Please analyze this file",
        "file_ids": [file_id]
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
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
    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with multiple file attachments
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Please compare these files",
        "file_ids": [file1_id, file2_id]
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
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

    let model = super::helpers::get_or_create_test_model(&server, &user2.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Analyze this file",
        "file_ids": [file_id]
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // SSE returns 200 OK, errors are in the stream
    assert_eq!(response.status(), StatusCode::OK);
    let body_text = response.text().await.unwrap();
    // Error should be in SSE stream and indicate access denied
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

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
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
        .post(&server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // SSE returns 200 OK, errors are in the stream
    assert_eq!(response.status(), StatusCode::OK);
    let body_text = response.text().await.unwrap();
    // Error should be in SSE stream and indicate file not found
    assert!(body_text.contains("not found") || body_text.contains("FILE_NOT_FOUND") || body_text.contains("error"),
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

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with empty file_ids array
    let payload = json!({
        "model_id": model_id,
        "branch_id": branch_id,
        "content": "Hello",
        "file_ids": []
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should succeed - empty array is valid
    assert_eq!(response.status(), StatusCode::OK);
}
