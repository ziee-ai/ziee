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

/// Headers captured from a single elicitation-response POST (the client's
/// JSON-RPC reply to the server-initiated `elicitation/create` request).
/// Used by tests to assert the client sent the headers the MCP Streamable
/// HTTP spec requires on every POST.
#[derive(Clone, Debug, Default)]
pub struct RecordedPostHeaders {
    pub accept: Option<String>,
    pub mcp_protocol_version: Option<String>,
    pub mcp_session_id: Option<String>,
    pub authorization: Option<String>,
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Programmable per-test plan for a single tool call.
#[derive(Clone)]
pub struct ElicitationScript {
    /// Tool name advertised in tools/list (the LLM picks tools by name).
    pub tool_name: String,
    /// Tool description shown to the LLM via tools/list.
    pub tool_description: String,
    /// Tool input schema shown to the LLM via tools/list.
    pub tool_input_schema: serde_json::Value,
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
            tool_name: "elicit_tool".to_string(),
            tool_description: "A tool that requires user input via elicitation".to_string(),
            tool_input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
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
    /// When true, the client's elicitation-response POST MUST carry an
    /// `Accept` header listing `text/event-stream` (per the MCP Streamable
    /// HTTP spec). If it doesn't, the mock replies 406 and drops the message
    /// without signaling the open SSE stream — faithfully reproducing a
    /// spec-compliant server (the TypeScript / Python SDK) so the unanswered
    /// `elicitation/create` request times out. Default false (lenient).
    strict_response_accept: Mutex<bool>,
    /// Headers captured from each elicitation-response POST, in order.
    response_post_headers: Mutex<Vec<RecordedPostHeaders>>,
}

pub struct MockElicitationServer {
    state: Arc<State_>,
    base_url: String,
    handle: Option<JoinHandle<()>>,
}

impl MockElicitationServer {
    #[allow(dead_code)]
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
            strict_response_accept: Mutex::new(false),
            response_post_headers: Mutex::new(Vec::new()),
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

    #[allow(dead_code)]
    pub fn requests(&self) -> Vec<serde_json::Value> {
        self.state.requests.lock().unwrap().clone()
    }

    pub fn set_elicitations_per_tool_call(&self, n: u32) {
        *self.state.elicitations_per_tool_call.lock().unwrap() = n;
    }

    /// Require the client's elicitation-response POST to carry a spec-correct
    /// `Accept` header. When enabled, a missing/wrong `Accept` causes the mock
    /// to reply 406 and drop the message, so the `elicitation/create` request
    /// times out — reproducing the production bug against a strict server.
    pub fn set_strict_response_accept(&self, v: bool) {
        *self.state.strict_response_accept.lock().unwrap() = v;
    }

    /// Headers captured from each elicitation-response POST, in order.
    pub fn response_post_headers(&self) -> Vec<RecordedPostHeaders> {
        self.state.response_post_headers.lock().unwrap().clone()
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
    headers: HeaderMap,
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
        // Capture the headers the client sent on this response POST so tests
        // can assert the spec-required Accept / MCP-Protocol-Version / etc.
        let recorded = RecordedPostHeaders {
            accept: header_str(&headers, "accept"),
            mcp_protocol_version: header_str(&headers, "mcp-protocol-version"),
            mcp_session_id: header_str(&headers, "mcp-session-id"),
            authorization: header_str(&headers, "authorization"),
        };
        let accept_ok = recorded
            .accept
            .as_deref()
            .map(|a| a.contains("text/event-stream"))
            .unwrap_or(false);
        state.response_post_headers.lock().unwrap().push(recorded);

        // A spec-compliant server (the TypeScript / Python SDK) rejects a POST whose Accept
        // header does not list text/event-stream with 406 Not Acceptable and
        // drops the message. Simulate that under strict mode: the headers were
        // captured above for assertion, but skip the elicitation_responses push
        // and the notify, so the open stream is never signaled and the pending
        // elicitation/create request goes unanswered and times out.
        if *state.strict_response_accept.lock().unwrap() && !accept_ok {
            return Response::builder()
                .status(StatusCode::NOT_ACCEPTABLE)
                .body(Body::from(""))
                .unwrap();
        }

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
        Some("tools/list") => {
            let script = state.script.lock().unwrap().clone();
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [{
                        "name": script.tool_name,
                        "description": script.tool_description,
                        "inputSchema": script.tool_input_schema,
                    }]
                }
            });
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        }
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
        let strict = *state.strict_response_accept.lock().unwrap();

        let mut all_responded = true;
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
            let got = tokio::time::timeout(
                script.elicitation_response_timeout,
                notify.notified(),
            ).await.is_ok();
            if !got {
                all_responded = false;
                break;
            }
        }

        if !all_responded && strict {
            // No (valid) response arrived for an elicitation/create request.
            // Mirror a spec-compliant server giving up: fail the tool call with
            // the exact error the production bug surfaced to users.
            let err = serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_call_id,
                "error": {
                    "code": -32001,
                    "message": "Elicitation failed: server->client request 'elicitation/create' timed out"
                }
            });
            yield Ok(Event::default().data(err.to_string()));
        } else {
            // Emit tool result and close. (Non-strict mode also takes this path
            // on timeout as a legacy safety net so older tests never hang.)
            let tool_result = serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_call_id,
                "result": {
                    "content": script.tool_result_content,
                    "isError": false,
                }
            });
            yield Ok(Event::default().data(tool_result.to_string()));
        }
    };

    Box::pin(s)
}

