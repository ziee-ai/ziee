//! SSE streaming integration tests

use reqwest::StatusCode;

// =====================================================
// Basic Streaming Tests
// =====================================================

#[tokio::test]
async fn test_send_message_returns_sse_content_type() {
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
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello"
    ).await;

    // Verify SSE content type
    let content_type = response.headers().get("content-type").unwrap();
    assert!(
        content_type.to_str().unwrap().contains("text/event-stream"),
        "Expected SSE content-type"
    );
}

#[tokio::test]
async fn test_send_message_stream_contains_data_events() {
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
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Test message"
    ).await;

    let bytes = response.bytes().await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    // Should contain SSE data events
    assert!(text.contains("data: "), "Should contain SSE data events");
}

#[tokio::test]
async fn test_send_message_stream_parses_json_chunks() {
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
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Parse test"
    ).await;

    let _chunks = super::helpers::parse_sse_stream(response).await;

    // Should be able to parse the SSE stream without panicking
    // (actual AI response may vary, so we just verify parsing works)
}

// =====================================================
// Chunk Structure Tests
// =====================================================

#[tokio::test]
async fn test_stream_chunks_have_expected_fields() {
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
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Chunk test"
    ).await;

    let chunks = super::helpers::parse_sse_stream(response).await;

    // If we have chunks, verify they have expected fields
    if !chunks.is_empty() {
        let first_chunk = &chunks[0];

        // ChatStreamChunk should have these fields (may be null/empty)
        assert!(first_chunk.get("content").is_some(), "Should have content field");
        assert!(first_chunk.get("message_id").is_some(), "Should have message_id field");
        assert!(first_chunk.get("conversation_id").is_some(), "Should have conversation_id field");
        assert!(first_chunk.get("branch_id").is_some(), "Should have branch_id field");
    }
}

// =====================================================
// Error Handling Tests
// =====================================================

#[tokio::test]
async fn test_stream_error_on_invalid_model() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let fake_model_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        fake_model_id,
        branch_id,
        "Error test"
    ).await;

    // Should return 404 before streaming starts
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
