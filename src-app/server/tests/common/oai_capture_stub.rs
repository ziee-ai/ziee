//! In-process, **raw-request-capturing** OpenAI-compatible chat stub.
//!
//! Complements `common::stub_chat` (the tool-loop fixture, which records a
//! processed `RecordedRequest` slice and scripts read_file/grep/remember turns):
//! THIS one captures every inbound `POST /v1/chat/completions` body **verbatim**
//! so a test can assert on the exact wire fields the provider layer produced
//! (temperature, reasoning_effort, top_k, stream, …), and scripts the response
//! shape (text + optional `reasoning_content` + tool calls + usage tokens). Used
//! by the ai-providers Tier-2 wiring tests.
//!
//! Unlike `stub_engine` (a fixed-reply subprocess), this runs an axum server on
//! the test runtime and (a) records every inbound `POST /v1/chat/completions`
//! body so a test can assert on the EXACT request the server's provider layer
//! produced, and (b) replies with a scripted OpenAI response (text + optional
//! `reasoning_content` + tool calls + usage). A `custom` provider pointing at
//! `base_url()` routes the real chat consumer path (build → extensions →
//! `OpenAIProvider` → stream finalize → DB) here with no API keys.
//!
//! Streaming vs non-streaming is chosen from the request's `stream` flag, so a
//! model that triggers the gpt-5 non-streaming workaround (`stream:false`) gets a
//! single JSON body, everything else gets SSE.

use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

/// One scripted tool call the stub emits as an OpenAI streaming `tool_calls`
/// delta. `name` is the wire name the server expects (for MCP that's the
/// `server_id__tool` form); `arguments` is a JSON string.
#[derive(Clone)]
pub struct StubToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// What the stub replies with for every `/v1/chat/completions` call.
#[derive(Clone)]
pub struct StubPlan {
    pub text: String,
    /// Emitted as `reasoning_content` deltas (DeepSeek-R1 style) → thinking.
    pub reasoning: Option<String>,
    pub tool_calls: Vec<StubToolCall>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// `usage.completion_tokens_details.reasoning_tokens`.
    pub reasoning_tokens: Option<u32>,
    /// `usage.prompt_tokens_details.cached_tokens` (the cache-hit signal).
    pub cached_tokens: Option<u32>,
}

impl Default for StubPlan {
    fn default() -> Self {
        Self {
            text: "Hello from stub".to_string(),
            reasoning: None,
            tool_calls: Vec::new(),
            prompt_tokens: 11,
            completion_tokens: 3,
            reasoning_tokens: None,
            cached_tokens: None,
        }
    }
}

impl StubPlan {
    /// A plan that just replies with `text`.
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: t.into(),
            ..Default::default()
        }
    }

    /// Add a `reasoning_content` stream + its `reasoning_tokens` usage count.
    pub fn with_reasoning(mut self, reasoning: impl Into<String>, tokens: u32) -> Self {
        self.reasoning = Some(reasoning.into());
        self.reasoning_tokens = Some(tokens);
        self
    }

    /// Add a `cached_tokens` usage count (the prompt-cache read signal).
    pub fn with_cached_tokens(mut self, n: u32) -> Self {
        self.cached_tokens = Some(n);
        self
    }
}

struct AppState {
    requests: Arc<Mutex<Vec<Value>>>,
    plan: StubPlan,
}

/// A running stub. The axum task is aborted on drop.
pub struct StubChat {
    pub port: u16,
    requests: Arc<Mutex<Vec<Value>>>,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for StubChat {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl StubChat {
    /// Bind a free loopback port and serve the scripted `plan`.
    pub async fn start(plan: StubPlan) -> StubChat {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = Arc::new(AppState {
            requests: requests.clone(),
            plan,
        });

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stub_chat loopback");
        let port = listener.local_addr().unwrap().port();

        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .route("/v1/models", get(models))
            .route("/health", get(|| async { "ok" }))
            .with_state(state);

        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app.into_make_service()).await;
        });

        StubChat {
            port,
            requests,
            handle,
        }
    }

    /// OpenAI-compatible base URL to hand a `custom` provider.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    /// Every captured request body, in order.
    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }

    /// The most recent captured request body (panics if none).
    pub fn last_request(&self) -> Value {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("stub_chat received no /v1/chat/completions request")
    }

    /// How many requests the stub has served.
    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

async fn models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [
            { "id": "stub-model", "object": "model" },
            { "id": "gpt-5", "object": "model" }
        ]
    }))
}

async fn chat_completions(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> Response {
    state.requests.lock().unwrap().push(body.clone());

    // Default to streaming; the gpt-5 workaround sends `stream:false`.
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(true);
    if streaming {
        Response::builder()
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from(build_sse(&state.plan)))
            .unwrap()
    } else {
        Json(build_non_streaming(&state.plan)).into_response()
    }
}

/// OpenAI `usage` object for both response shapes.
fn usage_value(p: &StubPlan) -> Value {
    json!({
        "prompt_tokens": p.prompt_tokens,
        "completion_tokens": p.completion_tokens,
        "total_tokens": p.prompt_tokens + p.completion_tokens,
        "completion_tokens_details": p.reasoning_tokens.map(|r| json!({ "reasoning_tokens": r })),
        "prompt_tokens_details": p.cached_tokens.map(|c| json!({ "cached_tokens": c })),
    })
}

fn sse_event(out: &mut String, v: &Value) {
    out.push_str("data: ");
    out.push_str(&v.to_string());
    out.push_str("\n\n");
}

/// Build the OpenAI SSE response: reasoning deltas → tool-call deltas → text
/// delta → a final chunk carrying `finish_reason` + `usage` → `[DONE]`. Usage
/// rides the finish chunk (not a trailing chunk) so the server captures it
/// before it finalizes on `finish_reason`.
fn build_sse(p: &StubPlan) -> String {
    let mut out = String::new();

    if let Some(r) = &p.reasoning {
        sse_event(
            &mut out,
            &json!({
                "id": "stub",
                "choices": [{ "index": 0, "delta": { "reasoning_content": r }, "finish_reason": null }]
            }),
        );
    }

    for (i, tc) in p.tool_calls.iter().enumerate() {
        sse_event(
            &mut out,
            &json!({
                "id": "stub",
                "choices": [{
                    "index": 0,
                    "delta": { "tool_calls": [{
                        "index": i,
                        "id": tc.id,
                        "type": "function",
                        "function": { "name": tc.name, "arguments": tc.arguments }
                    }] },
                    "finish_reason": null
                }]
            }),
        );
    }

    if !p.text.is_empty() {
        sse_event(
            &mut out,
            &json!({
                "id": "stub",
                "choices": [{ "index": 0, "delta": { "content": p.text }, "finish_reason": null }]
            }),
        );
    }

    let finish = if p.tool_calls.is_empty() { "stop" } else { "tool_calls" };
    sse_event(
        &mut out,
        &json!({
            "id": "stub",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": finish }],
            "usage": usage_value(p)
        }),
    );

    out.push_str("data: [DONE]\n\n");
    out
}

/// Build the single-object (non-streaming) response for the gpt-5 workaround.
fn build_non_streaming(p: &StubPlan) -> Value {
    let mut message = json!({ "role": "assistant", "content": p.text });
    if let Some(r) = &p.reasoning {
        message["reasoning_content"] = json!(r);
    }
    json!({
        "id": "stub",
        "choices": [{ "index": 0, "message": message, "finish_reason": "stop" }],
        "usage": usage_value(p)
    })
}
