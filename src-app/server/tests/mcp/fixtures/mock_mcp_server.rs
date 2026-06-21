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
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use base64::Engine;
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
    /// Emit a verbatim SSE body (the caller controls full framing — `id:`,
    /// `event:`, `data:`, blank-line terminators). Used for resumability tests
    /// where a stream must carry SSE event ids and may end after a priming
    /// event WITHOUT a result (simulating a mid-call disconnect that the client
    /// resumes via `Last-Event-Id`).
    SseRaw(String),
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
    /// Programmed responses for a **resume** GET request (one that carries
    /// `Last-Event-Id`, dropping the resume to the stored-event replay
    /// semantics in MCP spec § Transports/Resumability). FIFO. Empty → 405.
    get_responses_resume: Mutex<Vec<MockResponse>>,
    /// Programmed responses for the **standalone** GET-SSE the client opens
    /// after `initialized` (no `Last-Event-Id` header). Separated from the
    /// resume queue so a test driving resumability can't have its replay
    /// stolen by the connect-time standalone-stream pop. FIFO. Empty → 405.
    get_responses_standalone: Mutex<Vec<MockResponse>>,
    /// OAuth (Phase 4): when true, JSON-RPC POSTs without a valid Bearer get
    /// 401 + `WWW-Authenticate`, driving the client's client_credentials flow.
    require_oauth: Mutex<bool>,
    /// The bearer the `/token` endpoint issues and that protected POSTs accept.
    oauth_access_token: Mutex<String>,
    /// If set, `/token` validates the HTTP Basic `client_id:client_secret`.
    oauth_expected_client: Mutex<Option<(String, String)>>,
    /// One-shot: force the very next GET-SSE request to return 401 + a
    /// `WWW-Authenticate` challenge, then clear. Used to drive the client's
    /// "refresh on 401 mid-stream" code path without inventing a way to
    /// invalidate the cached bearer from the test side.
    force_401_on_next_get: Mutex<bool>,
    /// Own base URL (e.g. `http://127.0.0.1:PORT/`) for building the absolute
    /// `resource_metadata` URL in the `WWW-Authenticate` challenge.
    base_url: String,
    /// Byte-download fixtures served at `GET /download/{name}`, keyed by name.
    /// Used by the workflow `tool`-step `resource_link is_saved:false` path —
    /// the tool returns a `resource_link` whose `uri` points at this route and
    /// `persist_links` fetches the bytes over HTTP. Value is `(content_type,
    /// bytes)`.
    downloads: Mutex<HashMap<String, (String, Vec<u8>)>>,
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
        // Bind first so the state can carry its own absolute base URL (needed
        // for the OAuth `resource_metadata` challenge URL).
        let listener = TcpListener::bind("127.0.0.1:0").await
            .expect("bind mock server");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}/", port);

        let state = Arc::new(MockState {
            responses: Mutex::new(HashMap::new()),
            received: Mutex::new(Vec::new()),
            session_id_to_assign: Mutex::new(Some("mock-session-1".to_string())),
            require_protocol_version_header: Mutex::new(false),
            next_request_returns_404: Mutex::new(false),
            get_responses_resume: Mutex::new(Vec::new()),
            get_responses_standalone: Mutex::new(Vec::new()),
            require_oauth: Mutex::new(false),
            oauth_access_token: Mutex::new("mock-access-token".to_string()),
            oauth_expected_client: Mutex::new(None),
            force_401_on_next_get: Mutex::new(false),
            base_url: base_url.clone(),
            downloads: Mutex::new(HashMap::new()),
        });

        let app = Router::new()
            .route("/", post(handle_post).delete(handle_delete).get(handle_get))
            .route("/.well-known/oauth-protected-resource", get(handle_prm))
            .route("/.well-known/oauth-authorization-server", get(handle_as_metadata))
            .route("/token", post(handle_token))
            .route("/download/{name}", get(handle_download))
            .with_state(state.clone());

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

    /// Register a byte-download fixture served at `GET /download/{name}`.
    /// The workflow `tool`-step `resource_link is_saved:false` path fetches
    /// these over HTTP via `persist_links`. Returns nothing; use
    /// [`download_url`](Self::download_url) to build the `resource_link` `uri`.
    pub fn on_download(&self, name: &str, content_type: &str, bytes: &[u8]) {
        self.state
            .downloads
            .lock()
            .unwrap()
            .insert(name.to_string(), (content_type.to_string(), bytes.to_vec()));
    }

    /// Absolute URL of a registered download fixture
    /// (`http://127.0.0.1:PORT/download/{name}`). Use this as the `uri` of a
    /// `resource_link` content block so the dispatcher fetches the bytes.
    pub fn download_url(&self, name: &str) -> String {
        format!("{}download/{}", self.base_url, name)
    }

    /// Queue a response for a **resume** GET (one that carries
    /// `Last-Event-Id`). Kept as a convenience alias for the legacy single-
    /// queue API — every existing call site is a resume test.
    pub fn on_get(&self, response: MockResponse) {
        self.on_get_resume(response);
    }

    /// Queue a response for a resume GET (carries `Last-Event-Id`). Stored in
    /// a queue separate from the standalone-GET queue so the two flows can't
    /// steal each other's responses. FIFO; empty → 405.
    pub fn on_get_resume(&self, response: MockResponse) {
        self.state.get_responses_resume.lock().unwrap().push(response);
    }

    /// Queue a response for a **standalone** GET-SSE (no `Last-Event-Id`).
    /// FIFO; empty → 405 (the spec-conformant "no GET stream offered" signal,
    /// which the client tolerates silently).
    pub fn on_get_standalone(&self, response: MockResponse) {
        self.state.get_responses_standalone.lock().unwrap().push(response);
    }

    /// Require OAuth: JSON-RPC POSTs without `Authorization: Bearer <token>`
    /// get a 401 + `WWW-Authenticate` challenge; the co-located `/token`
    /// endpoint issues `access_token` to a client presenting the matching
    /// HTTP Basic `client_id:client_secret`.
    pub fn enable_oauth(&self, client_id: &str, client_secret: &str, access_token: &str) {
        *self.state.require_oauth.lock().unwrap() = true;
        *self.state.oauth_access_token.lock().unwrap() = access_token.to_string();
        *self.state.oauth_expected_client.lock().unwrap() =
            Some((client_id.to_string(), client_secret.to_string()));
    }

    /// Set the session ID the mock will return on initialize.
    pub fn set_session_id(&self, id: Option<&str>) {
        *self.state.session_id_to_assign.lock().unwrap() = id.map(|s| s.to_string());
    }

    /// Force the next GET-SSE request to return 401 + a `WWW-Authenticate`
    /// challenge (one-shot — flips back after firing). Used to drive the
    /// client's "refresh OAuth on 401 mid-stream" code path without
    /// inventing a way to invalidate the cached bearer from the test side.
    pub fn arm_401_on_next_get(&self) {
        *self.state.force_401_on_next_get.lock().unwrap() = true;
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

    // OAuth gate: without a valid Bearer, challenge with 401 + WWW-Authenticate
    // (drives the client's client_credentials flow).
    if *state.require_oauth.lock().unwrap() {
        let want = format!("Bearer {}", state.oauth_access_token.lock().unwrap());
        let ok = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == want)
            .unwrap_or(false);
        if !ok {
            let prm = format!("{}.well-known/oauth-protected-resource", state.base_url);
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(
                    "WWW-Authenticate",
                    format!("Bearer resource_metadata=\"{prm}\", scope=\"mcp\""),
                )
                .body(Body::from(""))
                .unwrap();
        }
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

async fn handle_get(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    // Record the GET (so tests can assert a resume carried `Last-Event-Id`).
    let header_map: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    state.received.lock().unwrap().push(ReceivedRequest {
        method: "__get_sse".to_string(),
        id: None,
        headers: header_map,
        body: serde_json::Value::Null,
    });

    // One-shot 401 — armed by `arm_401_on_next_get` to drive the client's
    // mid-stream OAuth-refresh path. Returns a spec-shaped challenge so the
    // client's `auth::obtain_token_from_challenge` finds the PRM URL.
    {
        let mut g = state.force_401_on_next_get.lock().unwrap();
        if *g {
            *g = false;
            let prm =
                format!("{}.well-known/oauth-protected-resource", state.base_url);
            let www = format!("Bearer resource_metadata=\"{prm}\"");
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("WWW-Authenticate", www)
                .body(Body::from(""))
                .unwrap();
        }
    }

    // Route by `Last-Event-Id` presence: a resume GET (with the header)
    // pulls from the resume queue; a standalone GET (no header) pulls from
    // the standalone queue. Empty → 405 (the spec-conformant default the
    // client must tolerate as a silent no-op).
    let has_resume_header = headers.contains_key("last-event-id");
    let programmed = if has_resume_header {
        state.get_responses_resume.lock().unwrap().pop_if_nonempty()
    } else {
        state
            .get_responses_standalone
            .lock()
            .unwrap()
            .pop_if_nonempty()
    };
    match programmed {
        Some(r) => render(r, None, state.clone()),
        None => Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from(""))
            .unwrap(),
    }
}

/// RFC 9728 Protected-Resource Metadata — points the client at the (co-located)
/// authorization server.
async fn handle_prm(State(state): State<Arc<MockState>>) -> Response {
    let body = serde_json::json!({
        "resource": state.base_url,
        "authorization_servers": [state.base_url],
    });
    json_response(body)
}

/// RFC 8414 Authorization-Server Metadata — advertises the token endpoint.
async fn handle_as_metadata(State(state): State<Arc<MockState>>) -> Response {
    let body = serde_json::json!({
        "issuer": state.base_url,
        "token_endpoint": format!("{}token", state.base_url),
        "grant_types_supported": ["client_credentials", "refresh_token"],
    });
    json_response(body)
}

/// OAuth token endpoint — validates HTTP Basic client auth + the grant, then
/// issues the configured access token.
async fn handle_token(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let header_map = headers
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    state.received.lock().unwrap().push(ReceivedRequest {
        method: "__token".to_string(),
        id: None,
        headers: header_map,
        body: serde_json::Value::String(body.clone()),
    });

    // Validate HTTP Basic client authentication if a client is configured.
    if let Some((cid, csec)) = state.oauth_expected_client.lock().unwrap().clone() {
        let want = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{cid}:{csec}"))
        );
        let got = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
        if got != want {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"error":"invalid_client"}"#))
                .unwrap();
        }
    }

    // Accept client_credentials (and refresh_token) grants.
    if !body.contains("grant_type=client_credentials") && !body.contains("grant_type=refresh_token")
    {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"error":"unsupported_grant_type"}"#))
            .unwrap();
    }

    let token = state.oauth_access_token.lock().unwrap().clone();
    json_response(serde_json::json!({
        "access_token": token,
        "token_type": "Bearer",
        "expires_in": 3600,
    }))
}

/// Serve a registered byte-download fixture (the `resource_link is_saved:false`
/// fetch target). 404 if the name isn't registered.
async fn handle_download(
    State(state): State<Arc<MockState>>,
    Path(name): Path<String>,
) -> Response {
    let entry = state.downloads.lock().unwrap().get(&name).cloned();
    match entry {
        Some((content_type, bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type)
            .body(Body::from(bytes))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(""))
            .unwrap(),
    }
}

/// Build a 200 application/json response from a JSON value.
fn json_response(body: serde_json::Value) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Small helper: pop the front of a queue if non-empty.
trait PopIfNonEmpty {
    fn pop_if_nonempty(&mut self) -> Option<MockResponse>;
}
impl PopIfNonEmpty for Vec<MockResponse> {
    fn pop_if_nonempty(&mut self) -> Option<MockResponse> {
        if self.is_empty() { None } else { Some(self.remove(0)) }
    }
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
        MockResponse::SseRaw(body) => {
            // Verbatim SSE body — caller controls `id:`/`event:`/`data:` framing.
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

