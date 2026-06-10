//! Tier-2 server-side integration tests for the `ai-providers` rework, driven
//! through the REAL chat consumer path (build → extensions → `OpenAIProvider` →
//! stream finalize → DB) with no API keys.
//!
//! Uses `common::stub_chat::StubChat`: an in-process OpenAI-compatible server
//! that records the exact request body the server's provider layer produced and
//! replies with a scripted response. A `custom` provider points the chat path at
//! it. (`custom` maps to the `OpenAIProvider` + the `"openai"` registry key, so a
//! model named `gpt-5` exercises registry-gated thinking + the non-streaming
//! workaround.)
//!
//! Coverage — the consumer wiring the crate's Tier-1 unit tests can't reach:
//!   * sampling: `model.parameters` reach the wire; OpenAI omits `top_k`; empty
//!     params fall back to the temperature/max_tokens defaults.
//!   * thinking: registry-gated enable emits `reasoning_effort` (+ non-streaming
//!     for gpt-5); an unknown model gets no thinking; a model's `reasoning_content`
//!     is persisted as a thinking content block with `ThinkingMetadata.token_count`.
//!   * caching telemetry: `cached_tokens` surfaces as `cache_read_input_tokens`
//!     on the terminal `complete` frame.
//!
//! Deferred (heavier setup, noted for a follow-up): the memory-prefix-stability
//! golden test (needs the memory extension + an embedding model) and the rich
//! tool-result image replay (needs a live MCP server returning an image). The
//! Anthropic/Gemini-specific wire shapes (adaptive thinking, cache_control,
//! tool_result image arrays, sampling gating) are owned by the crate's Tier-1
//! tests — the stub is OpenAI-shaped, so Tier-2 proves the consumer data-flow.

use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::helpers::{self, create_conversation, parse_uuid, send_and_collect};
use crate::common::oai_capture_stub::{StubChat, StubPlan};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

fn chat_perms() -> &'static [&'static str] {
    &[
        "conversations::create",
        "conversations::read",
        "messages::create",
        "messages::read",
        "llm_models::read",
    ]
}

/// Register a `custom` provider pointing at `base_url` + a chat model `model_name`
/// (with optional generation `parameters`), and grant `user_id` access. Mirrors
/// `helpers::create_stub_model` but lets the test pick the model name + params and
/// supply its own (capturing) stub.
async fn create_model(
    server: &TestServer,
    user_id: &str,
    base_url: &str,
    model_name: &str,
    parameters: Option<Value>,
) -> Value {
    let admin = create_user_with_permissions(
        server,
        "stub_chat_admin",
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

    let provider_resp = client
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("StubChat {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom",
            "enabled": true,
            "api_key": "test",
            "base_url": base_url,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        provider_resp.status(),
        StatusCode::CREATED,
        "stub-chat provider create failed"
    );
    let provider: Value = provider_resp.json().await.unwrap();

    let mut payload = json!({
        "provider_id": provider["id"],
        "name": model_name,
        "display_name": "Stub Chat Model",
        "description": "stub-chat tier-2 model",
        "enabled": true,
        "engine_type": "none",
        "file_format": "gguf",
        "capabilities": { "chat": true, "completion": true, "embedding": false }
    });
    if let Some(p) = parameters {
        payload["parameters"] = p;
    }

    let model_resp = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(
        model_resp.status(),
        StatusCode::CREATED,
        "stub-chat model create failed"
    );
    let model: Value = model_resp.json().await.unwrap();

    helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}

/// Create user → stub-backed model → conversation, then send one message and
/// wait for the turn to terminate (so the stub has received its request).
async fn run_turn(
    name: &str,
    plan: StubPlan,
    model_name: &str,
    parameters: Option<Value>,
) -> (StubChat, helpers::CollectedTurn) {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, name, chat_perms()).await;
    let stub = StubChat::start(plan).await;
    let model = create_model(&server, &user.user_id, &stub.base_url(), model_name, parameters).await;
    let model_id = parse_uuid(&model["id"]);

    // Preset a title so the title-generation extension (which makes its OWN
    // provider call) is skipped — the stub then receives exactly ONE request
    // (the reply), so `last_request()` is unambiguously the turn we assert on.
    let conversation =
        create_conversation(&server, &user.token, Some(model_id), Some("preset")).await;
    let conv_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    let turn = send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "hi").await;
    // Keep `server` alive until the turn is collected, then let it drop.
    drop(server);
    (stub, turn)
}

// ── T5: sampling params flow ──────────────────────────────────────────────

#[tokio::test]
async fn model_params_reach_provider_request() {
    // Use only values exactly representable in f32 to avoid float-repr flake.
    let params = json!({
        "temperature": 0.5,
        "top_p": 0.25,
        "top_k": 40,
        "seed": 42,
        "frequency_penalty": 0.125,
        "presence_penalty": 0.0625,
        "stop": ["END"],
        "max_tokens": 1234
    });
    let (stub, _turn) =
        run_turn("params_user", StubPlan::default(), "stub-model", Some(params)).await;

    // Exactly one provider call (title generation suppressed by the preset title).
    assert_eq!(
        stub.request_count(),
        1,
        "expected a single provider request for the reply"
    );
    let req = stub.last_request();
    assert_eq!(req["model"], "stub-model");
    assert_eq!(req["temperature"], 0.5);
    assert_eq!(req["top_p"], 0.25);
    assert_eq!(req["seed"], 42);
    assert_eq!(req["frequency_penalty"], 0.125);
    assert_eq!(req["presence_penalty"], 0.0625);
    assert_eq!(req["stop"][0], "END");
    assert_eq!(req["max_tokens"], 1234);
    // OpenAI Chat Completions has no `top_k` — it must be omitted even though the
    // model set it (it'd reach Anthropic/Gemini, not OpenAI).
    assert!(
        req.get("top_k").is_none(),
        "OpenAI request must not carry top_k, got: {req}"
    );
}

#[tokio::test]
async fn empty_model_params_fall_back_to_defaults() {
    let (stub, _turn) = run_turn("default_user", StubPlan::default(), "stub-model", None).await;

    let req = stub.last_request();
    // 0.7 is not exact in f32 — assert ~0.7 rather than exact-eq.
    let temp = req["temperature"].as_f64().expect("temperature present");
    assert!(
        (temp - 0.7).abs() < 1e-4,
        "default temperature should be ~0.7, got {temp}"
    );
    assert_eq!(req["max_tokens"], 8192, "default max_tokens");
}

// ── T1: thinking enable/disable + persistence ─────────────────────────────

#[tokio::test]
async fn thinking_enabled_for_registry_model_emits_reasoning_effort() {
    // `gpt-5` is registry thinking-capable (via custom→openai) AND triggers the
    // non-streaming workaround, so the request carries `reasoning_effort`,
    // `stream:false`, `max_completion_tokens`, and NO `temperature` (reasoning
    // models reject it). The stub answers non-streaming because stream:false.
    let (stub, turn) = run_turn(
        "thinking_user",
        StubPlan::text("done"),
        "gpt-5",
        None,
    )
    .await;

    let req = stub.last_request();
    assert_eq!(req["model"], "gpt-5");
    assert_eq!(req["reasoning_effort"], "high");
    assert_eq!(req["stream"], false, "gpt-5 uses the non-streaming workaround");
    assert!(
        req.get("max_completion_tokens").is_some(),
        "reasoning models use max_completion_tokens"
    );
    assert!(
        req.get("temperature").is_none(),
        "reasoning models must not send temperature, got: {req}"
    );
    assert_eq!(turn.text, "done");
}

#[tokio::test]
async fn thinking_disabled_for_unknown_model() {
    let (stub, _turn) =
        run_turn("no_thinking_user", StubPlan::default(), "stub-model", None).await;

    let req = stub.last_request();
    assert!(
        req.get("reasoning_effort").is_none(),
        "a non-registry model must not enable thinking, got: {req}"
    );
    assert_eq!(req["stream"], true, "non-gpt-5 models stream");
}

#[tokio::test]
async fn reasoning_content_persists_as_thinking_block() {
    // The model returns `reasoning_content` + `reasoning_tokens`; the server must
    // persist a thinking content block carrying that reasoning text, with
    // `ThinkingMetadata.token_count` = the reasoning token count.
    let plan = StubPlan::text("the answer").with_reasoning("let me think about it", 17);
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reasoning_user", chat_perms()).await;
    let stub = StubChat::start(plan).await;
    let model = create_model(&server, &user.user_id, &stub.base_url(), "stub-model", None).await;
    let model_id = parse_uuid(&model["id"]);

    // Preset title → skip the title extension's separate provider call.
    let conversation =
        create_conversation(&server, &user.token, Some(model_id), Some("preset")).await;
    let conv_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    let turn = send_and_collect(&server, &user.token, conv_id, branch_id, model_id, "hi").await;
    assert_eq!(turn.text, "the answer");

    let contents = helpers::get_message_contents_from_db(&server, turn.assistant_message_id).await;
    let thinking = contents
        .iter()
        .find(|c| c["content_type"] == "thinking")
        .unwrap_or_else(|| panic!("expected a thinking content block, got: {contents:?}"));
    assert_eq!(thinking["content"]["thinking"], "let me think about it");
    assert_eq!(
        thinking["content"]["metadata"]["token_count"], 17,
        "reasoning_tokens should land in ThinkingMetadata.token_count"
    );
    assert!(
        contents.iter().any(|c| c["content_type"] == "text"),
        "the answer text block should also persist"
    );
}

// ── Caching telemetry ─────────────────────────────────────────────────────

#[tokio::test]
async fn cache_read_tokens_surface_on_complete_frame() {
    // OpenAI reports cache hits via `prompt_tokens_details.cached_tokens`; the
    // crate maps that to `cache_read_input_tokens`, which must surface on the
    // terminal `complete` stream frame's usage.
    let plan = StubPlan::text("cached reply").with_cached_tokens(30);
    let (_stub, turn) = run_turn("cache_user", plan, "stub-model", None).await;

    let complete = turn
        .frames
        .iter()
        .find(|f| f.event_type == "complete")
        .expect("a complete frame");
    assert_eq!(
        complete.data["usage"]["cache_read_input_tokens"], 30,
        "cached_tokens should surface as cache_read_input_tokens on complete, got: {}",
        complete.data
    );
}
