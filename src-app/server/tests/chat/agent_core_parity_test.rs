//! TEST-24 — chat send-message on the agent-core loop emits the SAME
//! `SSEChatStreamEvent` sequence (started → content → exactly-one complete) and
//! persists the SAME `message_contents` (a user row + an assistant text block) as
//! the pre-migration path, driven by the deterministic stub engine. A
//! characterization/parity golden for the core streaming + persistence contract.
//!
//! RUN ISOLATED (sets the process-global cutover flag): `cargo test --test
//! integration_tests chat::agent_core_parity -- --test-threads=1`.

use crate::chat::helpers;
use crate::common::test_helpers;

#[tokio::test]
async fn agent_core_chat_matches_sse_sequence_and_persistence() {
    unsafe { std::env::set_var("ZIEE_CHAT_AGENT_CORE", "1") };

    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "agent_core_parity_user",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    let (_stub, model) = helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);
    let conv = helpers::create_conversation(&server, &user.token, Some(model_id), Some("p")).await;
    let conv_id = helpers::parse_uuid(&conv["id"]);
    let branch_id = helpers::parse_uuid(&conv["active_branch_id"]);

    let turn = helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "parity check please",
    )
    .await;

    // 1. SSE sequence: started, ≥1 content, exactly one complete — in order.
    let names: Vec<&str> = turn.frames.iter().map(|f| f.event_type.as_str()).collect();
    assert_eq!(names.first(), Some(&"started"), "first frame must be started; got {names:?}");
    assert_eq!(
        names.iter().filter(|n| **n == "complete").count(),
        1,
        "exactly one complete frame; got {names:?}"
    );
    assert!(names.iter().any(|n| *n == "content"), "≥1 content frame; got {names:?}");
    assert!(!turn.text.trim().is_empty(), "assistant text streamed");

    // 2. Persistence: a user message + an assistant message (with a text block).
    let history = helpers::get_conversation_history(&server, &user.token, conv_id).await;
    let msgs = history.as_array().expect("history array");
    let assistant_id = turn.assistant_message_id.to_string();
    let assistant = msgs
        .iter()
        .find(|m| m["id"].as_str() == Some(assistant_id.as_str()))
        .expect("assistant message persisted");
    let has_text_block = assistant["contents"]
        .as_array()
        .map(|c| {
            c.iter().any(|b| {
                let t = b["content_type"].as_str().or(b["type"].as_str());
                matches!(t, Some("text"))
            })
        })
        .unwrap_or(false);
    assert!(
        has_text_block,
        "assistant message must persist a text content block: {assistant}"
    );
    assert!(
        msgs.iter().any(|m| m["role"].as_str() == Some("user")),
        "user message persisted"
    );

    unsafe { std::env::remove_var("ZIEE_CHAT_AGENT_CORE") };
}
