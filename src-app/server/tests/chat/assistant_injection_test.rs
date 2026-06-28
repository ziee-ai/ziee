//! Integration test: the assistant chat-extension injects an assistant's
//! `instructions` into the LLM request as a labeled system message
//! (`assistant/chat_extension/assistant.rs::before_llm_call`).
//!
//! The audit (`r2-1972977b346a`) flagged "chat+assistant integration test
//! missing": `injection_test.rs` covers assistant+PROJECT *stacking* via a real
//! LLM, and `streaming_test.rs::send_with_assistant` only asserts a reply is
//! produced — neither proves the assistant's instruction TEXT actually reaches
//! the model request, nor the security scoping. This drives the REAL chat
//! consumer path (build → extensions → `OpenAIProvider` → stub) with no API key
//! and asserts on the exact request body the provider produced, via the
//! capturing `oai_capture_stub::StubChat`.

use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::helpers::{self, create_conversation, parse_uuid};
use crate::common::chat_stream_probe::ChatStreamProbe;
use crate::common::oai_capture_stub::{StubChat, StubPlan};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

const TURN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn chat_perms() -> &'static [&'static str] {
    &[
        "conversations::create",
        "conversations::read",
        "messages::create",
        "messages::read",
        "llm_models::read",
        "assistants::create",
        "assistants::read",
    ]
}

/// Register a `custom` provider pointing at the capturing stub + a chat model,
/// and grant `user_id` access. Mirrors `stub_chat_tier2_test::create_model`.
async fn create_stub_model(server: &TestServer, user_id: &str, base_url: &str) -> Value {
    let admin = create_user_with_permissions(
        server,
        "assistant_inj_admin",
        &[
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    let provider: Value = client
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("StubInj {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom",
            "enabled": true,
            "api_key": "test",
            "base_url": base_url,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let model: Value = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider["id"],
            "name": "stub-assistant-inj",
            "display_name": "Stub Assistant Injection Model",
            "description": "assistant-injection integration model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "embedding": false }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}

/// Create a (non-template) user assistant with `instructions`, returning its id.
async fn create_assistant_with_instructions(
    server: &TestServer,
    token: &str,
    name: &str,
    instructions: &str,
) -> Uuid {
    let resp = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "description": "injection test assistant",
            "instructions": instructions,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "assistant create failed");
    let body: Value = resp.json().await.unwrap();
    parse_uuid(&body["id"])
}

/// Subscribe → POST `/messages` (optionally carrying `assistant_id`) → wait for
/// the turn to terminate so the stub has captured its request.
async fn send_with_assistant(
    server: &TestServer,
    token: &str,
    conv_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    assistant_id: Option<Uuid>,
) {
    let mut probe = ChatStreamProbe::open(server, token).await;
    probe.subscribe(Some(conv_id)).await;

    let mut body = json!({
        "content": "hello",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
    });
    if let Some(a) = assistant_id {
        body["assistant_id"] = json!(a.to_string());
    }

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "send failed");

    let _ = probe.collect_until_terminal(conv_id, TURN_TIMEOUT).await;
}

/// Concatenated text of every `system`-role message in the captured request.
fn system_text(request: &Value) -> String {
    let empty = vec![];
    request
        .get("messages")
        .and_then(|m| m.as_array())
        .unwrap_or(&empty)
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .map(|m| match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The assistant's own `instructions` reach the LLM request as a labeled
/// system message (the production injection path, asserted on the wire).
#[tokio::test]
async fn assistant_instructions_are_injected_into_the_llm_request() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "assistant_inj_user", chat_perms()).await;
    let stub = StubChat::start(StubPlan::text("done")).await;
    let model = create_stub_model(&server, &user.user_id, &stub.base_url()).await;
    let model_id = parse_uuid(&model["id"]);

    let beacon = "ZZZ_ASSISTANT_BEACON_42 always answer in exactly one short sentence";
    let assistant_id =
        create_assistant_with_instructions(&server, &user.token, "Beacon Bot", beacon).await;

    // Preset the title so the title-generation extension makes no extra provider
    // call — the stub then captures exactly the reply request.
    let conversation =
        create_conversation(&server, &user.token, Some(model_id), Some("preset")).await;
    let conv_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    send_with_assistant(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        Some(assistant_id),
    )
    .await;

    assert_eq!(stub.request_count(), 1, "expected exactly one reply request");
    let sys = system_text(&stub.last_request());
    assert!(
        sys.contains(beacon),
        "assistant instructions must reach the LLM request; system text was: {sys}"
    );
    assert!(
        sys.contains("Assistant template instructions"),
        "instructions must be wrapped in the labeled system-policy delimiter; got: {sys}"
    );

    drop(server);
}

/// SECURITY: a user passing ANOTHER user's private (non-template) assistant_id
/// must NOT get that assistant's instructions injected — `get_for_user` scopes
/// to the caller's own assistants + public templates, so the beacon never
/// reaches the request (the turn still completes normally).
#[tokio::test]
async fn another_users_private_assistant_is_not_injected() {
    let server = TestServer::start().await;
    let owner = create_user_with_permissions(&server, "assistant_inj_owner", chat_perms()).await;
    let intruder =
        create_user_with_permissions(&server, "assistant_inj_intruder", chat_perms()).await;
    let stub = StubChat::start(StubPlan::text("done")).await;
    let model = create_stub_model(&server, &owner.user_id, &stub.base_url()).await;
    let model_id = parse_uuid(&model["id"]);
    // The intruder needs access to the same stub model to send at all.
    helpers::ensure_user_has_model_access(&server, &intruder.user_id, &model).await;

    let secret = "ZZZ_PRIVATE_BEACON_owner_only_99";
    let owner_assistant =
        create_assistant_with_instructions(&server, &owner.token, "Owner Bot", secret).await;

    // The intruder owns this conversation and tries to inject the owner's
    // private assistant by id.
    let conversation =
        create_conversation(&server, &intruder.token, Some(model_id), Some("preset")).await;
    let conv_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    send_with_assistant(
        &server,
        &intruder.token,
        conv_id,
        branch_id,
        model_id,
        Some(owner_assistant),
    )
    .await;

    assert_eq!(stub.request_count(), 1, "expected exactly one reply request");
    let sys = system_text(&stub.last_request());
    assert!(
        !sys.contains(secret),
        "another user's private assistant instructions must NOT be injected; leaked system text: {sys}"
    );

    drop(server);
}
