//! Programmable in-process mock MCP server for testing error paths.
//!
//! Unlike `EverythingServer` (real reference implementation, only happy
//! paths), this is a small axum server that can be programmed to return
//! specific responses per request: JSON-RPC errors, malformed bodies,
//! wrong content-types, specific HTTP statuses, SSE streams with
//! notifications interleaved, etc.
//!
//! Usage:
//! ```ignore
//! let mock = MockMcpServer::start().await;
//! // Configure responses keyed by JSON-RPC method:
//! mock.on_method("tools/list", MockResponse::json_rpc_error(-32603, "boom"));
//! // Then point HttpMcpClient at mock.base_url() and exercise it.
//! ```

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
    Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// A single mock response for a given method.
#[derive(Clone)]
pub enum MockResponse {
    /// Return JSON-RPC success result. The id is filled in from the request.
    JsonOk(serde_json::Value),
    /// Return JSON-RPC error. Status will be 200 (errors are in-body per spec).
    JsonRpcError { code: i64, message: String },
    /// Return raw body with a specific HTTP status and content-type.
    Raw { status: u16, content_type: &'static str, body: String },
    /// Return an SSE stream with the given events (each gets `data: ` prefix +
    /// `\n\n` terminator). Useful for testing notification-before-response.
    SseStream(Vec<String>),
    /// Return HTTP 202 Accepted with empty body (success for notifications).
    Accepted,
    /// Sleep then return JsonOk — for timeout/cancellation tests.
    #[allow(dead_code)]
    DelayedJsonOk { delay_ms: u64, value: serde_json::Value },
}

/// Server state — atomic counters + the programmed responses.
struct MockState {
    /// Per-method response queues. Pop from the front each time the
    /// method is called; if empty, fall back to a default OK response.
    responses: Mutex<HashMap<String, Vec<MockResponse>>>,
    /// Methods received (in order), for assertion after-the-fact.
    received: Mutex<Vec<ReceivedRequest>>,
    /// If set, force this session id on the initialize response.
    session_id_to_assign: Mutex<Option<String>>,
    /// Whether to reject requests missing MCP-Protocol-Version with 400.
    require_protocol_version_header: Mutex<bool>,
    /// Whether to reject the next request with HTTP 404 (used for
    /// session-recovery tests).
    next_request_returns_404: Mutex<bool>,
}

#[derive(Debug, Clone)]
pub struct ReceivedRequest {
    pub method: String,
    pub id: Option<i64>,
    #[allow(dead_code)]
    pub headers: HashMap<String, String>,
    #[allow(dead_code)]
    pub body: serde_json::Value,
}

pub struct MockMcpServer {
    state: Arc<MockState>,
    base_url: String,
    handle: Option<JoinHandle<()>>,
}

impl MockMcpServer {
    /// Start the mock on a random port. Returns once it's bound.
    pub async fn start() -> Self {
        let state = Arc::new(MockState {
            responses: Mutex::new(HashMap::new()),
            received: Mutex::new(Vec::new()),
            session_id_to_assign: Mutex::new(Some("mock-session-1".to_string())),
            require_protocol_version_header: Mutex::new(false),
            next_request_returns_404: Mutex::new(false),
        });

        let app = Router::new()
            .route("/", post(handle_post).delete(handle_delete).get(handle_get))
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await
            .expect("bind mock server");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}/", port);

        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // Brief settle
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        Self { state, base_url, handle: Some(handle) }
    }

    pub fn base_url(&self) -> String {
        self.base_url.clone()
    }

    /// Queue a response for the given method. Multiple calls to the same
    /// method consume responses in FIFO order.
    pub fn on_method(&self, method: &str, response: MockResponse) {
        self.state.responses.lock().unwrap()
            .entry(method.to_string())
            .or_default()
            .push(response);
    }

    /// Set the session ID the mock will return on initialize.
    pub fn set_session_id(&self, id: Option<&str>) {
        *self.state.session_id_to_assign.lock().unwrap() = id.map(|s| s.to_string());
    }

    /// Make the next request fail with HTTP 404 (one-shot — flips back
    /// after firing). Used to test the 404→reinit recovery path.
    pub fn arm_404_once(&self) {
        *self.state.next_request_returns_404.lock().unwrap() = true;
    }

    /// After the test exercises the client, inspect what we received.
    pub fn received(&self) -> Vec<ReceivedRequest> {
        self.state.received.lock().unwrap().clone()
    }

    /// How many requests for a given method.
    pub fn count_for(&self, method: &str) -> usize {
        self.received().iter().filter(|r| r.method == method).count()
    }
}

impl Drop for MockMcpServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() { h.abort(); }
    }
}

// ─── Axum handlers ──────────────────────────────────────────────────────────

async fn handle_post(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    // Optionally reject requests missing MCP-Protocol-Version
    if *state.require_protocol_version_header.lock().unwrap()
        && !headers.contains_key("mcp-protocol-version")
    {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(r#"{"error":"missing MCP-Protocol-Version header"}"#))
            .unwrap();
    }

    // One-shot 404 (for session-recovery test)
    {
        let mut guard = state.next_request_returns_404.lock().unwrap();
        if *guard {
            *guard = false;
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(""))
                .unwrap();
        }
    }

    // Parse request body
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}"#))
                .unwrap();
        }
    };

    let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
    let id = json.get("id").and_then(|v| v.as_i64());

    // Record the request
    let header_map = headers.iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    state.received.lock().unwrap().push(ReceivedRequest {
        method: method.clone(),
        id,
        headers: header_map,
        body: json.clone(),
    });

    // Default initialize response if not overridden — needed for any test
    // that calls connect() and doesn't program a custom init.
    if method == "initialize" {
        if let Some(custom) = state.responses.lock().unwrap()
            .get_mut("initialize")
            .and_then(|v| if v.is_empty() { None } else { Some(v.remove(0)) })
        {
            return render(custom, id, state.clone());
        }
        let sid = state.session_id_to_assign.lock().unwrap().clone();
        let mut resp = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json");
        if let Some(s) = sid {
            resp = resp.header("MCP-Session-Id", s);
        }
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                "serverInfo": { "name": "mock-mcp", "version": "0.0.1" },
            },
        });
        return resp.body(Body::from(body.to_string())).unwrap();
    }

    // notifications/* always get 202 Accepted unless overridden
    if method.starts_with("notifications/") {
        if let Some(custom) = state.responses.lock().unwrap()
            .get_mut(&method)
            .and_then(|v| if v.is_empty() { None } else { Some(v.remove(0)) })
        {
            return render(custom, id, state.clone());
        }
        return Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Body::from(""))
            .unwrap();
    }

    // Look up programmed response for this method
    let response = state.responses.lock().unwrap()
        .get_mut(&method)
        .and_then(|v| if v.is_empty() { None } else { Some(v.remove(0)) });

    if let Some(r) = response {
        return render(r, id, state.clone());
    }

    // Default: -32601 Method not found
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": format!("Method not found: {}", method) },
    });
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn handle_delete(State(state): State<Arc<MockState>>) -> StatusCode {
    state.received.lock().unwrap().push(ReceivedRequest {
        method: "__delete_session".to_string(),
        id: None,
        headers: HashMap::new(),
        body: serde_json::Value::Null,
    });
    StatusCode::OK
}

async fn handle_get() -> StatusCode {
    // We don't implement server-initiated GET-SSE stream in the mock.
    StatusCode::METHOD_NOT_ALLOWED
}

fn render(response: MockResponse, id: Option<i64>, state: Arc<MockState>) -> Response {
    match response {
        MockResponse::JsonOk(result) => {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result,
            });
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        }
        MockResponse::JsonRpcError { code, message } => {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message },
            });
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        }
        MockResponse::Raw { status, content_type, body } => {
            Response::builder()
                .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
                .header("Content-Type", content_type)
                .body(Body::from(body))
                .unwrap()
        }
        MockResponse::SseStream(events) => {
            let mut body = String::new();
            for event in events {
                // Substitute the placeholder __ID__ with the actual request id
                // so SseStream tests can include the response with correct id
                let event = event.replace("__ID__", &id.map(|i| i.to_string()).unwrap_or_else(|| "null".to_string()));
                for line in event.lines() {
                    body.push_str("data: ");
                    body.push_str(line);
                    body.push('\n');
                }
                body.push('\n');
            }
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/event-stream")
                .body(Body::from(body))
                .unwrap()
        }
        MockResponse::Accepted => {
            Response::builder()
                .status(StatusCode::ACCEPTED)
                .body(Body::from(""))
                .unwrap()
        }
        MockResponse::DelayedJsonOk { delay_ms, value } => {
            // We can't easily await inside this sync fn — caller would have
            // to make this whole function async. Skip for now; if we need
            // timeout tests we'll route differently.
            let _ = (delay_ms, state); // silence unused
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": value,
            });
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        }
    }
}

