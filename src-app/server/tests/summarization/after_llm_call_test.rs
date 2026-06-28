//! `after_llm_call` background-spawn behavior (summarization chat extension).
//!
//! The extension's `after_llm_call` returns `ExtensionAction::Complete`
//! immediately and does the summary refresh in a detached `tokio::spawn`, so
//! the chat hot path is never blocked. Inside that spawn a cheap guard skips
//! brand-new branches: `if history_count < 4 { return }`.
//!
//! This drives the REAL chat stream (via the in-process stub chat model — the
//! legitimate LLM boundary) with summarization ENABLED, sends a single turn
//! (the branch then holds 2 messages, < 4), and asserts NO summary is produced
//! — proving the background hook ran and the short-branch guard held. The
//! positive (≥-threshold → summary written) path needs a real LLM and is
//! covered by `real_llm_test::r4/r5`.

use std::time::Duration;

use serde_json::{json, Value};

use crate::chat::helpers::{create_conversation, create_stub_model, parse_uuid, send_and_collect};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

#[tokio::test]
async fn after_llm_call_skips_summary_for_brand_new_short_branch() {
    let server = TestServer::start().await;

    // Enable summarization deployment-wide so the `after_llm_call` hook gets
    // past its enabled-check and reaches the history-count guard.
    let admin = create_user_with_permissions(
        &server,
        "summ_guard_admin",
        &["summarization::settings::manage"],
    )
    .await;
    let enable = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert!(
        enable.status().is_success(),
        "enable summarization → {}",
        enable.status()
    );

    // A user who can chat + read the conversation summary.
    let user = create_user_with_permissions(
        &server,
        "summ_guard_user",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    // Stub chat model (no real LLM) + a conversation with a preset title (so the
    // title-generation extension doesn't fire its own provider call).
    let (_stub, model) = create_stub_model(&server, &user.user_id).await;
    let model_id = parse_uuid(&model["id"]);
    let conversation =
        create_conversation(&server, &user.token, Some(model_id), Some("preset")).await;
    let conv_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    // One real turn → the branch now holds exactly 2 messages (user + assistant),
    // which is < 4.
    let _turn = send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "hi").await;

    // The hook's refresh runs in a detached spawn; give it time to execute the
    // guard. The guard returns BEFORE any summarizer model call, so this is a
    // generous-but-bounded wait, not a flaky LLM dependency.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let summary = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/summary")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(summary.status(), 200);
    let body: Value = summary.json().await.unwrap();
    assert!(
        body.is_null(),
        "a brand-new short branch (< 4 messages) must NOT be summarized; got {body}"
    );
}
