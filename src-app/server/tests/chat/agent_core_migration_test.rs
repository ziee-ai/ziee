//! Behavioral verification of the chat→agent-core re-home (ITEM-24/25/26).
//!
//! Drives a REAL chat turn through the full production path with the
//! `ZIEE_CHAT_AGENT_CORE=1` cutover flag set, so the send goes:
//! handler → `start_generation_agent_core` → `ChatAgentTurn` → `AgentCore::run`
//! → `ProviderModelClient` (stub engine, real HTTP+SSE) → `ChatEventSink` →
//! `/api/chat/stream`. Asserts the reply STREAMS and the assistant message is
//! PERSISTED as blocks — proving the ports + host reproduce chat's behavior.
//!
//! Uses the deterministic stub engine (not the bridge) so it runs anywhere; the
//! agent loop itself is separately verified against the real Qwen bridge in
//! `agent-core/tests/real_llm_loop.rs`.
//!
//! RUN ISOLATED: it sets a process-global env flag, so run it alone
//! (`cargo test --test integration_tests chat::agent_core_migration -- --test-threads=1`).

use crate::chat::helpers;
use crate::common::test_helpers;

/// A basic (no-tool) assistant turn on the agent-core path streams a non-empty
/// reply and persists the assistant message.
#[tokio::test]
async fn agent_core_stub_chat_streams_and_persists() {
    // Route the chat send through the shared agent-core loop. The server subprocess
    // inherits this at spawn; run this test isolated so no sibling server sees it.
    unsafe { std::env::set_var("ZIEE_CHAT_AGENT_CORE", "1") };

    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "agent_core_chat_user",
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

    let conv = helpers::create_conversation(&server, &user.token, Some(model_id), Some("ac")).await;
    let conv_id = helpers::parse_uuid(&conv["id"]);
    let branch_id = helpers::parse_uuid(&conv["active_branch_id"]);

    let turn = helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "Hello from the agent-core migration smoke test.",
    )
    .await;

    // 1. The reply streamed over SSE (Content frames assembled to non-empty text).
    assert!(
        !turn.text.trim().is_empty(),
        "agent-core turn produced no streamed assistant text; frames: {:?}",
        turn.frames
    );

    // 2. The assistant message was persisted (block-aware transcript) and reads back
    //    from the (keyset-paginated) history endpoint.
    let history = helpers::get_conversation_history(&server, &user.token, conv_id).await;
    let msgs = history.as_array().expect("history messages array");
    let assistant_id = turn.assistant_message_id.to_string();
    let persisted = msgs.iter().find(|m| m["id"].as_str() == Some(assistant_id.as_str()));
    assert!(
        persisted.is_some(),
        "assistant message {assistant_id} not persisted; history: {history}"
    );

    // 3. The user message was also persisted (pre-loop message lifecycle).
    assert!(
        msgs.iter().any(|m| m["role"].as_str() == Some("user")),
        "user message not persisted on the agent-core path"
    );

    unsafe { std::env::remove_var("ZIEE_CHAT_AGENT_CORE") };
}
