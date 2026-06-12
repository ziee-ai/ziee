//! Mock MCP server that reproduces the `dscc` elicitation pattern: it answers
//! `tools/call` with a plain **`application/json`** response (no SSE on the POST
//! stream) and delivers its `elicitation/create` request on the **standalone
//! GET-SSE stream** instead. ziee historically dropped GET-stream elicitation
//! (the request would time out); this fixture exercises the GET-path handler.
//!
//! Handshake:
//! 1. Client `initialize` (POST) → caps advertise `elicitation`.
//! 2. Client opens the standalone `GET` stream (held open).
//! 3. Client POSTs `tools/call`. The handler pushes `elicitation/create` onto
//!    the open GET stream, then **blocks** awaiting the client's reply.
//! 4. Client POSTs the elicitation response (JSON-RPC result, no method) → the
//!    handler is unblocked and returns the tool result as `application/json`.

use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;

struct State_ {
    session_id: String,
    tool_name: String,
    message: String,
    requested_schema: serde_json::Value,
    tool_result_content: Vec<serde_json::Value>,
    /// Broadcasts serialized `elicitation/create` events to the open GET stream.
    elicit_tx: broadcast::Sender<String>,
    /// Fires when the client POSTs the elicitation response.
    response_received: Arc<Notify>,
    /// Recorded elicitation response bodies.
    responses: Mutex<Vec<serde_json::Value>>,
    next_server_request_id: Mutex<i64>,
    /// How long the `tools/call` handler waits for the client's reply before
    /// giving up and returning the tool result anyway (so tests never hang).
    response_timeout: Duration,
}

pub struct MockGetStreamElicitationServer {
    state: Arc<State_>,
    base_url: String,
    handle: Option<JoinHandle<()>>,
}

impl MockGetStreamElicitationServer {
    pub async fn start() -> Self {
        let (elicit_tx, _) = broadcast::channel::<String>(16);
        let state = Arc::new(State_ {
            session_id: format!("dscc-mock-{}", uuid::Uuid::new_v4()),
            tool_name: "get_stream_tool".to_string(),
            message: "Run the permutation test?".to_string(),
            requested_schema: serde_json::json!({
                "type": "object",
                "properties": { "empirical": { "type": "boolean" } },
                "required": ["empirical"]
            }),
            tool_result_content: vec![serde_json::json!({
                "type": "text",
                "text": "get-stream-tool-done"
            })],
            elicit_tx,
            response_received: Arc::new(Notify::new()),
            responses: Mutex::new(Vec::new()),
            next_server_request_id: Mutex::new(2000),
            response_timeout: Duration::from_secs(4),
        });

        let app = Router::new()
            .route("/", get(handle_get).post(handle_post).delete(handle_delete))
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

    pub fn responses(&self) -> Vec<serde_json::Value> {
        self.state.responses.lock().unwrap().clone()
    }
}

impl Drop for MockGetStreamElicitationServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

// ─── Handlers ──────────────────────────────────────────────────────────────

/// Standalone GET stream — forwards `elicitation/create` events the
/// `tools/call` handler broadcasts onto it. Subscribed at request time so the
/// `tools/call` handler can wait for `receiver_count() > 0` before broadcasting.
async fn handle_get(State(state): State<Arc<State_>>) -> Response {
    use async_stream::stream;
    let mut rx = state.elicit_tx.subscribe();
    let s = stream! {
        // Priming retry directive (mirrors real servers; also seeds backoff).
        yield Ok::<Event, Infallible>(Event::default().retry(Duration::from_secs(3)));
        loop {
            match rx.recv().await {
                Ok(data) => yield Ok(Event::default().data(data)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    let boxed: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = Box::pin(s);
    Sse::new(boxed).keep_alive(KeepAlive::default()).into_response()
}

async fn handle_post(State(state): State<Arc<State_>>, body: String) -> Response {
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}"#))
                .unwrap();
        }
    };

    let method = json.get("method").and_then(|m| m.as_str());
    let id = json.get("id").cloned();

    // Client's reply to the server-initiated elicitation/create (no method).
    if method.is_none() && (json.get("result").is_some() || json.get("error").is_some()) {
        state.responses.lock().unwrap().push(json.clone());
        state.response_received.notify_one();
        return Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Body::from(""))
            .unwrap();
    }

    match method {
        Some("initialize") => json_response(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {}, "elicitation": {} },
                "serverInfo": { "name": "dscc-mock", "version": "0.0.1" },
            },
        }), Some(&state.session_id)),
        Some(m) if m.starts_with("notifications/") => Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Body::from(""))
            .unwrap(),
        Some("tools/list") => json_response(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [{
                    "name": state.tool_name,
                    "description": "A tool that elicits on the standalone GET stream",
                    "inputSchema": { "type": "object", "properties": {} },
                }]
            }
        }), None),
        Some("tools/call") => {
            // Push elicitation/create onto the open GET stream, then block until
            // the client answers — finally return the result as plain JSON.
            let tool_call_id = id.clone().unwrap_or(serde_json::Value::Null);
            let server_req_id = {
                let mut g = state.next_server_request_id.lock().unwrap();
                let v = *g;
                *g += 1;
                v
            };
            let elicit = serde_json::json!({
                "jsonrpc": "2.0",
                "id": server_req_id,
                "method": "elicitation/create",
                "params": {
                    "message": state.message,
                    "requestedSchema": state.requested_schema,
                }
            });

            // Wait (bounded) for the GET stream to be subscribed, then broadcast.
            for _ in 0..100 {
                if state.elicit_tx.receiver_count() > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let _ = state.elicit_tx.send(elicit.to_string());

            // Block until the client POSTs its reply (bounded so tests don't hang).
            let _ = tokio::time::timeout(
                state.response_timeout,
                state.response_received.notified(),
            )
            .await;

            json_response(serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_call_id,
                "result": { "content": state.tool_result_content, "isError": false }
            }), None)
        }
        Some(other) => json_response(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {}", other) },
        }), None),
        None => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(r#"{"error":"missing method"}"#))
            .unwrap(),
    }
}

async fn handle_delete(State(_state): State<Arc<State_>>) -> StatusCode {
    StatusCode::OK
}

fn json_response(body: serde_json::Value, session_id: Option<&str>) -> Response {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json");
    if let Some(sid) = session_id {
        builder = builder.header("MCP-Session-Id", sid);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}
