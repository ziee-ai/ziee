//! Built-in `ask_user` elicitation integration tests (chat-extension level).
//!
//! Exercises the LLM-initiated elicitation roundtrip end-to-end:
//!
//!   user message
//!     → assistant calls the always-on built-in `ask_user` tool
//!     → chat loop intercepts it (no loopback dispatch), registers the
//!       elicitation + emits `mcpElicitationRequired` on the chat stream
//!     → test POSTs `/api/mcp/elicitation/{id}/respond`
//!     → the oneshot unblocks → the answer becomes the tool result
//!     → assistant continues in the SAME turn and echoes the answer
//!
//! Unlike `mcp_elicitation_test.rs` (external mock MCP server + real LLM),
//! this drives the **built-in** server, which is auto-attached + auto-approved
//! in every conversation, so there's no server to create and no auto-approve
//! to set. The model is the deterministic [`StubChat`] (`STUB_PLAN=ask_user`),
//! so these run free + offline — no `ANTHROPIC_API_KEY` needed.

use std::time::Duration;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::chat_stream_probe::ChatStreamProbe;
use crate::common::stub_chat::{register_stub_model, StubChat};
use crate::common::test_helpers::{create_user_with_permissions, TestUser};
use crate::common::TestServer;

const TURN_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-test scaffold: stub model + a fresh conversation + an open, subscribed
/// chat-stream probe. Returns everything a test needs to drive a turn.
struct Fixture {
    server: TestServer,
    user: TestUser,
    model_id: Uuid,
    conv_id: Uuid,
    branch_id: Uuid,
    probe: ChatStreamProbe,
    // Kept alive for the test; also inspected (`stub.requests()`) to assert what
    // tools the chat loop actually attached.
    stub: StubChat,
}

/// Default scaffold: a TOOL-CAPABLE stub model (so ask_user is auto-attached).
async fn setup() -> Fixture {
    setup_with(true).await
}

/// Scaffold with an explicit tool-capability so tests can prove the always-on
/// ask_user attach is GATED on `model_tools_capable` (a non-tool model must NOT
/// receive the ask_user tool).
async fn setup_with(tools: bool) -> Fixture {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = create_user_with_permissions(&server, "ask_user", &["*"]).await;
    let model_id_str =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, tools, None).await;
    let model_id = Uuid::parse_str(&model_id_str).unwrap();

    let conv = post(
        &server,
        &user.token,
        "/conversations",
        json!({ "model_id": model_id_str }),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    let probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv_id)).await;

    Fixture {
        server,
        user,
        model_id,
        conv_id,
        branch_id,
        probe,
        stub,
    }
}

async fn post(server: &TestServer, token: &str, path: &str, body: Value) -> Value {
    let resp = reqwest::Client::new()
        .post(server.api_url(path))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("post");
    assert!(
        resp.status().is_success(),
        "POST {path} -> {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.unwrap_or(Value::Null)
}

/// Fire the chat turn that makes the stub call `ask_user`. `ask_user` is
/// auto-attached, so the request only needs `enable_mcp` + an EMPTY server list.
async fn send_ask_user_turn(fx: &Fixture) {
    send_turn(fx, "STUB_PLAN=ask_user pick a color for me").await;
}

/// Fire a chat turn with arbitrary content (drives different STUB_PLAN arms).
/// `ask_user` is auto-attached for tool-capable models, so the request only
/// needs `enable_mcp` + an EMPTY server list (no third-party server requested).
async fn send_turn(fx: &Fixture, content: &str) {
    let payload = json!({
        "content": content,
        "model_id": fx.model_id.to_string(),
        "branch_id": fx.branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [] },
    });
    let resp = reqwest::Client::new()
        .post(
            fx.server
                .api_url(&format!("/conversations/{}/messages", fx.conv_id)),
        )
        .header("Authorization", format!("Bearer {}", fx.user.token))
        .json(&payload)
        .send()
        .await
        .expect("send message");
    assert_eq!(
        resp.status(),
        200,
        "send: {}",
        resp.text().await.unwrap_or_default()
    );
}

/// The wire tool names the chat loop attached to the FIRST tool-carrying
/// generation request (the loop request; title/summarizer requests are
/// tool-less and excluded). Empty vec if no request carried tools.
fn first_attached_tool_names(fx: &Fixture) -> Vec<String> {
    fx.stub
        .requests()
        .into_iter()
        .find(|r| !r.tool_names.is_empty())
        .map(|r| r.tool_names)
        .unwrap_or_default()
}

/// POST a response to an elicitation. Returns the raw response so callers can
/// assert on the status (200 happy path / 403 owner-mismatch).
async fn respond(
    server: &TestServer,
    token: &str,
    elicitation_id: &str,
    body: Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/mcp/elicitation/{elicitation_id}/respond")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("respond POST")
}

/// Drive the turn up to the elicitation gate and return the `mcpElicitationRequired`
/// frame's data. Asserts the form shape (the schema + the "Assistant" server label).
async fn drive_to_elicitation(fx: &mut Fixture) -> Value {
    send_ask_user_turn(fx).await;

    let frames = fx
        .probe
        .collect_until(fx.conv_id, &["mcpElicitationRequired"], TURN_TIMEOUT)
        .await;
    let last = frames.last().expect("at least one frame");
    assert_eq!(
        last.event_type, "mcpElicitationRequired",
        "turn must pause on the elicitation gate, got '{}'",
        last.event_type
    );
    last.data.clone()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// Happy path: the model calls `ask_user`, the user accepts with a value, and
/// the answer flows back to the model as the tool result (the stub echoes it).
#[tokio::test]
async fn ask_user_accept_returns_the_answer_to_the_model() {
    let mut fx = setup().await;

    let data = drive_to_elicitation(&mut fx).await;

    // The form surfaced by the assistant carries the requested JSON Schema +
    // the "Assistant" server label (NOT a third-party MCP server name).
    assert_eq!(
        data["server"].as_str(),
        Some("Assistant"),
        "the assistant is the one asking"
    );
    let enum_vals = &data["requested_schema"]["properties"]["color"]["enum"];
    assert!(
        enum_vals.as_array().is_some_and(|a| a.iter().any(|v| v == "green")),
        "the multiple-choice schema should reach the client: {data}"
    );

    // With an EMPTY mcp_config, the loop attaches ONLY auto-attached BUILT-INS
    // (no third-party server requested). For a tool-capable model that set is:
    // ask_user (elicitation) + get_tool_result (tool_result), both always-on, plus
    // literature_search/fetch_paper_fulltext (lit_search, enabled by default).
    // This proves always-on attach works AND doesn't pull in any third-party tool.
    let tools = first_attached_tool_names(&fx);
    assert!(
        tools.iter().any(|t| t.ends_with("__ask_user") || t == "ask_user"),
        "ask_user must be auto-attached, got: {tools:?}"
    );
    // Every attached tool is a known built-in — nothing external leaked in.
    const BUILTIN_SUFFIXES: &[&str] = &[
        "__ask_user",
        "__get_tool_result",
        "__literature_search",
        "__fetch_paper_fulltext",
    ];
    assert!(
        tools
            .iter()
            .all(|t| BUILTIN_SUFFIXES.iter().any(|s| t.ends_with(s))),
        "only built-in tools should attach with an empty mcp_config, got: {tools:?}"
    );

    let elicitation_id = data["elicitation_id"].as_str().expect("elicitation_id").to_string();

    // Accept with a value → unblocks the tool.
    let resp = respond(
        &fx.server,
        &fx.user.token,
        &elicitation_id,
        json!({ "action": "accept", "content": { "color": "green" } }),
    )
    .await;
    assert_eq!(resp.status(), 200, "accept respond must succeed");

    // The turn resumes; the stub echoes the answer it received as the tool result.
    let frames = fx
        .probe
        .collect_until_terminal(fx.conv_id, TURN_TIMEOUT)
        .await;
    let terminal = frames.last().expect("terminal frame");
    assert_eq!(terminal.event_type, "complete", "turn should complete");

    // Pin the EXACT serialized answer the model received — not just that the
    // token "green" appears somewhere — so a regression that drops the field
    // name or mangles the JSON is caught.
    let text = ChatStreamProbe::assemble_text(&frames);
    assert!(
        text.contains(r#"{"color":"green"}"#),
        "assistant should echo the exact serialized answer; got: {text:?}"
    );
}

/// Decline path: the user declines, and the model is told so (non-error result)
/// so it can reason about the outcome instead of treating it as a failure.
#[tokio::test]
async fn ask_user_decline_is_reported_to_the_model() {
    let mut fx = setup().await;

    let data = drive_to_elicitation(&mut fx).await;
    let elicitation_id = data["elicitation_id"].as_str().expect("elicitation_id").to_string();

    let resp = respond(
        &fx.server,
        &fx.user.token,
        &elicitation_id,
        json!({ "action": "decline" }),
    )
    .await;
    assert_eq!(resp.status(), 200, "decline respond must succeed");

    let frames = fx
        .probe
        .collect_until_terminal(fx.conv_id, TURN_TIMEOUT)
        .await;
    let terminal = frames.last().expect("terminal frame");
    assert_eq!(terminal.event_type, "complete", "turn should complete after decline");

    let text = ChatStreamProbe::assemble_text(&frames);
    assert!(
        text.contains("declined"),
        "the decline must reach the model as the tool result; got: {text:?}"
    );
}

/// Cancel path: the user dismisses the form. The model is told "no response"
/// (non-error) — the same result a timeout/stream-close synthesizes — so it can
/// continue without treating the dismissal as a tool failure.
#[tokio::test]
async fn ask_user_cancel_is_reported_as_no_response() {
    let mut fx = setup().await;

    let data = drive_to_elicitation(&mut fx).await;
    let elicitation_id = data["elicitation_id"].as_str().expect("elicitation_id").to_string();

    let resp = respond(
        &fx.server,
        &fx.user.token,
        &elicitation_id,
        json!({ "action": "cancel" }),
    )
    .await;
    assert_eq!(resp.status(), 200, "cancel respond must succeed");

    let frames = fx
        .probe
        .collect_until_terminal(fx.conv_id, TURN_TIMEOUT)
        .await;
    let terminal = frames.last().expect("terminal frame");
    assert_eq!(terminal.event_type, "complete", "turn should complete after cancel");

    let text = ChatStreamProbe::assemble_text(&frames);
    assert!(
        text.contains("The user did not respond (cancelled or timed out)."),
        "cancel must reach the model as the exact non-response marker; got: {text:?}"
    );
}

/// Security: a different user (even one WITH `mcp_servers::read`) cannot answer
/// an elicitation they do not own — the registry owner-bind fails the check 403.
#[tokio::test]
async fn ask_user_response_by_non_owner_is_forbidden() {
    let mut fx = setup().await;

    let data = drive_to_elicitation(&mut fx).await;
    let elicitation_id = data["elicitation_id"].as_str().expect("elicitation_id").to_string();

    // A second, unrelated user — passes the permission gate (`*`) but is not the
    // elicitation owner, so the owner-check must reject them.
    let attacker = create_user_with_permissions(&fx.server, "ask_user_attacker", &["*"]).await;
    let resp = respond(
        &fx.server,
        &attacker.token,
        &elicitation_id,
        json!({ "action": "accept", "content": { "color": "red" } }),
    )
    .await;
    assert_eq!(
        resp.status(),
        403,
        "a non-owner must not be able to answer another user's elicitation"
    );

    // The rightful owner can still answer, proving the elicitation is intact.
    let resp = respond(
        &fx.server,
        &fx.user.token,
        &elicitation_id,
        json!({ "action": "accept", "content": { "color": "blue" } }),
    )
    .await;
    assert_eq!(resp.status(), 200, "the owner can still answer");

    let frames = fx
        .probe
        .collect_until_terminal(fx.conv_id, TURN_TIMEOUT)
        .await;
    let text = ChatStreamProbe::assemble_text(&frames);
    assert!(
        text.contains("blue"),
        "the owner's answer should flow back to the model; got: {text:?}"
    );
}

/// Multi-field + validated-input schema: a single ask_user form collecting a
/// free string, a bounded integer, and a pattern-validated string round-trips
/// every field back to the model in one turn.
#[tokio::test]
async fn ask_user_multi_field_schema_round_trips_all_fields() {
    let mut fx = setup().await;

    send_turn(&fx, "STUB_PLAN=ask_user_multi tell me about yourself").await;
    let frames = fx
        .probe
        .collect_until(fx.conv_id, &["mcpElicitationRequired"], TURN_TIMEOUT)
        .await;
    let data = frames.last().expect("frame").data.clone();
    assert_eq!(data["server"].as_str(), Some("Assistant"));
    // All three field shapes reached the client.
    let props = &data["requested_schema"]["properties"];
    assert_eq!(props["age"]["type"], "integer");
    assert_eq!(props["code"]["pattern"], "^[A-Z]{3}$");

    let elicitation_id = data["elicitation_id"].as_str().expect("elicitation_id").to_string();
    let resp = respond(
        &fx.server,
        &fx.user.token,
        &elicitation_id,
        json!({ "action": "accept", "content": { "nickname": "Phi", "age": 30, "code": "ABC" } }),
    )
    .await;
    assert_eq!(resp.status(), 200);

    let frames = fx.probe.collect_until_terminal(fx.conv_id, TURN_TIMEOUT).await;
    let text = ChatStreamProbe::assemble_text(&frames);
    // The whole answer JSON round-trips to the model.
    assert!(text.contains("\"nickname\":\"Phi\""), "got: {text:?}");
    assert!(text.contains("\"age\":30"), "got: {text:?}");
    assert!(text.contains("\"code\":\"ABC\""), "got: {text:?}");
}

/// Empty-message ask_user (malformed tool args): the built-in returns the
/// is_error "non-empty message" marker WITHOUT surfacing a form, and that marker
/// reaches the model so it can retry with a real prompt. No mcpElicitationRequired
/// is emitted, so the turn runs straight to terminal.
#[tokio::test]
async fn ask_user_empty_message_returns_error_without_form() {
    let mut fx = setup().await;

    send_turn(&fx, "STUB_PLAN=ask_user_empty go").await;
    let frames = fx.probe.collect_until_terminal(fx.conv_id, TURN_TIMEOUT).await;

    // No form was surfaced.
    assert!(
        !frames.iter().any(|f| f.event_type == "mcpElicitationRequired"),
        "an empty-message ask_user must NOT surface a form"
    );
    let text = ChatStreamProbe::assemble_text(&frames);
    assert!(
        text.contains("non-empty"),
        "the validation error must reach the model; got: {text:?}"
    );
}

/// Real-LLM end-to-end: a REAL Anthropic model DECIDES to call ask_user, the
/// backend emits the form on the chat stream, the test answers, and the model
/// continues using the answer. This exercises the full LLM -> backend -> stream
/// path the stub tests can't (and is what proves the form actually surfaces with
/// a real model). Gated on ANTHROPIC_API_KEY — skips cleanly when unset.
#[tokio::test]
async fn ask_user_real_llm_round_trip() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ask_user_real", &["*"]).await;

    // Cheapest current Claude snapshot; grants the user access. Returns null
    // (clean skip) when ANTHROPIC_API_KEY is absent.
    let cfg = crate::chat::helpers::TestModelConfig {
        provider_type: "anthropic",
        model_name: "claude-haiku-4-5-20251001",
        display_name: "Claude Haiku 4.5",
    };
    let model =
        crate::chat::helpers::create_test_model_with_config(&server, &cfg, Some(&user.user_id))
            .await;
    if model.is_null() {
        eprintln!("Skipping ask_user real-LLM test: ANTHROPIC_API_KEY not set");
        return;
    }
    let model_id = model["id"].as_str().unwrap().to_string();

    // Pin tool-capability on the model itself so the always-on gate attaches
    // ask_user regardless of catalog snapshot matching (best-effort; the catalog
    // also marks this model tool-capable).
    let _ = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-models/{model_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "capabilities": { "chat": true, "tools": true } }))
        .send()
        .await;

    let conv = post(
        &server,
        &user.token,
        "/conversations",
        json!({ "model_id": model_id }),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = conv["active_branch_id"].as_str().unwrap().to_string();

    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv_id)).await;

    let payload = json!({
        "content": "I want to pick a color. Use the ask_user tool to ask me to choose exactly one \
                    of: red, green, or blue (use an enum schema). Do NOT guess or choose for me — \
                    you MUST call ask_user and wait for my answer.",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [] },
    });
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("send message");
    assert_eq!(resp.status(), 200, "send: {}", resp.text().await.unwrap_or_default());

    // The REAL model should call ask_user → the form surfaces on the chat stream.
    let frames = probe
        .collect_until(conv_id, &["mcpElicitationRequired"], Duration::from_secs(90))
        .await;
    let last = frames.last().expect("at least one frame");
    assert_eq!(
        last.event_type, "mcpElicitationRequired",
        "the real model must call ask_user and surface a form; got '{}'",
        last.event_type
    );
    assert_eq!(
        last.data["server"].as_str(),
        Some("Assistant"),
        "the assistant is the one asking"
    );

    // Answer with a value for whatever field the model requested.
    let elicitation_id = last.data["elicitation_id"].as_str().expect("elicitation_id").to_string();
    let field = last.data["requested_schema"]["properties"]
        .as_object()
        .and_then(|p| p.keys().next().cloned())
        .unwrap_or_else(|| "color".to_string());
    let r = respond(
        &server,
        &user.token,
        &elicitation_id,
        json!({ "action": "accept", "content": { field: "green" } }),
    )
    .await;
    assert_eq!(r.status(), 200, "accept respond must succeed");

    // The turn resumes and finishes; the tool completed without error and the
    // model produced a reply that uses the answer.
    let frames = probe
        .collect_until_terminal(conv_id, Duration::from_secs(90))
        .await;
    let terminal = frames.last().expect("terminal frame");
    assert_eq!(terminal.event_type, "complete", "turn should complete after answering");
    assert!(
        !frames.iter().any(|f| f.event_type == "error"),
        "no error frames after answering"
    );
    let tool_complete = frames.iter().find(|f| f.event_type == "mcpToolComplete");
    if let Some(tc) = tool_complete {
        assert_ne!(
            tc.data["is_error"].as_bool(),
            Some(true),
            "ask_user tool must complete without error"
        );
    }
    let text = ChatStreamProbe::assemble_text(&frames);
    assert!(!text.trim().is_empty(), "assistant should reply after the answer");
    eprintln!("[ask_user real-LLM] final assistant reply: {text}");
}

/// Capability gate (regression guard for the always-on attach): a
/// NON-tool-capable model must NOT receive the ask_user tool. The chat request
/// carries NO tools, and the turn completes normally (no provider tools-array
/// rejection). Mirrors the unit coverage in mcp.rs::auto_attach_ids_from_flags
/// at the full HTTP layer.
#[tokio::test]
async fn ask_user_not_attached_to_non_tool_capable_model() {
    let mut fx = setup_with(false).await;

    send_turn(&fx, "just say hi").await;
    let frames = fx.probe.collect_until_terminal(fx.conv_id, TURN_TIMEOUT).await;
    let terminal = frames.last().expect("terminal frame");
    assert_eq!(terminal.event_type, "complete", "turn should complete cleanly");

    // The loop attached NO tools at all — ask_user was correctly gated off.
    assert!(
        first_attached_tool_names(&fx).is_empty(),
        "no tools should be attached to a non-tool-capable model, got: {:?}",
        first_attached_tool_names(&fx)
    );
    assert!(
        !frames.iter().any(|f| f.event_type == "mcpElicitationRequired"),
        "a non-tool-capable model must never surface an ask_user form"
    );
}
