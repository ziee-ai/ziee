//! Streaming integration tests — migrated to the fire-and-forget model.
//!
//! The old per-request SSE response (`POST /messages/stream`) is gone; the
//! reply now streams over the per-user `GET /api/chat/stream`. These tests use
//! the deterministic stub-engine-backed model (`helpers::create_stub_model`) +
//! `ChatStreamProbe`, so they run WITHOUT API keys. The raw transport-format
//! assertions (content-type, `data:` lines) moved to `chat_stream_test.rs`;
//! what remains here is the extension behaviour that rides the stream
//! (titleUpdated, assistant system-message injection) plus the invalid-model
//! guard.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;

use super::helpers;
use crate::common::chat_stream_probe::ChatStreamProbe;

const TURN_TIMEOUT: Duration = Duration::from_secs(20);

fn perms() -> &'static [&'static str] {
    &[
        "conversations::create",
        "conversations::read",
        "messages::create",
        "messages::read",
        "llm_models::read",
    ]
}

async fn setup(
    name: &str,
) -> (
    crate::common::TestServer,
    crate::common::test_helpers::TestUser,
    crate::common::stub_engine::StubEngine,
    uuid::Uuid, // model_id
) {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, name, perms()).await;
    let (stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    (server, user, stub, model_id)
}

#[tokio::test]
async fn test_invalid_model_returns_404() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", perms()).await;

    let conversation = helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    // POST /messages with a non-existent model must 404 before generation.
    let response = helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        uuid::Uuid::new_v4(),
        branch_id,
        "Error test",
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_stream_has_content_and_exactly_one_complete() {
    let (server, user, _stub, model_id) = setup("stream_user").await;
    let conversation = helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    let turn = helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "Hi").await;

    let content = turn.frames.iter().filter(|f| f.event_type == "content").count();
    assert!(content > 0, "stream should carry content frames");
    let complete = turn.frames.iter().filter(|f| f.event_type == "complete").count();
    assert_eq!(complete, 1, "stream should end on exactly one complete frame");
    assert_eq!(turn.text, "Hello from stub");
}

#[tokio::test]
async fn test_title_updated_event_on_first_message() {
    let (server, user, _stub, model_id) = setup("title_user").await;
    let conversation = helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    let turn = helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "Hi").await;

    let title_frame = turn.frames.iter().find(|f| f.event_type == "titleUpdated");
    assert!(
        title_frame.is_some(),
        "first message should emit a titleUpdated frame; got {:?}",
        turn.frames.iter().map(|f| &f.event_type).collect::<Vec<_>>()
    );
    let title = title_frame.unwrap().data["title"].as_str().unwrap_or("");
    assert!(!title.is_empty(), "generated title should not be empty");
}

#[tokio::test]
async fn test_title_not_generated_for_subsequent_messages() {
    let (server, user, _stub, model_id) = setup("title_2nd_user").await;
    let conversation = helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    let _first = helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "First").await;
    let second = helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "Second").await;

    assert!(
        second.frames.iter().all(|f| f.event_type != "titleUpdated"),
        "subsequent messages must NOT emit titleUpdated"
    );
}

#[tokio::test]
async fn test_title_persisted_in_database() {
    let (server, user, _stub, model_id) = setup("title_db_user").await;
    let conversation = helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    let before = helpers::get_conversation(&server, &user.token, conv_id).await;
    assert!(before["title"].is_null(), "no title before the first exchange");

    // The title is written synchronously in `finalize()` (before the terminal
    // frame), so once the turn completes it is already persisted.
    let _turn = helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "Tell me about Paris").await;

    let after = helpers::get_conversation(&server, &user.token, conv_id).await;
    assert!(after["title"].is_string(), "title should be persisted after first exchange");
    assert!(!after["title"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_assistant_extension_injects_system_message() {
    let (server, user, _stub, model_id) = {
        let server = crate::common::TestServer::start().await;
        let user = crate::common::test_helpers::create_user_with_permissions(
            &server,
            "assistant_user",
            &[
                "conversations::create",
                "conversations::read",
                "messages::create",
                "messages::read",
                "llm_models::read",
                "assistants::create",
                "assistants::read",
            ],
        )
        .await;
        let (stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
        let model_id = helpers::parse_uuid(&model["id"]);
        (server, user, stub, model_id)
    };

    // Create an assistant with system instructions.
    let assistant_response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Test Assistant",
            "description": "Test assistant for streaming tests",
            "instructions": "You are a helpful assistant. Be concise.",
            "parameters": {},
            "is_template": false,
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(assistant_response.status(), StatusCode::CREATED);
    let assistant: serde_json::Value = assistant_response.json().await.unwrap();
    let assistant_id = helpers::parse_uuid(&assistant["id"]);

    let conversation = helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send with assistant_id; the reply still streams to completion.
    let content = send_with_assistant(&server, &user.token, conv_id, branch_id, model_id, Some(assistant_id), "What is 2+2?").await;
    assert!(content > 0, "assistant-driven turn should carry content frames");
}

#[tokio::test]
async fn test_assistant_extension_handles_missing_assistant() {
    let (server, user, _stub, model_id) = setup("missing_assistant_user").await;
    let conversation = helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conversation["id"]);
    let branch_id = helpers::parse_uuid(&conversation["active_branch_id"]);

    // A non-existent assistant_id must not fail the turn (extension logs + skips).
    let content = send_with_assistant(&server, &user.token, conv_id, branch_id, model_id, Some(uuid::Uuid::new_v4()), "Test").await;
    assert!(content > 0, "missing assistant should still produce a reply");
}

/// Subscribe → POST `/messages` with an optional `assistant_id` → collect until
/// terminal; return the number of content frames seen.
async fn send_with_assistant(
    server: &crate::common::TestServer,
    token: &str,
    conv_id: uuid::Uuid,
    branch_id: uuid::Uuid,
    model_id: uuid::Uuid,
    assistant_id: Option<uuid::Uuid>,
    content: &str,
) -> usize {
    let mut probe = ChatStreamProbe::open(server, token).await;
    probe.subscribe(Some(conv_id)).await;

    let mut body = json!({
        "content": content,
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
    });
    if let Some(a) = assistant_id {
        body["assistant_id"] = json!(a.to_string());
    }

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages", conv_id)))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "send with assistant should be 200");

    let frames = probe.collect_until_terminal(conv_id, TURN_TIMEOUT).await;
    frames.iter().filter(|f| f.event_type == "content").count()
}
