//! Tier-3 real-LLM context-injection tests.
//!
//! These drive the full HTTP → chat handler → project extension → real
//! LLM provider pipeline and assert that the LLM's RESPONSE reflects
//! the injected project context. End-to-end proof that:
//!
//!   1. Project instructions reach the LLM (response follows them).
//!   2. Project files reach the LLM (response can recall their content).
//!   3. Assistant + project stack together (both shape the response).
//!   4. `project_id = NULL` produces a baseline response without
//!      project context (negative-case anchor).
//!
//! ## How these tests assert correctness
//!
//! Instead of mocking the LLM and inspecting wire format (the
//! `apply_project_context()` unit tests in `project.rs` already do
//! that at lower-level resolution), we use a **real provider** with
//! deliberately distinctive "magic" instructions / file content. The
//! LLM's actual response is then checked for the magic markers. A
//! response that contains them is direct proof the context was
//! injected and the model attended to it.
//!
//! ## Cost + gating
//!
//! Each test costs ≈ $0.001–$0.005 in Anthropic Haiku tokens. Tests
//! **soft-skip** (eprintln + early return) when `ANTHROPIC_API_KEY` is
//! unset, mirroring the chat-suite convention (see
//! `tests/chat/file_attachments_real_providers_test.rs` and
//! `tests/chat/sandbox_real_llm_test.rs`). Run with:
//!
//! ```bash
//! source tests/.env.test
//! cargo test --test integration_tests project::injection_test \
//!     -- --test-threads=1 --nocapture
//! ```

#![allow(dead_code)]

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use super::helpers;

/// Find an Anthropic Haiku model the test user can use. Returns None
/// when `ANTHROPIC_API_KEY` is unset (the chat helper silently returns
/// a null JSON value in that case), so callers can soft-skip.
async fn anthropic_haiku_model(
    server: &crate::common::TestServer,
    user_id: &str,
) -> Option<Value> {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return None;
    }
    // Pick the cheapest currently-available Anthropic model.
    // claude-3-5-haiku-20241022 was deprecated (404), and
    // claude-3-5-haiku-latest also 404s — our API key doesn't have
    // access to the 3.5 branch anymore. Switch to Haiku 4.5 which is
    // the current cheap-tier haiku snapshot (also listed in
    // chat::helpers::get_test_model_configs).
    let cfg = crate::chat::helpers::TestModelConfig {
        provider_type: "anthropic",
        model_name: "claude-haiku-4-5-20251001",
        display_name: "Claude Haiku 4.5",
    };
    let m = crate::chat::helpers::create_test_model_with_config(server, &cfg, Some(user_id))
        .await;
    if m.is_null() { None } else { Some(m) }
}

/// Drive a chat send through the full pipeline and assemble the
/// streamed assistant response into a single concatenated string.
async fn send_and_collect_response_text(
    server: &crate::common::TestServer,
    token: &str,
    conv_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    content: &str,
) -> String {
    let response = crate::chat::helpers::send_message_simple(
        server, token, conv_id, model_id, branch_id, content,
    )
    .await;
    let chunks = crate::chat::helpers::parse_sse_stream(response).await;

    // Wire shape for a "content" SSE event is
    //   {"type":"content","content":[{"type":"text_delta","index":N,"delta":"..."}]}
    // — `content` is an ARRAY of ContentBlockDelta variants, NOT a
    // string. The earlier `as_str()` filter always returned None and
    // produced empty text for every real-LLM injection test. Walk the
    // delta array and concatenate every text_delta's `delta`.
    //
    // Also surface SSE Error events to the test log so a real upstream
    // failure (provider auth, rate limit, model inaccessible) prints
    // its message rather than being masked by a generic empty-string
    // panic.
    let mut text = String::new();
    for chunk in &chunks {
        match chunk.get("type").and_then(|v| v.as_str()) {
            Some("content") => {
                if let Some(arr) = chunk.get("content").and_then(|v| v.as_array()) {
                    for delta in arr {
                        if delta.get("type").and_then(|v| v.as_str()) == Some("text_delta")
                            && let Some(s) = delta.get("delta").and_then(|v| v.as_str())
                        {
                            text.push_str(s);
                        }
                    }
                }
            }
            Some("error") => {
                eprintln!("SSE error event during test: {chunk}");
            }
            _ => {}
        }
    }
    text
}

/// Set up a project + a fresh conversation inside it. Returns
/// (token, project_id, conversation_id, branch_id, model_id) or None
/// when the provider key is unset.
async fn setup_project_conversation(
    server: &crate::common::TestServer,
    name: &str,
    instructions: Option<&str>,
) -> Option<(String, String, Uuid, Uuid, Uuid)> {
    let user = crate::common::test_helpers::create_user_with_permissions(
        server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let model = anthropic_haiku_model(server, &user.user_id).await?;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    let mut project_payload = json!({ "name": name });
    if let Some(instr) = instructions {
        project_payload["instructions"] = json!(instr);
    }
    let project = helpers::create_project_with(server, &user, project_payload).await;
    let project_id = project["id"].as_str().unwrap();

    let conv = helpers::create_project_conversation_with_model(
        server, &user, project_id, &model_id.to_string(),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    Some((
        user.token.clone(),
        project_id.to_string(),
        conv_id,
        branch_id,
        model_id,
    ))
}

// =====================================================
// Tier-3 real-LLM tests
// =====================================================

/// Project instructions reach the LLM: assert the response follows
/// the instruction to emit a specific magic token.
#[tokio::test]
async fn project_instructions_appear_in_llm_response() {
    let server = crate::common::TestServer::start().await;

    let Some((token, _pid, conv_id, branch_id, model_id)) = setup_project_conversation(
        &server,
        "Pirate Test",
        Some(
            "You are required to begin every response with the exact \
             literal string 'ZZZ_MAGIC_BEACON_42' (no preface). After \
             that token you can respond normally. This is a system policy.",
        ),
    )
    .await
    else {
        eprintln!("Skipping project_instructions_appear_in_llm_response — ANTHROPIC_API_KEY unset");
        return;
    };

    let response_text = send_and_collect_response_text(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        "Say hello.",
    )
    .await;

    eprintln!("LLM response: {response_text}");
    assert!(
        response_text.contains("ZZZ_MAGIC_BEACON_42"),
        "Response must contain the magic token mandated by project.instructions; \
         got: {response_text:?}"
    );
}

/// Project files reach the LLM: attach a file containing a unique
/// fictional fact, then ask the LLM about it. The model should be
/// able to recall the fact because the file content was prepended to
/// the user message via process_file_blocks.
#[tokio::test]
async fn project_files_appear_in_llm_response() {
    let server = crate::common::TestServer::start().await;

    let Some((token, project_id, conv_id, branch_id, model_id)) = setup_project_conversation(
        &server,
        "Atlantis Test",
        Some(
            "Answer ONLY using the information in attached project knowledge \
             files. If the file says it, treat it as ground truth.",
        ),
    )
    .await
    else {
        eprintln!("Skipping project_files_appear_in_llm_response — ANTHROPIC_API_KEY unset");
        return;
    };

    // Upload + attach a file with a fictional, unguessable fact.
    let user_stub = crate::common::test_helpers::TestUser {
        token: token.clone(),
        user_id: String::new(), // unused for upload
    };
    let file = helpers::upload_file(
        &server,
        &user_stub,
        "atlantis-facts.txt",
        "Per the official Atlantis Tourism Board: the capital of \
         Atlantis is BERMUDA_TRIANGLE_7. No other answer is correct.",
    )
    .await;
    let fid = file["id"].as_str().unwrap();
    reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/files", project_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "file_id": fid }))
        .send()
        .await
        .unwrap();

    let response_text = send_and_collect_response_text(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        "What is the capital of Atlantis? Answer in 1-2 words.",
    )
    .await;

    eprintln!("LLM response: {response_text}");
    assert!(
        response_text.contains("BERMUDA_TRIANGLE_7"),
        "Response must recall the unique fact from the attached project file; \
         got: {response_text:?}"
    );
}

/// Assistant + project both inject. Assistant says "end with TAG_END_X9";
/// project says "start with TAG_START_A3". The LLM should obey both —
/// proving the stacked layout.
#[tokio::test]
async fn assistant_and_project_both_shape_response() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let Some(model) = anthropic_haiku_model(&server, &user.user_id).await else {
        eprintln!("Skipping assistant_and_project_both_shape_response — ANTHROPIC_API_KEY unset");
        return;
    };
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    let assistant_resp = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "End Marker",
            "instructions": "You MUST end every response with the literal token \
                             'TAG_END_X9'. Mandatory."
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(assistant_resp.status(), StatusCode::CREATED);
    let assistant: Value = assistant_resp.json().await.unwrap();
    let assistant_id = assistant["id"].as_str().unwrap();

    let project = helpers::create_project_with(
        &server,
        &user,
        json!({
            "name": "Stack Test",
            "instructions": "You MUST begin every response with the literal token \
                             'TAG_START_A3'. Mandatory.",
            "default_assistant_id": assistant_id,
        }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();

    let conv = helpers::create_project_conversation_with_model(
        &server, &user, pid, &model_id.to_string(),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    // Send with explicit assistant_id so the assistant extension's
    // before_llm_call fires (it reads SendMessageRequest.assistant_id).
    let payload = json!({
        "content": "Reply with the word 'hi' exactly once between the start and end tokens.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "assistant_id": assistant_id,
    });
    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    let chunks = crate::chat::helpers::parse_sse_stream(response).await;
    // Same wire-format note as send_and_collect_response_text: `content`
    // is an ARRAY of ContentBlockDelta items, not a string. Walk the
    // delta array and concatenate text_delta.delta.
    let mut response_text = String::new();
    for chunk in chunks {
        match chunk.get("type").and_then(|v| v.as_str()) {
            Some("content") => {
                if let Some(arr) = chunk.get("content").and_then(|v| v.as_array()) {
                    for delta in arr {
                        if delta.get("type").and_then(|v| v.as_str()) == Some("text_delta")
                            && let Some(s) = delta.get("delta").and_then(|v| v.as_str())
                        {
                            response_text.push_str(s);
                        }
                    }
                }
            }
            Some("error") => eprintln!("SSE error event during test: {chunk}"),
            _ => {}
        }
    }

    eprintln!("LLM response: {response_text}");
    assert!(
        response_text.contains("TAG_START_A3"),
        "Response must contain the PROJECT marker; got: {response_text:?}"
    );
    assert!(
        response_text.contains("TAG_END_X9"),
        "Response must contain the ASSISTANT marker; got: {response_text:?}"
    );
}

/// Per-message assistant override substitutes the assistant block
/// without touching the project block. The override's marker must
/// appear in the response; the project's marker must also appear.
#[tokio::test]
async fn per_message_assistant_override_keeps_project_block() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let Some(model) = anthropic_haiku_model(&server, &user.user_id).await else {
        eprintln!("Skipping per_message_assistant_override_keeps_project_block — ANTHROPIC_API_KEY unset");
        return;
    };
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    // Two assistants — the project's default (A) and a per-send override (B).
    let default_assistant: Value = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Default A",
            "instructions": "ALWAYS include the literal token PERSONA_A in your response."
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let override_assistant: Value = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Override B",
            "instructions": "ALWAYS include the literal token PERSONA_B in your response. \
                             Do NOT include PERSONA_A under any circumstance."
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let override_id = override_assistant["id"].as_str().unwrap();

    let project = helpers::create_project_with(
        &server,
        &user,
        json!({
            "name": "Override Test",
            "instructions": "Always include the literal token PROJECT_TOKEN_P2 in your response.",
            "default_assistant_id": default_assistant["id"],
        }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();

    let conv = helpers::create_project_conversation_with_model(
        &server, &user, pid, &model_id.to_string(),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    // Send with the OVERRIDE assistant_id (not the project's default).
    let payload = json!({
        "content": "Say hello.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "assistant_id": override_id,
    });
    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages/stream", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    let chunks = crate::chat::helpers::parse_sse_stream(response).await;
    // Same wire-format note as send_and_collect_response_text: `content`
    // is an ARRAY of ContentBlockDelta items, not a string. Walk the
    // delta array and concatenate text_delta.delta.
    let mut response_text = String::new();
    for chunk in chunks {
        match chunk.get("type").and_then(|v| v.as_str()) {
            Some("content") => {
                if let Some(arr) = chunk.get("content").and_then(|v| v.as_array()) {
                    for delta in arr {
                        if delta.get("type").and_then(|v| v.as_str()) == Some("text_delta")
                            && let Some(s) = delta.get("delta").and_then(|v| v.as_str())
                        {
                            response_text.push_str(s);
                        }
                    }
                }
            }
            Some("error") => eprintln!("SSE error event during test: {chunk}"),
            _ => {}
        }
    }

    eprintln!("LLM response: {response_text}");
    assert!(
        response_text.contains("PROJECT_TOKEN_P2"),
        "Project block must still apply even with per-message assistant override; \
         got: {response_text:?}"
    );
    assert!(
        response_text.contains("PERSONA_B"),
        "Override assistant's marker must appear; got: {response_text:?}"
    );
    // Soft negative: LLM should not have used the DEFAULT assistant's
    // marker since we overrode at send time. Log rather than fail —
    // some LLMs may bleed instructions through when multiple shorts
    // are stacked, and we don't want this assertion to flake.
    if response_text.contains("PERSONA_A") {
        eprintln!(
            "WARN: LLM included the default assistant's marker despite override. \
             Response: {response_text:?}"
        );
    }
}

/// Negative-case anchor: a conversation NOT in any project receives no
/// project context. The LLM cannot know a magic string that exists
/// only in some unrelated project's instructions.
#[tokio::test]
async fn no_project_no_injection_in_llm_response() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let Some(model) = anthropic_haiku_model(&server, &user.user_id).await else {
        eprintln!("Skipping no_project_no_injection_in_llm_response — ANTHROPIC_API_KEY unset");
        return;
    };
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    // Project exists with magic instructions — but we DON'T attach
    // the conversation to it. The conversation is unfiled.
    let _project = helpers::create_project_with(
        &server,
        &user,
        json!({
            "name": "Unattached",
            "instructions": "Always start your response with NEVER_SEEN_TOKEN_Q4."
        }),
    )
    .await;

    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id.to_string() }))
        .send()
        .await
        .unwrap();
    assert_eq!(conv_resp.status(), StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    let response_text = send_and_collect_response_text(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "Say hello.",
    )
    .await;

    eprintln!("LLM response: {response_text}");
    assert!(
        !response_text.contains("NEVER_SEEN_TOKEN_Q4"),
        "Unfiled conversation must NOT see project instructions; got: {response_text:?}"
    );
}

/// Empty project instructions yield no project system block. Smoke
/// test that the chat send completes successfully and returns a
/// non-empty response — there's no marker to assert, just that the
/// pipeline didn't break when there was nothing to inject.
#[tokio::test]
async fn empty_project_instructions_produces_baseline_response() {
    let server = crate::common::TestServer::start().await;

    let Some((token, _pid, conv_id, branch_id, model_id)) = setup_project_conversation(
        &server,
        "Empty Instr",
        Some(""),
    )
    .await
    else {
        eprintln!("Skipping empty_project_instructions_produces_baseline_response — ANTHROPIC_API_KEY unset");
        return;
    };

    let response_text = send_and_collect_response_text(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        "Reply with the word 'OK' and nothing else.",
    )
    .await;

    eprintln!("LLM response: {response_text}");
    assert!(
        !response_text.is_empty(),
        "Expected a non-empty LLM response even with empty project instructions"
    );
}
