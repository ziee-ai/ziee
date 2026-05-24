//! Standalone mock MCP server with sampling support — for manual UI testing.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use clap::Parser;
use serde_json::{json, Value};
use axum::serve::ListenerExt;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;

// ============================================================================
// CLI
// ============================================================================
#[derive(Parser)]
#[command(name = "mock-sampling-server")]
#[command(about = "Standalone mock MCP server with sampling support for UI testing")]
struct Cli {
    #[arg(long, default_value = "3456")]
    port: u16,
}

// ============================================================================
// Shared State
// ============================================================================
struct MockState {
    pending_sampling: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
}

// ============================================================================
// Main
// ============================================================================
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", cli.port))
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to port {}: {}", cli.port, e));

    let port = listener.local_addr().unwrap().port();
    let url = format!("http://0.0.0.0:{}", port);

    println!("Mock MCP Sampling Server");
    println!("========================");
    println!("Listening at: {}", url);
    println!();
    println!("To use in Ziee Chat:");
    println!(" 1. Go to Settings → System MCP Servers → Add Server");
    println!(" 2. Transport type: HTTP");
    println!(" 3. URL: {}", url);
    println!(" 4. Enable \"Supports Sampling\"");
    println!(" 5. Assign the server to your user group");
    println!(" 6. In chat, enable the server and send a message using the research tool.");
    println!();
    println!("Press Ctrl+C to stop.");
    println!();

    let state = Arc::new(MockState {
        pending_sampling: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(100),
    });

    let app = Router::new()
        .route("/", post(dispatch_handler))
        .with_state(state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener.tap_io(|s| { let _ = s.set_nodelay(true); }), app)
            .with_graceful_shutdown(async move { let _ = shutdown_rx.await; })
            .await
            .expect("Mock MCP server crashed");
    });

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("\nShutting down...");
    let _ = shutdown_tx.send(());
}

// ============================================================================
// Helpers
// ============================================================================
fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let ms = now.subsec_millis();
    format!("{:02}:{:02}:{:02}.{:03}", (secs % 86400) / 3600, (secs % 3600) / 60, secs % 60, ms)
}

// ============================================================================
// Logging helpers
// ============================================================================
fn log_headers(headers: &HeaderMap) -> String {
    headers
        .iter()
        .map(|(k, v)| format!("  {}: {}", k, v.to_str().unwrap_or("<binary>")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_json_response(label: &str, status: u16, body: Value) -> Response<Body> {
    let body_str = body.to_string();
    let response = Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("mcp-session-id", "mock-session-1")
        .body(Body::from(body_str.clone()))
        .unwrap();
    eprintln!(
        "[MockMCP {}] → {} status={}\nheaders:\n{}\nbody: {}",
        timestamp(),
        label,
        status,
        log_headers(response.headers()),
        body_str,
    );
    response
}

// ============================================================================
// Route Handler
// ============================================================================
async fn dispatch_handler(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let raw_body = String::from_utf8_lossy(&body).to_string();

    eprintln!(
        "[MockMCP {}] ← REQUEST\nheaders:\n{}\nbody: {}",
        timestamp(),
        log_headers(&headers),
        raw_body,
    );

    let body_json: Value = match serde_json::from_str(&raw_body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[MockMCP {}] Body parse error: {}", timestamp(), e);
            let response = Response::builder().status(400).body(Body::empty()).unwrap();
            eprintln!(
                "[MockMCP {}] → RESPONSE status=400\nheaders:\n{}",
                timestamp(),
                log_headers(response.headers()),
            );
            return response;
        }
    };

    

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
                let id = body_json.get("id").cloned().unwrap_or(json!(1));
                let result = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{"type": "text", "text": "Error: research tool requires SSE sampling support"}],
                        "is_error": true
                    }
                });
                build_json_response("tools/call (non-SSE error)", 200, result)
            }
        }
        None if body_json.get("result").is_some() => handle_sampling_response(state, &body_json).await,
        _ => handle_empty_ok(),
    }
}

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
    build_json_response("initialize", 200, result)
}

fn handle_empty_ok() -> Response<Body> {
    build_json_response("empty ok", 200, json!({}))
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
                    "properties": { "query": {"type": "string", "description": "The research question"} },
                    "required": ["query"]
                }
            }]
        }
    });
    build_json_response("tools/list", 200, result)
}

// ============================================================================
// SSE Tool Handler – FINAL CLEAN VERSION
// ============================================================================
async fn handle_tool_call_sse(state: Arc<MockState>, body: &Value) -> Response<Body> {
    let call_id = body.get("id").cloned().unwrap_or(json!(1));
    let query = body
        .get("params")
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.get("query"))
        .and_then(|q| q.as_str())
        .unwrap_or("unknown query")
        .to_string();

    eprintln!(
        "[MockMCP {}] → tool call SSE opening stream for query: {} (headers: Content-Type: text/event-stream, mcp-session-id: mock-session-1)",
        timestamp(), query
    );

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(16);

    tokio::spawn(async move {
        // Clean send_sse – silently ignores "channel closed" on the final result
        let send_sse = |data: Value| {
            let tx = tx.clone();
            async move {
                let json_str = data.to_string();
                let event = Event::default().event("message").data(json_str.clone());

                eprintln!("[MockMCP {}] → Sending SSE event: {}", timestamp(), json_str);

                // Ignore error (normal when the client closes the stream after the final result)
                let _ = tx.send(Ok(event)).await;
            }
        };

        // === Sampling #1 ===
        let id1 = state.next_id.fetch_add(1, Ordering::SeqCst);
        let (res_tx1, res_rx1) = oneshot::channel::<Value>();

        {
            let mut pending = state.pending_sampling.lock().await;
            pending.insert(id1, res_tx1);
        }

        let req1 = json!({
            "jsonrpc": "2.0",
            "id": id1,
            "method": "sampling/createMessage",
            "params": {
                "messages": [{
                    "role": "user",
                    "content": { "type": "text", "text": format!("Answer this question: {}", query) }
                }],
                "maxTokens": 500
            }
        });

        eprintln!("[MockMCP {}] → Preparing sampling request #1 (id={})", timestamp(), id1);
        send_sse(req1).await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let result1 = match time::timeout(Duration::from_secs(300), res_rx1).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => json!({"role":"assistant","content":{"type":"text","text":"[Request failed]"},"model":"unknown"}),
            Err(_) => {
                eprintln!("[MockMCP {}] Sampling #1 timeout after 300s", timestamp());
                json!({"role":"assistant","content":{"type":"text","text":"[Sampling request #1 timed out]"},"model":"unknown"})
            }
        };

        let text1 = result1
            .get("content")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("No answer received")
            .to_string();

        eprintln!("[MockMCP {}] ← sampling result #1: {}", timestamp(), text1);

        // === Sampling #2 ===
        let id2 = state.next_id.fetch_add(1, Ordering::SeqCst);
        let (res_tx2, res_rx2) = oneshot::channel::<Value>();

        {
            let mut pending = state.pending_sampling.lock().await;
            pending.insert(id2, res_tx2);
        }

        let req2 = json!({
            "jsonrpc": "2.0",
            "id": id2,
            "method": "sampling/createMessage",
            "params": {
                "messages": [{
                    "role": "user",
                    "content": { "type": "text", "text": format!("Summarize in one sentence: {}", text1) }
                }],
                "maxTokens": 100
            }
        });

        eprintln!("[MockMCP {}] → Preparing sampling request #2 (id={})", timestamp(), id2);
        send_sse(req2).await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let result2 = match time::timeout(Duration::from_secs(300), res_rx2).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => json!({"role":"assistant","content":{"type":"text","text":"[Request failed]"},"model":"unknown"}),
            Err(_) => {
                eprintln!("[MockMCP {}] Sampling #2 timeout after 300s", timestamp());
                json!({"role":"assistant","content":{"type":"text","text":"[Sampling request #2 timed out]"},"model":"unknown"})
            }
        };

        let text2 = result2
            .get("content")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("No summary received")
            .to_string();

        eprintln!("[MockMCP {}] ← sampling result #2: {}", timestamp(), text2);

        // === Final tool result ===
        let final_result = json!({
            "jsonrpc": "2.0",
            "id": call_id,
            "result": {
                "content": [{"type": "text", "text": text2}],
                "is_error": false
            }
        });

        eprintln!("[MockMCP {}] → Preparing final tool result", timestamp());
        send_sse(final_result).await;
    });

    let mut response = Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
        .into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("mcp-session-id"),
        axum::http::header::HeaderValue::from_static("mock-session-1"),
    );
    response.headers_mut().insert(                                                        
      axum::http::header::HeaderName::from_static("x-accel-buffering"),
      axum::http::header::HeaderValue::from_static("no"),                               
  );
    response
}

// ============================================================================
// Sampling Response Handler
// ============================================================================
async fn handle_sampling_response(state: Arc<MockState>, body: &Value) -> Response<Body> {
    let id = body.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
    let result = body.get("result").cloned().unwrap_or(Value::Null);

    eprintln!("[MockMCP {}] ← sampling response id={}: {}", timestamp(), id, result);

    let sender = {
        let mut pending = state.pending_sampling.lock().await;
        pending.remove(&id)
    };

    if let Some(tx) = sender {
        if tx.send(result).is_err() {
            eprintln!("[MockMCP {}] Warning: receiver dropped for id={}", timestamp(), id);
        } else {
            eprintln!("[MockMCP {}] Successfully delivered result for id={}", timestamp(), id);
        }
    } else {
        eprintln!("[MockMCP {}] Warning: no pending request for id={}", timestamp(), id);
    }

    build_json_response("sampling response ack", 200, json!({}))
}