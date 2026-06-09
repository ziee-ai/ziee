//! Integration tests for the per-user chat-token stream + fire-and-forget send.
//!
//! These drive a DETERMINISTIC generation via a stub-engine-backed `custom`
//! provider (`helpers::create_stub_model`), so the full production path runs —
//! `POST /messages` → detached `start_generation` → `publish_frame` → the
//! per-user `GET /api/chat/stream` (scoped via `PUT /stream/subscription`) — and
//! we assert on real frames, not the routing logic in isolation (that is
//! covered by the in-source unit tests in `modules/chat/stream/registry.rs`).
//!
//! The stub replies `"Hello from stub"`; `create_stub_model_with_delay` paces
//! the deltas so a turn can be cancelled mid-flight.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value;
use uuid::Uuid;

use super::helpers;
use crate::common::chat_stream_probe::ChatStreamProbe;
use crate::common::test_helpers::TestUser;

const TURN_TIMEOUT: Duration = Duration::from_secs(20);

async fn chat_user(server: &crate::common::TestServer, name: &str) -> TestUser {
    crate::common::test_helpers::create_user_with_permissions(
        server,
        name,
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await
}

/// Create a conversation already bound to `model_id`; return (conv_id, branch_id).
async fn new_conversation(
    server: &crate::common::TestServer,
    token: &str,
    model_id: Uuid,
) -> (Uuid, Uuid) {
    let conversation = helpers::create_conversation(server, token, Some(model_id), None).await;
    (
        helpers::parse_uuid(&conversation["id"]),
        helpers::parse_uuid(&conversation["active_branch_id"]),
    )
}

#[tokio::test]
async fn send_returns_ids_and_streams_reply_to_subscriber() {
    let server = crate::common::TestServer::start().await;
    let user = chat_user(&server, "stream_user").await;
    let (_stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    let (conv_id, branch_id) = new_conversation(&server, &user.token, model_id).await;

    let turn =
        helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "Hi").await;

    // The POST returned real ids immediately…
    assert!(turn.user_message_id.is_some(), "user message id missing");
    // …and the reply streamed over the chat-token stream to completion.
    assert_eq!(turn.text, "Hello from stub", "assembled reply mismatch");
    assert!(
        turn.frames.iter().any(|f| f.event_type == "started"),
        "missing started frame"
    );
    assert!(
        turn.frames
            .last()
            .is_some_and(|f| f.event_type == "complete"),
        "stream did not end on complete"
    );

    // And the turn persisted: the assistant message is readable from history.
    let history =
        helpers::get_conversation_history(&server, &user.token, conv_id).await;
    let msgs = history.as_array().expect("history array");
    assert!(
        msgs.iter()
            .any(|m| m["id"].as_str() == Some(turn.assistant_message_id.to_string().as_str())),
        "assistant message not persisted"
    );
}

#[tokio::test]
async fn subscriber_on_other_conversation_receives_nothing() {
    let server = crate::common::TestServer::start().await;
    let user = chat_user(&server, "scope_user").await;
    let (_stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);

    let (conv_a, branch_a) = new_conversation(&server, &user.token, model_id).await;
    let (conv_b, _branch_b) = new_conversation(&server, &user.token, model_id).await;

    // Positive control: a probe subscribed to A must see A's turn.
    let mut probe_a = ChatStreamProbe::open(&server, &user.token).await;
    probe_a.subscribe(Some(conv_a)).await;
    // A second device viewing B must NOT see A's frames.
    let mut probe_b = ChatStreamProbe::open(&server, &user.token).await;
    probe_b.subscribe(Some(conv_b)).await;

    helpers::send_message_simple(&server, &user.token, conv_a, model_id, branch_a, "Hi").await;

    // A streamed to completion…
    probe_a.collect_until_terminal(conv_a, TURN_TIMEOUT).await;
    // …while the B-scoped connection stayed silent (server-side scoping).
    probe_b.expect_silence(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn unsubscribed_connection_receives_nothing() {
    let server = crate::common::TestServer::start().await;
    let user = chat_user(&server, "unsub_user").await;
    let (_stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    let (conv_id, branch_id) = new_conversation(&server, &user.token, model_id).await;

    // Positive control subscribed to the conversation.
    let mut probe_sub = ChatStreamProbe::open(&server, &user.token).await;
    probe_sub.subscribe(Some(conv_id)).await;
    // A connection that never subscribed must receive nothing (no broadcast).
    let mut probe_none = ChatStreamProbe::open(&server, &user.token).await;

    helpers::send_message_simple(&server, &user.token, conv_id, model_id, branch_id, "Hi").await;

    probe_sub.collect_until_terminal(conv_id, TURN_TIMEOUT).await;
    probe_none.expect_silence(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn cross_user_isolation_no_frame_leak() {
    let server = crate::common::TestServer::start().await;
    let user_a = chat_user(&server, "iso_a").await;
    let user_b = chat_user(&server, "iso_b").await;
    let (_stub, model) = helpers::create_stub_model(&server, &user_a.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    let (conv_a, branch_a) = new_conversation(&server, &user_a.token, model_id).await;

    // User B opens a stream. He cannot subscribe to A's conversation (the PUT
    // verifies ownership), so he simply listens — and must never see A's turn.
    let mut probe_b = ChatStreamProbe::open(&server, &user_b.token).await;
    // Positive control for A.
    let mut probe_a = ChatStreamProbe::open(&server, &user_a.token).await;
    probe_a.subscribe(Some(conv_a)).await;

    helpers::send_message_simple(&server, &user_a.token, conv_a, model_id, branch_a, "Hi").await;

    probe_a.collect_until_terminal(conv_a, TURN_TIMEOUT).await;
    probe_b.expect_silence(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn stop_without_active_generation_returns_409() {
    let server = crate::common::TestServer::start().await;
    let user = chat_user(&server, "stop_409_user").await;
    let (_stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    let (conv_id, branch_id) = new_conversation(&server, &user.token, model_id).await;

    // Send and let the (fast) turn finish, so the assistant message exists but
    // is no longer generating.
    let turn =
        helpers::send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "Hi").await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/messages/{}/stop",
            conv_id, turn.assistant_message_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "stopping a finished turn should be 409 NO_ACTIVE_GENERATION"
    );
}

#[tokio::test]
async fn stop_cancels_in_flight_generation() {
    let server = crate::common::TestServer::start().await;
    let user = chat_user(&server, "stop_cancel_user").await;
    // 1s between deltas: "Hello" arrives ~1s in, with a 1s window before the
    // natural completion — ample room to cancel event-driven (not time-based).
    let (_stub, model) = helpers::create_stub_model_with_delay(&server, &user.user_id, 1_000).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    let (conv_id, branch_id) = new_conversation(&server, &user.token, model_id).await;

    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv_id)).await;

    let resp =
        helpers::send_message_simple(&server, &user.token, conv_id, model_id, branch_id, "Hi").await;
    let body: Value = resp.json().await.unwrap();
    let assistant_id = helpers::parse_uuid(&body["assistant_message_id"]);

    // Wait until the first content delta has streamed (generation is in flight).
    probe
        .expect_event(conv_id, "content", Duration::from_secs(10))
        .await;

    // Stop it.
    let stop = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/messages/{}/stop",
            conv_id, assistant_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(stop.status(), StatusCode::NO_CONTENT);

    // The stream delivers a terminal complete marked cancelled.
    let frames = probe.collect_until_terminal(conv_id, Duration::from_secs(10)).await;
    let terminal = frames.last().expect("a terminal frame");
    assert_eq!(terminal.event_type, "complete");
    assert_eq!(
        terminal.data["finish_reason"].as_str(),
        Some("cancelled"),
        "cancelled turn should complete with finish_reason=cancelled"
    );
}
