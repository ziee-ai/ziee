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
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
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
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
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
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
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
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
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

    // Filter to content chunks only (exclude titleUpdated, etc.)
    let content_chunks: Vec<_> = chunks
        .iter()
        .filter(|chunk| chunk.get("type").and_then(|t| t.as_str()) == Some("content"))
        .collect();

    // If we have content chunks, verify they have expected fields
    if !content_chunks.is_empty() {
        let first_content_chunk = content_chunks[0];

        // ChatStreamChunk should have these fields (may be null/empty)
        assert!(first_content_chunk.get("content").is_some(), "Should have content field");
        assert!(first_content_chunk.get("message_id").is_some(), "Should have message_id field");
        assert!(first_content_chunk.get("conversation_id").is_some(), "Should have conversation_id field");
        assert!(first_content_chunk.get("branch_id").is_some(), "Should have branch_id field");
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

#[tokio::test]
async fn test_sse_stream_has_event_names() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create", "llm_models::read"],
    ).await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let payload = serde_json::json!({
        "content": "Hello",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    let bytes = response.bytes().await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    // SSE format should have "event:" lines followed by "data:" lines
    assert!(text.contains("event: content"), "Stream should contain 'event: content' lines");
    assert!(text.contains("data: "), "Stream should contain 'data:' lines");

    // Count event lines
    let event_count = text.lines().filter(|line| line.starts_with("event:")).count();
    assert!(event_count > 0, "Stream should have at least one event line");

    eprintln!("✅ SSE stream properly formatted with {} event lines", event_count);
}

// =====================================================
// Extension Event Tests
// =====================================================

#[tokio::test]
async fn test_title_extension_sends_title_updated_event() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create", "llm_models::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send first message to trigger title generation
    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "What is the capital of France?",
    )
    .await;

    let events = super::helpers::parse_sse_events(response).await;

    // Debug: Print all event names
    eprintln!("DEBUG: Received {} events", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("DEBUG: Event {}: name='{}', has_type={}", i, event.event, event.data.get("type").is_some());
    }

    // Find titleUpdated event
    let title_event = events.iter().find(|e| e.event == "titleUpdated");

    if title_event.is_none() {
        // Print what we actually got for debugging
        eprintln!("ERROR: No titleUpdated event found. Events received:");
        for event in &events {
            eprintln!("  - event: {}, data: {}", event.event, event.data);
        }
    }

    assert!(
        title_event.is_some(),
        "Stream should contain titleUpdated event after first message exchange. Got {} events",
        events.len()
    );

    let title_data = &title_event.unwrap().data;
    assert!(
        title_data.get("title").is_some(),
        "titleUpdated event should have title field"
    );
    assert!(
        title_data["title"].is_string(),
        "title field should be a string"
    );

    let title_str = title_data["title"].as_str().unwrap();
    assert!(
        !title_str.is_empty(),
        "Generated title should not be empty"
    );

    eprintln!("✅ Title extension generated title: '{}'", title_str);
}

#[tokio::test]
async fn test_title_not_generated_for_subsequent_messages() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create", "llm_models::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send first message
    let _first_response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "First message",
    )
    .await;

    // Send second message - should NOT generate title
    let second_response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Second message",
    )
    .await;

    let events = super::helpers::parse_sse_events(second_response).await;

    // Should NOT have titleUpdated event
    let title_event = events.iter().find(|e| e.event == "titleUpdated");

    assert!(
        title_event.is_none(),
        "Stream should NOT contain titleUpdated event for subsequent messages"
    );

    eprintln!("✅ Title extension correctly skips title generation for subsequent messages");
}

#[tokio::test]
async fn test_sse_events_have_correct_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create", "llm_models::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Test event structure",
    )
    .await;

    let events = super::helpers::parse_sse_events(response).await;

    // Should have content events
    let content_events: Vec<_> = events.iter().filter(|e| e.event == "content").collect();
    assert!(
        !content_events.is_empty(),
        "Stream should contain content events"
    );

    // Verify content event structure
    for event in &content_events {
        assert!(
            event.data.get("type").is_some(),
            "Content event should have 'type' field"
        );
        let event_type = event.data["type"].as_str().unwrap();
        assert_eq!(
            event_type, "content",
            "Content event should have type='content'"
        );
    }

    // Should have complete event
    let complete_events: Vec<_> = events.iter().filter(|e| e.event == "complete").collect();
    assert_eq!(
        complete_events.len(),
        1,
        "Stream should contain exactly one complete event"
    );

    // Verify complete event structure
    let complete_data = &complete_events[0].data;
    assert!(
        complete_data.get("type").is_some(),
        "Complete event should have 'type' field"
    );
    assert_eq!(
        complete_data["type"].as_str().unwrap(),
        "complete",
        "Complete event should have type='complete'"
    );
    assert!(
        complete_data.get("finish_reason").is_some(),
        "Complete event should have finish_reason field"
    );

    eprintln!("✅ SSE events have correct structure:");
    eprintln!("   - {} content events", content_events.len());
    eprintln!("   - {} complete events", complete_events.len());
}

#[tokio::test]
async fn test_title_persisted_in_database() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read", "messages::create", "llm_models::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Verify conversation has no title initially
    let before = super::helpers::get_conversation(&server, &user.token, conversation_id).await;
    assert!(
        before["title"].is_null(),
        "Conversation should not have a title initially"
    );

    // Send message to trigger title generation
    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Tell me about Paris",
    )
    .await;

    // Consume the entire SSE stream to ensure finalize() completes
    let _events = super::helpers::parse_sse_events(response).await;

    // Verify conversation now has a title
    let after = super::helpers::get_conversation(&server, &user.token, conversation_id).await;
    assert!(
        after["title"].is_string(),
        "Conversation should have a title after first exchange"
    );

    let title = after["title"].as_str().unwrap();
    assert!(
        !title.is_empty(),
        "Generated title should not be empty"
    );

    eprintln!("✅ Title persisted to database: '{}'", title);
}

// =====================================================
// Assistant Extension Tests
// =====================================================

#[tokio::test]
async fn test_assistant_extension_injects_system_message() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "assistants::create",
            "assistants::read",
        ],
    )
    .await;

    // Create an assistant with system instructions
    let assistant_payload = serde_json::json!({
        "name": "Test Assistant",
        "description": "Test assistant for SSE tests",
        "instructions": "You are a helpful assistant. Always be concise and friendly.",
        "parameters": {},
        "is_template": false,
        "enabled": true
    });

    let assistant_response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&assistant_payload)
        .send()
        .await
        .unwrap();

    assert_eq!(assistant_response.status(), reqwest::StatusCode::CREATED);
    let assistant: serde_json::Value = assistant_response.json().await.unwrap();
    let assistant_id = super::helpers::parse_uuid(&assistant["id"]);

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with assistant_id
    let payload = serde_json::json!({
        "content": "What is 2+2?",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "assistant_id": assistant_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/messages/stream",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should successfully stream (assistant extension doesn't fail on errors)
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let events = super::helpers::parse_sse_events(response).await;

    // Should have content events (meaning the message was sent successfully)
    let content_events: Vec<_> = events.iter().filter(|e| e.event == "content").collect();
    assert!(
        !content_events.is_empty(),
        "Should have content events when using assistant"
    );

    eprintln!(
        "✅ Assistant extension successfully processed request with {} content events",
        content_events.len()
    );
}

#[tokio::test]
async fn test_assistant_extension_handles_missing_assistant() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create", "llm_models::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Send message with non-existent assistant_id
    let fake_assistant_id = uuid::Uuid::new_v4();
    let payload = serde_json::json!({
        "content": "Test message",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "assistant_id": fake_assistant_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/messages/stream",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should still succeed (assistant extension logs warning but doesn't fail)
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let events = super::helpers::parse_sse_events(response).await;

    // Should still have content events
    let content_events: Vec<_> = events.iter().filter(|e| e.event == "content").collect();
    assert!(
        !content_events.is_empty(),
        "Should have content events even with missing assistant"
    );

    eprintln!("✅ Assistant extension gracefully handles missing assistant");
}
