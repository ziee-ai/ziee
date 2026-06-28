//! In-process mock HTTP MCP server with sampling support.
//!
//! Implements the MCP HTTP+SSE transport protocol and exposes a single `research` tool
//! that makes 2 sequential sampling requests (LLM calls back into our server) before
//! returning the final answer. Used by sampling integration tests.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use tokio_stream::wrappers::ReceiverStream;

// ============================================================================
// Mock behavior variants
// ============================================================================

/// Configures how the mock server handles tool calls.
pub enum MockBehavior {
    /// Two sequential sampling requests, then returns result (default behavior)
    Normal,
    /// Fires sampling request #1, then drops the response channel without ever sending a result.
    /// Simulates Ziee not responding — tests that the system degrades gracefully (BUG-10).
    DropFirstResponse,
    /// Sends an Image content type in the first sampling request.
    /// Simulates an MCP server that passes a screenshot/file to the LLM for analysis.
    /// The image is a 1×1 white PNG (69 bytes) — the minimum valid PNG that won't be
    /// rejected by LLM APIs for being malformed. We're testing the pipeline, not the LLM's
    /// ability to describe images.
    SendImageContent,
}

/// A valid 1×1 white PNG encoded as base64 (69 bytes).
/// Used by MockBehavior::SendImageContent to inject Image content into sampling requests.
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwADhQGAWjR9awAAAABJRU5ErkJggg==";

// ============================================================================
// Shared State
// ============================================================================

struct MockState {
    /// Pending sampling requests, keyed by JSON-RPC request id.
    pending_sampling: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    /// Monotonic counter for sampling request IDs (starts at 100 to avoid
    /// collision with JSON-RPC id=1 used for tool calls).
    next_id: AtomicU64,
    /// How many sampling/createMessage requests were processed end-to-end.
    sampling_call_count: AtomicU64,
    /// The result text returned by Ziee for each sampling call, in order.
    sampling_results: Mutex<Vec<String>>,
    /// Whether each sampling response from Ziee was structurally valid per the MCP spec.
    /// Valid means: role="assistant", content is present, model is a non-empty string.
    sampling_results_valid: Mutex<Vec<bool>>,
    /// Configured mock behavior for tool calls.
    behavior: MockBehavior,
}

// ============================================================================
// Public API
// ============================================================================

pub struct MockSamplingServer {
    pub port: u16,
    state: Arc<MockState>,
    /// Dropping this field sends the shutdown signal to the axum server.
    _shutdown_tx: oneshot::Sender<()>,
}

impl MockSamplingServer {
    /// Start with normal two-sequential-sampling-requests behavior.
    pub async fn start() -> Self {
        Self::start_with_behavior(MockBehavior::Normal).await
    }

    /// Start with a specific behavior for tool call handling.
    pub async fn start_with_behavior(behavior: MockBehavior) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind mock MCP server");
        let port = listener.local_addr().unwrap().port();

        let state = Arc::new(MockState {
            pending_sampling: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(100),
            sampling_call_count: AtomicU64::new(0),
            sampling_results: Mutex::new(Vec::new()),
            sampling_results_valid: Mutex::new(Vec::new()),
            behavior,
        });

        let app = Router::new()
            .route("/", post(dispatch_handler))
            .with_state(state.clone());

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.await.ok();
                })
                .await
                .expect("Mock MCP server crashed");
        });

        Self {
            port,
            state,
            _shutdown_tx: shutdown_tx,
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Total number of sampling/createMessage requests completed (Ziee responded to each).
    pub fn sampling_call_count(&self) -> u64 {
        self.state.sampling_call_count.load(Ordering::SeqCst)
    }

    /// Texts returned by Ziee for each sampling call, in order.
    pub async fn sampling_results(&self) -> Vec<String> {
        self.state.sampling_results.lock().await.clone()
    }

    /// Whether each sampling response from Ziee was structurally valid per the MCP spec.
    /// Each entry corresponds to one sampling call (in order). `true` means the response
    /// had role="assistant", a content field, and a non-empty model string.
    pub async fn sampling_results_valid(&self) -> Vec<bool> {
        self.state.sampling_results_valid.lock().await.clone()
    }
}

// ============================================================================
// Route Handler
// ============================================================================

async fn dispatch_handler(
    State(state): State<Arc<MockState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let body_json: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("[MockMCP] Body parse error: {}", e);
            return Response::builder()
                .status(400)
                .body(Body::empty())
                .unwrap();
        }
    };

    tracing::debug!("[MockMCP] ← {}", body_json);

    match body_json.get("method").and_then(|m| m.as_str()) {
        Some("initialize") => handle_initialize(),
        Some("notifications/initialized") => handle_empty_ok(),
        Some("tools/list") => handle_tools_list(&body_json),
        Some("tools/call") => {
            let accept = headers
                .get("accept")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if accept.contains("text/event-stream") {
                handle_tool_call_sse(state, &body_json).await
            } else {
                // Research tool requires SSE streaming (sampling). Return a clear error
                // instead of falling back to the tools/list handler.
                let id = body_json.get("id").cloned().unwrap_or(json!(1));
                let result = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{"type": "text", "text": "Error: research tool requires a client with sampling/SSE capability (Accept: text/event-stream)"}],
                        "is_error": true
                    }
                });
                Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .header("mcp-session-id", "mock-session-1")
                    .body(Body::from(result.to_string()))
                    .unwrap()
            }
        }
        None if body_json.get("result").is_some() => {
            // JSON-RPC response: the client is POSTing back a sampling result
            handle_sampling_response(state, &body_json).await
        }
        _ => handle_empty_ok(),
    }
}

// ============================================================================
// Individual Handlers
// ============================================================================

fn handle_initialize() -> Response<Body> {
    let result = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"sampling": {}},
            "serverInfo": {"name": "mock-sampling-server", "version": "0.1.0"}
        }
    });
    tracing::debug!("[MockMCP] → initialize response");
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .header("mcp-session-id", "mock-session-1")
        .body(Body::from(result.to_string()))
        .unwrap()
}

fn handle_empty_ok() -> Response<Body> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .header("mcp-session-id", "mock-session-1")
        .body(Body::from("{}"))
        .unwrap()
}

fn handle_tools_list(body: &Value) -> Response<Body> {
    let id = body.get("id").cloned().unwrap_or(json!(1));
    let result = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [{
                "name": "research",
                "description": "Research a query using two sequential LLM sampling calls",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "The research question"}
                    },
                    "required": ["query"]
                }
            }]
        }
    });
    tracing::debug!("[MockMCP] → tools/list response");
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .header("mcp-session-id", "mock-session-1")
        .body(Body::from(result.to_string()))
        .unwrap()
}

async fn handle_tool_call_sse(state: Arc<MockState>, body: &Value) -> Response<Body> {
    let query = body
        .get("params")
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.get("query"))
        .and_then(|q| q.as_str())
        .unwrap_or("unknown query")
        .to_string();

    tracing::debug!("[MockMCP] → tool call SSE for query: {}", query);

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(16);

    // Dispatch to the appropriate behavior handler
    match state.behavior {
        MockBehavior::Normal => {
            tokio::spawn(tool_call_normal(state, query, tx));
        }
        MockBehavior::DropFirstResponse => {
            tokio::spawn(tool_call_drop_first_response(state, query, tx));
        }
        MockBehavior::SendImageContent => {
            tokio::spawn(tool_call_send_image_content(state, query, tx));
        }
    }

    Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("mcp-session-id", "mock-session-1")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
}

// ============================================================================
// Tool call behavior implementations
// ============================================================================

/// Normal behavior: 2 sequential sampling requests, then return result.
async fn tool_call_normal(
    state: Arc<MockState>,
    query: String,
    tx: tokio::sync::mpsc::Sender<Result<Bytes, Infallible>>,
) {
    // === Sampling request #1: ask the LLM to answer the query directly ===
    let id1 = state.next_id.fetch_add(1, Ordering::SeqCst);
    let (res_tx1, res_rx1) = oneshot::channel::<Value>();
    state.pending_sampling.lock().await.insert(id1, res_tx1);

    let req1 = json!({
        "jsonrpc": "2.0",
        "id": id1,
        "method": "sampling/createMessage",
        "params": {
            "messages": [{
                "role": "user",
                "content": {
                    "type": "text",
                    "text": format!("Answer this question: {}", query)
                }
            }],
            "maxTokens": 500
        }
    });
    tracing::debug!("[MockMCP] → sampling request #1 (id={})", id1);
    let _ = tx.send(Ok(Bytes::from(format!("data: {}\n\n", req1)))).await;

    let result1 = match tokio::time::timeout(Duration::from_secs(30), res_rx1).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => {
            tracing::warn!("[MockMCP] Sampling channel dropped for request #1 (Ziee disconnected?)");
            let _ = tx.send(Ok(Bytes::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32000,\"message\":\"sampling channel dropped\"}}\n\n"
            ))).await;
            return;
        }
        Err(_) => {
            tracing::warn!("[MockMCP] Timeout waiting for sampling response #1");
            let _ = tx.send(Ok(Bytes::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32000,\"message\":\"sampling timeout\"}}\n\n"
            ))).await;
            return;
        }
    };

    let text1 = result1
        .get("content")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    tracing::debug!("[MockMCP] ← sampling result #1: {}", text1);

    // Record result #1
    state.sampling_call_count.fetch_add(1, Ordering::SeqCst);
    state.sampling_results.lock().await.push(text1.clone());

    // === Sampling request #2: summarize the answer ===
    let id2 = state.next_id.fetch_add(1, Ordering::SeqCst);
    let (res_tx2, res_rx2) = oneshot::channel::<Value>();
    state.pending_sampling.lock().await.insert(id2, res_tx2);

    let req2 = json!({
        "jsonrpc": "2.0",
        "id": id2,
        "method": "sampling/createMessage",
        "params": {
            "messages": [{
                "role": "user",
                "content": {
                    "type": "text",
                    "text": format!("Summarize in one sentence: {}", text1)
                }
            }],
            "maxTokens": 100
        }
    });
    tracing::debug!("[MockMCP] → sampling request #2 (id={})", id2);
    let _ = tx.send(Ok(Bytes::from(format!("data: {}\n\n", req2)))).await;

    let result2 = match tokio::time::timeout(Duration::from_secs(30), res_rx2).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => {
            tracing::warn!("[MockMCP] Sampling channel dropped for request #2");
            let _ = tx.send(Ok(Bytes::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32000,\"message\":\"sampling channel dropped\"}}\n\n"
            ))).await;
            return;
        }
        Err(_) => {
            tracing::warn!("[MockMCP] Timeout waiting for sampling response #2");
            let _ = tx.send(Ok(Bytes::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32000,\"message\":\"sampling timeout\"}}\n\n"
            ))).await;
            return;
        }
    };

    let text2 = result2
        .get("content")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    tracing::debug!("[MockMCP] ← sampling result #2: {}", text2);

    // Record result #2
    state.sampling_call_count.fetch_add(1, Ordering::SeqCst);
    state.sampling_results.lock().await.push(text2.clone());

    // === Final tool result ===
    let final_result = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{"type": "text", "text": text2}],
            "is_error": false
        }
    });
    tracing::debug!("[MockMCP] → final tool result");
    let _ = tx.send(Ok(Bytes::from(format!("data: {}\n\n", final_result)))).await;
    // tx drops here → stream closes
}

/// DropFirstResponse: fires sampling request #1, then immediately drops the response channel.
/// Simulates Ziee not responding — tests graceful degradation (BUG-10 scenario).
async fn tool_call_drop_first_response(
    state: Arc<MockState>,
    query: String,
    tx: tokio::sync::mpsc::Sender<Result<Bytes, Infallible>>,
) {
    let id1 = state.next_id.fetch_add(1, Ordering::SeqCst);
    // Register the sender but immediately drop it — Ziee's response will find no receiver
    let (res_tx1, _res_rx1_dropped) = oneshot::channel::<Value>();
    state.pending_sampling.lock().await.insert(id1, res_tx1);

    let req1 = json!({
        "jsonrpc": "2.0",
        "id": id1,
        "method": "sampling/createMessage",
        "params": {
            "messages": [{
                "role": "user",
                "content": {
                    "type": "text",
                    "text": format!("Answer this question: {}", query)
                }
            }],
            "maxTokens": 500
        }
    });
    tracing::debug!("[MockMCP] DropFirstResponse: sending sampling request #1 (id={})", id1);
    let _ = tx.send(Ok(Bytes::from(format!("data: {}\n\n", req1)))).await;

    // Drop _res_rx1_dropped and return without sending a final result.
    // The SSE channel (tx) closes naturally here — the stream ends with no tool result.
    // This tests that Ziee handles a truncated SSE stream without panicking.
    tracing::debug!("[MockMCP] DropFirstResponse: dropping response channel (stream will end without result)");
}

/// SendImageContent: fires one sampling request whose message body contains Image content.
/// Tests that the sampling handler correctly deserializes and forwards Image content to the LLM.
async fn tool_call_send_image_content(
    state: Arc<MockState>,
    _query: String,
    tx: tokio::sync::mpsc::Sender<Result<Bytes, Infallible>>,
) {
    let id1 = state.next_id.fetch_add(1, Ordering::SeqCst);
    let (res_tx1, res_rx1) = oneshot::channel::<Value>();
    state.pending_sampling.lock().await.insert(id1, res_tx1);

    // Send a sampling request with Image content type.
    // The image is a 1×1 white PNG — the minimum valid PNG. We test that the pipeline
    // doesn't crash on Image content; we don't assert anything about the LLM's response.
    let req1 = json!({
        "jsonrpc": "2.0",
        "id": id1,
        "method": "sampling/createMessage",
        "params": {
            "messages": [{
                "role": "user",
                "content": {
                    "type": "image",
                    "data": TINY_PNG_BASE64,
                    "mimeType": "image/png"
                }
            }],
            "maxTokens": 100
        }
    });
    tracing::debug!("[MockMCP] SendImageContent: sending image sampling request #1 (id={})", id1);
    let _ = tx.send(Ok(Bytes::from(format!("data: {}\n\n", req1)))).await;

    let result1 = match tokio::time::timeout(Duration::from_secs(60), res_rx1).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => {
            tracing::warn!("[MockMCP] Image sampling channel dropped");
            let _ = tx.send(Ok(Bytes::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32000,\"message\":\"image sampling channel dropped\"}}\n\n"
            ))).await;
            return;
        }
        Err(_) => {
            tracing::warn!("[MockMCP] Timeout waiting for image sampling response");
            let _ = tx.send(Ok(Bytes::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32000,\"message\":\"image sampling timeout\"}}\n\n"
            ))).await;
            return;
        }
    };

    let text1 = result1
        .get("content")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    tracing::debug!("[MockMCP] ← image sampling result: {}", text1);

    // Record result
    state.sampling_call_count.fetch_add(1, Ordering::SeqCst);
    state.sampling_results.lock().await.push(text1.clone());

    // Return the LLM's response as the tool result
    let final_result = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{"type": "text", "text": text1}],
            "is_error": false
        }
    });
    tracing::debug!("[MockMCP] SendImageContent: → final tool result");
    let _ = tx.send(Ok(Bytes::from(format!("data: {}\n\n", final_result)))).await;
}

async fn handle_sampling_response(state: Arc<MockState>, body: &Value) -> Response<Body> {
    let id = body.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
    let result = body.get("result").cloned().unwrap_or(Value::Null);

    tracing::debug!("[MockMCP] ← sampling response id={}: {}", id, result);

    // Validate the structure of the response per MCP sampling spec
    let role = result.get("role").and_then(|r| r.as_str()).unwrap_or("");
    let has_content = result.get("content").is_some();
    let has_model = result.get("model")
        .and_then(|m| m.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let is_valid = role == "assistant" && has_content && has_model;
    state.sampling_results_valid.lock().await.push(is_valid);

    if !is_valid {
        tracing::warn!(
            "[MockMCP] Sampling response id={} failed validation: role={:?}, has_content={}, has_model={}",
            id, role, has_content, has_model
        );
    }

    let sender = state.pending_sampling.lock().await.remove(&id);
    if let Some(tx) = sender {
        let _ = tx.send(result);
    } else {
        tracing::warn!("[MockMCP] No pending sampling request for id={}", id);
    }

    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from("{}"))
        .unwrap()
}
