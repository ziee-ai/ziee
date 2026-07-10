//! In-process mock HTTP MCP server that impersonates the desktop `office_bridge`.
//!
//! The real `office_bridge` MCP server is **desktop-only** (it brokers to a live
//! Office task pane), so it is not registrable in the server integration harness.
//! But the server-side approval decision (`compute_needs_approval`) keys ONLY on
//! the MCP server *id* + the tool name + the `mode` argument — never on the pane.
//! So a plain HTTP MCP server that advertises `run_office_js` and `list_open_documents`,
//! registered under the deterministic `office_bridge_mcp_server_id()`, drives the
//! exact read→auto-run / write→approval path a real desktop pane would, end-to-end
//! through the real chat→MCP→approval loop — no desktop, no Excel.
//!
//! `tools/call` returns a canned success and records each invocation (tool name +
//! the `mode` the model declared), so a test can assert whether the tool actually
//! executed (read / approved) or was withheld pending approval (write / denied).

use std::convert::Infallible;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

/// One recorded `tools/call` invocation.
#[derive(Clone, Debug)]
pub struct RecordedCall {
    pub tool_name: String,
    /// The `mode` argument the model supplied on a `run_office_js` call (None for
    /// `list_open_documents`, or if the model omitted it).
    pub mode: Option<String>,
}

struct MockState {
    /// Every `tools/call` the server actually executed, in order.
    calls: Mutex<Vec<RecordedCall>>,
}

pub struct MockOfficeServer {
    pub port: u16,
    state: Arc<MockState>,
    /// Dropping this field shuts the axum server down.
    _shutdown_tx: oneshot::Sender<()>,
}

impl MockOfficeServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind mock office MCP server");
        let port = listener.local_addr().unwrap().port();

        let state = Arc::new(MockState {
            calls: Mutex::new(Vec::new()),
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
                .expect("Mock office MCP server crashed");
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

    /// Every `tools/call` the mock actually executed, in order.
    pub async fn calls(&self) -> Vec<RecordedCall> {
        self.state.calls.lock().await.clone()
    }

    /// Number of `run_office_js` calls the mock actually executed.
    pub async fn run_office_js_call_count(&self) -> usize {
        self.state
            .calls
            .lock()
            .await
            .iter()
            .filter(|c| c.tool_name == "run_office_js")
            .count()
    }
}

async fn dispatch_handler(
    State(state): State<Arc<MockState>>,
    body: Bytes,
) -> Response<Body> {
    let body_json: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("[MockOffice] Body parse error: {}", e);
            return Response::builder().status(400).body(Body::empty()).unwrap();
        }
    };

    tracing::debug!("[MockOffice] ← {}", body_json);

    let result = match body_json.get("method").and_then(|m| m.as_str()) {
        Some("initialize") => handle_initialize(&body_json),
        Some("notifications/initialized") => return empty_ok(),
        Some("tools/list") => handle_tools_list(&body_json),
        Some("tools/call") => handle_tool_call(state, &body_json).await,
        _ => return empty_ok(),
    };

    json_response(result)
}

fn handle_initialize(body: &Value) -> Value {
    let id = body.get("id").cloned().unwrap_or(json!(1));
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "serverInfo": {"name": "mock-office-bridge", "version": "0.1.0"}
        }
    })
}

/// Advertise EXACTLY the shipped office_bridge surface. The `run_office_js`
/// description is copied verbatim from the desktop `tools.rs` so the model gets
/// the same read/write guidance it gets in production — this test proves the
/// SHIPPED schema drives the model to set `mode` correctly.
fn handle_tools_list(body: &Value) -> Value {
    let id = body.get("id").cloned().unwrap_or(json!(1));
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "list_open_documents",
                    "description": "List the Microsoft Office documents (Word, Excel, PowerPoint) currently open on the user's desktop, with each document's name, full path, host application, and saved state.",
                    "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
                },
                {
                    "name": "run_office_js",
                    "description": "Run an Office.js script against one open Office document, identified by its `doc_full_name` (from list_open_documents). You write the body of an async function that receives the Office.js request `context`; it runs inside the host's Word.run / Excel.run / PowerPoint.run. On a script error a structured error is returned so you can correct and retry. Requires the document's task pane to be open.\n\n### CRITICAL: set `mode` correctly (read vs write)\nApply this rule to YOUR script BEFORE calling:\n- If the script contains ANY of these, `mode` MUST be \"write\": a property assignment (`=`, e.g. `range.values = …`), or a mutating call (`insert*`, `add`, `delete`, `remove`, `clear`, `set*`, `replace`).\n- Use \"read\" ONLY if the script exclusively navigates/loads and returns data (`getRange`/`load`/`sync`/`return`) with NO assignment and NO mutating call.\n- If in ANY doubt, use \"write\".\n\nWhy it matters: `mode:\"write\"` requires the user's approval before the script runs; `mode:\"read\"` runs immediately with no prompt.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "doc_full_name": {"type": "string", "description": "The app-qualified full name of the target document."},
                            "script": {"type": "string", "description": "The Office.js script body to run."},
                            "mode": {
                                "type": "string",
                                "enum": ["read", "write"],
                                "description": "REQUIRED. \"write\" if the script contains any property assignment or mutating call; \"read\" ONLY if it exclusively loads and returns data. A \"write\" requires user approval before it runs; a \"read\" runs without prompting. When in doubt, use \"write\"."
                            }
                        },
                        "required": ["doc_full_name", "script", "mode"]
                    }
                }
            ]
        }
    })
}

/// Execute a tool call: record the invocation, return a canned success. Because
/// this handler only runs when the loop actually dispatches the tool, a recorded
/// `run_office_js` call is proof the tool EXECUTED (i.e. was NOT withheld behind
/// an unresolved / denied approval).
async fn handle_tool_call(state: Arc<MockState>, body: &Value) -> Value {
    let id = body.get("id").cloned().unwrap_or(json!(1));
    let params = body.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let mode = args
        .get("mode")
        .and_then(Value::as_str)
        .map(str::to_string);

    state.calls.lock().await.push(RecordedCall {
        tool_name: tool_name.clone(),
        mode: mode.clone(),
    });

    let text = if tool_name == "list_open_documents" {
        json!([{
            "name": "Book1.xlsx",
            "fullName": "Book1.xlsx",
            "host": "excel",
            "saved": true
        }])
        .to_string()
    } else {
        // run_office_js — a plausible read-back result; the pane is mocked away.
        json!({"ok": true, "mode": mode}).to_string()
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{"type": "text", "text": text}],
            "isError": false
        }
    })
}

fn json_response(result: Value) -> Response<Body> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .header("mcp-session-id", "mock-office-session-1")
        .body(Body::from(result.to_string()))
        .unwrap()
}

fn empty_ok() -> Response<Body> {
    Response::builder()
        .status(202)
        .body(Body::empty())
        .unwrap()
}

// Silence an unused-import lint when the crate compiles this file without the
// integration tests referencing every symbol.
#[allow(dead_code)]
fn _assert_infallible(_: Infallible) {}
