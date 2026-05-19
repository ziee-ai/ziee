//! Mock MCP server for elicitation roundtrip testing.
//!
//! Unlike `mock_mcp_server.rs` (stateless per-request), this fixture keeps
//! a `tools/call` SSE stream open across multiple HTTP requests so it can
//! coordinate the full elicitation handshake:
//!
//! 1. Client POSTs `tools/call` → mock returns an SSE stream that emits an
//!    `elicitation/create` request, then awaits a signal.
//! 2. Client POSTs the elicitation response (a JSON-RPC response to the
//!    server-initiated request) → mock records it and signals the open SSE
//!    stream to continue.
//! 3. Mock emits the tool result on the still-open SSE stream and closes.
//!
//! Each request body is also recorded for assertion after the test runs.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// Programmable per-test plan for a single tool call.
#[derive(Clone)]
pub struct ElicitationScript {
    /// elicitation/create `message` field
    pub message: String,
    /// elicitation/create `requestedSchema` field
    pub requested_schema: serde_json::Value,
    /// Final tool result content (placed under `content`)
    pub tool_result_content: Vec<serde_json::Value>,
    /// How long to wait for the client's elicitation response before
    /// giving up and emitting the tool result anyway (so tests don't hang).
    pub elicitation_response_timeout: Duration,
}

impl Default for ElicitationScript {
    fn default() -> Self {
        Self {
            message: "Please confirm.".to_string(),
            requested_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "confirm": { "type": "boolean" }
                },
                "required": ["confirm"]
            }),
            tool_result_content: vec![serde_json::json!({
                "type": "text",
                "text": "tool done"
            })],
            elicitation_response_timeout: Duration::from_secs(5),
        }
    }
}

struct State_ {
    script: Mutex<ElicitationScript>,
    /// Fires when the client POSTs the elicitation response.
    elicitation_response_received: Arc<Notify>,
    /// Recorded elicitation response bodies (one per response POST).
    elicitation_responses: Mutex<Vec<serde_json::Value>>,
    /// Recorded incoming requests (method + raw body) for assertion.
    requests: Mutex<Vec<serde_json::Value>>,
    /// Session id assigned on initialize.
    session_id: String,
    /// Monotonic id used for server-initiated requests (elicitation/create).
    next_server_request_id: Mutex<i64>,
    /// How many elicitation/create requests to issue per tool call (default 1).
    elicitations_per_tool_call: Mutex<u32>,
}

pub struct MockElicitationServer {
    state: Arc<State_>,
    base_url: String,
    handle: Option<JoinHandle<()>>,
}

impl MockElicitationServer {
    pub async fn start() -> Self {
        Self::start_with_script(ElicitationScript::default()).await
    }

    pub async fn start_with_script(script: ElicitationScript) -> Self {
        let state = Arc::new(State_ {
            script: Mutex::new(script),
            elicitation_response_received: Arc::new(Notify::new()),
            elicitation_responses: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
            session_id: format!("elicit-mock-{}", uuid::Uuid::new_v4()),
            next_server_request_id: Mutex::new(1000),
            elicitations_per_tool_call: Mutex::new(1),
        });

        let app = Router::new()
            .route("/", post(handle_post).delete(handle_delete))
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}/", port);

        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        Self { state, base_url, handle: Some(handle) }
    }

    pub fn base_url(&self) -> String {
        self.base_url.clone()
    }

    pub fn elicitation_responses(&self) -> Vec<serde_json::Value> {
        self.state.elicitation_responses.lock().unwrap().clone()
    }

    pub fn requests(&self) -> Vec<serde_json::Value> {
        self.state.requests.lock().unwrap().clone()
    }

    pub fn set_elicitations_per_tool_call(&self, n: u32) {
        *self.state.elicitations_per_tool_call.lock().unwrap() = n;
    }
}

impl Drop for MockElicitationServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

// ─── Handlers ──────────────────────────────────────────────────────────────

async fn handle_post(
    State(state): State<Arc<State_>>,
    _headers: HeaderMap,
    body: String,
) -> Response {
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}"#))
                .unwrap();
        }
    };

    state.requests.lock().unwrap().push(json.clone());

    let method = json.get("method").and_then(|m| m.as_str());
    let id = json.get("id").cloned();

    // Response to a server-initiated request (no method, has result/error).
    if method.is_none() && (json.get("result").is_some() || json.get("error").is_some()) {
        state.elicitation_responses.lock().unwrap().push(json.clone());
        state.elicitation_response_received.notify_one();
        return Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Body::from(""))
            .unwrap();
    }

    match method {
        Some("initialize") => {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": { "tools": {}, "elicitation": {} },
                    "serverInfo": { "name": "mock-elicit", "version": "0.0.1" },
                },
            });
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("MCP-Session-Id", &state.session_id)
                .body(Body::from(body.to_string()))
                .unwrap()
        }
        Some(m) if m.starts_with("notifications/") => Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Body::from(""))
            .unwrap(),
        Some("tools/call") => {
            // Return a long-lived SSE stream that emits elicitation/create
            // event(s), waits for the client's response, then emits the
            // tool result.
            let tool_call_id = id.clone().unwrap_or(serde_json::Value::Null);
            let stream = build_elicitation_stream(state.clone(), tool_call_id);
            Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
        }
        Some(other) => {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", other) },
            });
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(r#"{"error":"missing method"}"#))
            .unwrap(),
    }
}

async fn handle_delete(State(_state): State<Arc<State_>>) -> StatusCode {
    StatusCode::OK
}

// ─── Stream builder ────────────────────────────────────────────────────────

fn build_elicitation_stream(
    state: Arc<State_>,
    tool_call_id: serde_json::Value,
) -> Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> {
    use async_stream::stream;

    let s = stream! {
        let n_elicitations = *state.elicitations_per_tool_call.lock().unwrap();
        let script = state.script.lock().unwrap().clone();
        let notify = state.elicitation_response_received.clone();

        for _ in 0..n_elicitations {
            // Server-initiated elicitation/create request.
            let server_req_id = {
                let mut g = state.next_server_request_id.lock().unwrap();
                let id = *g;
                *g += 1;
                id
            };
            let elicit_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": server_req_id,
                "method": "elicitation/create",
                "params": {
                    "message": script.message,
                    "requestedSchema": script.requested_schema,
                }
            });
            yield Ok(Event::default().data(elicit_req.to_string()));

            // Wait for the client to POST the response on a different
            // connection. Bounded by the script's timeout so tests don't hang.
            let _ = tokio::time::timeout(
                script.elicitation_response_timeout,
                notify.notified(),
            ).await;
        }

        // Emit tool result and close.
        let tool_result = serde_json::json!({
            "jsonrpc": "2.0",
            "id": tool_call_id,
            "result": {
                "content": script.tool_result_content,
                "isError": false,
            }
        });
        yield Ok(Event::default().data(tool_result.to_string()));
    };

    Box::pin(s)
}

