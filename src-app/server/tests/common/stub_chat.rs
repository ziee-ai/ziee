//! In-process OpenAI-compatible **stub chat provider** for chat-loop tests.
//!
//! The audit's **T0 prerequisite**: there is no mock CHAT provider in the
//! server tests — `tests/chat/helpers.rs::get_or_create_test_model` uses a REAL
//! provider keyed by env API keys, so nothing scripts model output. Almost every
//! Track A/B chat-loop assertion (manifest injected, `read_file` round-trips,
//! inline self-save without a 2nd call, capability gating) needs a deterministic
//! model.
//!
//! This fixture stands up a loopback axum server speaking the OpenAI
//! `/v1/chat/completions` SSE + tool-call wire format the real `OpenAIProvider`
//! parses. Registering it as a `custom` provider therefore exercises the FULL
//! chat path (request build → SSE parse → tool-use loop → continuation) with
//! canned, scripted output — only token generation is faked.
//!
//! ## Scripting
//! The model's behaviour is driven by a `STUB_PLAN=<plan>` token the test author
//! embeds in the user message, combined with whether the request history already
//! carries a tool result (turn detection — the same request is replayed each
//! continuation, so the presence of a `role:"tool"` message is what advances the
//! script). Plans:
//!   - `text` (default): one assistant turn of plain text.
//!   - `read_first_file`: turn 1 → `read_file({id})` where `id` is parsed from
//!     the injected manifest system block; turn 2 (a tool result is present) →
//!     text that echoes the returned file content (proves the round-trip).
//!   - `grep_first_file`: turn 1 → `grep_files({pattern})` (pattern from a
//!     `STUB_GREP=<word>` token, default `the`); turn 2 → text.
//!   - `remember`: ONE turn → an answer text **and** a `remember` tool call in
//!     the same assistant message (Track B inline self-save; the side-effect loop
//!     must finalize without a 2nd generation call).
//!
//! Every `/v1/chat/completions` hit is recorded so a test can assert what the
//! model actually saw (manifest present? which tools attached? tool result on
//! the continuation?) and how many generation calls the loop made.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// One recorded `/v1/chat/completions` request — the slice of the request the
/// chat-loop tests assert against.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    /// `function.name` of every tool attached to this request.
    pub tool_names: Vec<String>,
    /// True when the message history already carried a `role:"tool"` result —
    /// i.e. this is a continuation turn, not the first generation.
    pub had_tool_result: bool,
    /// True when a system message carried the Track A files manifest header.
    pub has_manifest: bool,
    /// The `STUB_PLAN=` token parsed from the last user message (or `"text"`).
    pub plan: String,
    /// Concatenated visible text of EVERY message in the request (system + user
    /// + tool). Lets a test assert whether a file's inlined content bytes are
    /// present (e.g. that an old attachment was NOT re-inlined on a later turn).
    pub all_text: String,
}

impl RecordedRequest {
    pub fn has_tool(&self, name: &str) -> bool {
        self.tool_names.iter().any(|t| tool_name_matches(t, name))
    }
}

/// MCP tools reach the model namespaced as `{server_id}__{tool}` (see
/// `mcp/chat_extension/helpers.rs::convert_mcp_tool_to_ai_tool`), so a test
/// asking for the bare `read_file`/`remember` must match the prefixed wire name.
fn tool_name_matches(wire_name: &str, bare: &str) -> bool {
    wire_name == bare || wire_name.ends_with(&format!("__{bare}"))
}

/// Resolve the FULL wire name (e.g. `{server_id}__read_file`) for a bare tool the
/// stub wants to call. The chat loop recovers the route by splitting on `__`, so
/// the stub MUST emit the prefixed name it actually saw, not the bare one.
fn resolve_wire_name<'a>(tool_names: &'a [String], bare: &str) -> Option<&'a str> {
    tool_names
        .iter()
        .find(|t| tool_name_matches(t, bare))
        .map(|s| s.as_str())
}

#[derive(Clone)]
struct StubState {
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

/// A running stub chat server. Drop aborts the background task.
pub struct StubChat {
    /// Base URL to register as the provider's `base_url`
    /// (`http://127.0.0.1:PORT/v1`) — the OpenAI provider appends
    /// `/chat/completions`.
    pub base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: JoinHandle<()>,
}

impl Drop for StubChat {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl StubChat {
    /// Bind a loopback OpenAI-compatible stub and start serving.
    pub async fn start() -> StubChat {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = StubState {
            requests: requests.clone(),
        };
        let app = Router::new()
            .route("/v1/models", get(models))
            .route("/v1/chat/completions", post(chat_completions))
            .route("/v1/embeddings", post(embeddings))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stub chat server");
        let port = listener.local_addr().expect("local_addr").port();
        let base_url = format!("http://127.0.0.1:{port}/v1");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app.into_make_service()).await;
        });
        StubChat {
            base_url,
            requests,
            handle,
        }
    }

    /// All recorded requests (clone — safe to inspect after the send).
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Count generation calls whose tool set included `name`. The title /
    /// summarizer extensions issue tool-less calls, so counting tool-carrying
    /// requests isolates the main chat loop from those.
    pub fn requests_with_tool(&self, name: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.has_tool(name))
            .count()
    }

    /// True if any recorded request carried the Track A manifest system block.
    pub fn any_manifest(&self) -> bool {
        self.requests().iter().any(|r| r.has_manifest)
    }
}

async fn models() -> Response {
    Json(json!({
        "object": "list",
        "data": [{"id": "stub-model", "object": "model", "owned_by": "stub"}]
    }))
    .into_response()
}

async fn embeddings(body: axum::body::Bytes) -> Response {
    let model = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_else(|| "stub-embed".to_string());
    Json(json!({
        "object": "list",
        "model": model,
        "data": [{"object": "embedding", "index": 0, "embedding": [0.01, 0.02, 0.03, 0.04]}],
        "usage": {"prompt_tokens": 1, "total_tokens": 1}
    }))
    .into_response()
}

async fn chat_completions(State(s): State<StubState>, body: axum::body::Bytes) -> Response {
    let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let model = v
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("stub-model")
        .to_string();
    let streaming = v.get("stream").and_then(|b| b.as_bool()).unwrap_or(false);

    let empty = Vec::new();
    let messages = v.get("messages").and_then(|m| m.as_array()).unwrap_or(&empty);

    // Tool names attached to this request.
    let tool_names: Vec<String> = v
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    t.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    let had_tool_result = messages
        .iter()
        .any(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"));

    // System-block text (manifest detection + file-id parse). Concatenate every
    // system message's text.
    let system_text: String = messages
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .map(|m| message_text(m))
        .collect::<Vec<_>>()
        .join("\n");
    let has_manifest = system_text.contains("Files available in this conversation");

    let last_user = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .map(message_text)
        .unwrap_or_default();
    let plan = parse_token(&last_user, "STUB_PLAN=").unwrap_or_else(|| "text".to_string());

    let all_text: String = messages.iter().map(message_text).collect::<Vec<_>>().join("\n");

    s.requests.lock().unwrap().push(RecordedRequest {
        tool_names: tool_names.clone(),
        had_tool_result,
        has_manifest,
        all_text,
        plan: plan.clone(),
    });

    // Build the scripted turn: (text, optional tool call (name, args json)).
    let (text, tool_call) = script(&plan, had_tool_result, &tool_names, &system_text, &last_user, messages);

    if streaming {
        return stream_response(&model, text, tool_call);
    }
    json_response(&model, text, tool_call)
}

/// Decide the assistant turn. Returns `(text, Option<(tool_name, args_json)>)`.
fn script(
    plan: &str,
    had_tool_result: bool,
    tool_names: &[String],
    system_text: &str,
    last_user: &str,
    messages: &[Value],
) -> (Option<String>, Option<(String, Value)>) {
    match plan {
        "read_first_file" => {
            if let (false, Some(wire)) =
                (had_tool_result, resolve_wire_name(tool_names, "read_file"))
            {
                if let Some(id) = first_manifest_id(system_text) {
                    return (None, Some((wire.to_string(), json!({ "id": id }))));
                }
                // No id resolvable — degrade to text so the loop terminates.
                return (Some("No readable files were listed.".into()), None);
            }
            // Continuation: echo the tool result so the test can assert the
            // round-trip actually returned the file's content.
            let echoed = last_tool_result_text(messages);
            (
                Some(format!("Based on the file, here is the content: {echoed}")),
                None,
            )
        }
        "grep_first_file" => {
            if let (false, Some(wire)) =
                (had_tool_result, resolve_wire_name(tool_names, "grep_files"))
            {
                let pattern = parse_token(last_user, "STUB_GREP=").unwrap_or_else(|| "the".into());
                return (None, Some((wire.to_string(), json!({ "pattern": pattern }))));
            }
            let echoed = last_tool_result_text(messages);
            (Some(format!("Matches: {echoed}")), None)
        }
        "remember" => {
            if let (false, Some(wire)) =
                (had_tool_result, resolve_wire_name(tool_names, "remember"))
            {
                let content = parse_token(last_user, "STUB_PLAN=remember ")
                    .filter(|c| !c.trim().is_empty())
                    .unwrap_or_else(|| "The user shared a durable fact.".into());
                // Answer text AND the side-effect save in the same turn.
                return (
                    Some("Got it — I'll remember that.".into()),
                    Some((
                        wire.to_string(),
                        json!({ "content": content, "scope": "conversation" }),
                    )),
                );
            }
            (Some("Got it — I'll remember that.".into()), None)
        }
        // "text" and any unknown plan → a plain answer.
        _ => (Some("Hello from the stub model.".into()), None),
    }
}

/// Extract the visible text of an OpenAI message (`content` is a string OR an
/// array of `{type:"text", text}` parts).
fn message_text(m: &Value) -> String {
    match m.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| {
                if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                    p.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Text of the most recent `role:"tool"` message (the read_file/grep result).
fn last_tool_result_text(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"))
        .map(message_text)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

/// Parse the substring after `prefix` up to end-of-line.
fn parse_token(text: &str, prefix: &str) -> Option<String> {
    let idx = text.find(prefix)?;
    let rest = &text[idx + prefix.len()..];
    let line: String = rest.lines().next().unwrap_or("").trim().to_string();
    if line.is_empty() { None } else { Some(line) }
}

/// Parse the first `id=<uuid>` from the manifest system block.
fn first_manifest_id(system_text: &str) -> Option<String> {
    // Manifest rows are `- id={uuid} · …`. Find `id=` then read 36 chars.
    let idx = system_text.find("id=")?;
    let after = &system_text[idx + 3..];
    let candidate: String = after.chars().take(36).collect();
    // Cheap UUID shape check (8-4-4-4-12 with hyphens at the right spots).
    if candidate.len() == 36
        && candidate.as_bytes()[8] == b'-'
        && candidate.as_bytes()[13] == b'-'
        && candidate.as_bytes()[18] == b'-'
        && candidate.as_bytes()[23] == b'-'
        && candidate
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        Some(candidate)
    } else {
        None
    }
}

fn stream_response(model: &str, text: Option<String>, tool_call: Option<(String, Value)>) -> Response {
    let mut events: Vec<Event> = Vec::new();
    events.push(sse_chunk(model, json!({"role": "assistant"}), None));

    if let Some(t) = &text {
        events.push(sse_chunk(model, json!({"content": t}), None));
    }

    let finish = if let Some((name, args)) = &tool_call {
        events.push(sse_chunk(
            model,
            json!({
                "tool_calls": [{
                    "index": 0,
                    "id": "call_stub_1",
                    "type": "function",
                    "function": { "name": name, "arguments": args.to_string() }
                }]
            }),
            None,
        ));
        "tool_calls"
    } else {
        "stop"
    };
    events.push(sse_chunk(model, json!({}), Some(finish)));

    let stream = futures::stream::iter(
        events
            .into_iter()
            .map(Ok::<Event, std::convert::Infallible>)
            .chain(std::iter::once(Ok(Event::default().data("[DONE]")))),
    );
    Sse::new(stream).into_response()
}

fn json_response(model: &str, text: Option<String>, tool_call: Option<(String, Value)>) -> Response {
    let mut message = json!({ "role": "assistant", "content": text });
    let finish = if let Some((name, args)) = &tool_call {
        message["tool_calls"] = json!([{
            "id": "call_stub_1",
            "type": "function",
            "function": { "name": name, "arguments": args.to_string() }
        }]);
        "tool_calls"
    } else {
        "stop"
    };
    Json(json!({
        "id": "chatcmpl-stub",
        "object": "chat.completion",
        "model": model,
        "choices": [{ "index": 0, "message": message, "finish_reason": finish }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    }))
    .into_response()
}

fn sse_chunk(model: &str, delta: Value, finish: Option<&str>) -> Event {
    let data = json!({
        "id": "chatcmpl-stub",
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
    });
    Event::default().data(data.to_string())
}

// ── Provider + model registration against the stub ──────────────────────────

/// Register a `custom` provider pointing at the stub + one tool-capable model,
/// and grant `user_id` access via a fresh group. Returns the model id (UUID
/// string). `tools` controls `capabilities.tools`; `context_length` (when set)
/// seeds `capabilities.context_length` for the summarizer window tests.
///
/// `admin_token` must carry `llm_providers::{read,edit}` + `llm_models::{read,
/// create}` + group-management permissions.
pub async fn register_stub_model(
    server: &crate::common::TestServer,
    admin_token: &str,
    user_id: &str,
    base_url: &str,
    tools: bool,
    context_length: Option<u32>,
) -> String {
    use reqwest::StatusCode;
    let client = reqwest::Client::new();

    // 1. Provider (custom → OpenAI-compatible against the stub URL).
    let provider: Value = {
        let resp = client
            .post(server.api_url("/llm-providers"))
            .header("Authorization", format!("Bearer {admin_token}"))
            .json(&json!({
                "name": format!("stub_provider_{}", &uuid::Uuid::new_v4().to_string()[..8]),
                "provider_type": "custom",
                "enabled": true,
                "api_key": "stub-key",
                "base_url": base_url,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::CREATED,
            "stub provider create failed: {}",
            resp.text().await.unwrap_or_default()
        );
        resp.json().await.unwrap()
    };
    let provider_id = provider["id"].as_str().unwrap().to_string();

    // 2. Model with tool capability (+ optional native context window).
    let mut capabilities = json!({ "chat": true, "tools": tools });
    if let Some(cl) = context_length {
        capabilities["context_length"] = json!(cl);
    }
    let model: Value = {
        let resp = client
            .post(server.api_url("/llm-models"))
            .header("Authorization", format!("Bearer {admin_token}"))
            .json(&json!({
                "provider_id": provider_id,
                "name": "stub-model",
                "display_name": "Stub Model",
                "enabled": true,
                "engine_type": "none",
                "file_format": "gguf",
                "capabilities": capabilities,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::CREATED,
            "stub model create failed: {}",
            resp.text().await.unwrap_or_default()
        );
        resp.json().await.unwrap()
    };
    let model_id = model["id"].as_str().unwrap().to_string();

    // 3. Grant the user access: fresh group → user → provider.
    let group: Value = client
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "name": format!("stub_access_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            "description": "stub model access",
            "permissions": []
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let group_id = group["id"].as_str().unwrap();

    let r = client
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "user_id": user_id, "group_id": group_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT, "group assign failed");

    let r = client
        .put(server.api_url(&format!("/groups/{group_id}/providers")))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "provider_ids": [provider_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK, "provider→group assign failed");

    model_id
}
