// ============================================================================
// Agentic-chat integration tests (Track A files MCP + Track B inline memory).
//
// These are the audit's T0-dependent Tier-2 tests: they drive the FULL chat
// tool-use loop against the in-process **stub chat provider**
// (`crate::common::stub_chat`) — a loopback OpenAI-compatible server with
// scripted tool calls — so manifest injection, the `read_file` round-trip, the
// `enable_mcp=false` auto-attach gate, capability gating, and Track B's
// side-effect inline self-save all run end-to-end without a real LLM key.
//
// Scripting is driven by a `STUB_PLAN=` token in the user message (see
// `stub_chat.rs`). Files reached via the manifest are PROJECT knowledge files
// (a current-turn upload is inlined by the recency rule, so it would not force a
// `read_file`); the model reads them on demand.
//
// NOTE: like all integration tests these need Postgres + a TestServer; run with
// `--test-threads=1`. They are written to the existing harness patterns and are
// compile-verified; first execution may need the usual debugging pass.
// ============================================================================

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Bytes;
use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::common::stub_chat::StubChat;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};
use crate::common::TestServer;

/// A user with a broad permission grant (`*`) so the one identity can create
/// providers/models/groups/projects, upload files, and chat.
async fn power_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["*"]).await
}

/// Upload a text file to the user's library; returns its id.
async fn upload_text(server: &TestServer, user: &TestUser, filename: &str, body: &str) -> String {
    use reqwest::multipart;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(body.as_bytes().to_vec())
            .file_name(filename.to_string())
            .mime_str("text/plain")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::CREATED,
        "upload: {}",
        resp.text().await.unwrap_or_default()
    );
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

/// Create a project, returns its id.
async fn create_project(server: &TestServer, user: &TestUser, name: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": name }))
        .send()
        .await
        .expect("create project");
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED);
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

/// Attach a library file to a project's knowledge files.
async fn attach_file_to_project(server: &TestServer, user: &TestUser, project_id: &str, file_id: &str) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{project_id}/files")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "file_id": file_id }))
        .send()
        .await
        .expect("attach file");
    assert!(
        resp.status().is_success(),
        "attach file: {}",
        resp.text().await.unwrap_or_default()
    );
}

/// Create a conversation pinned to `model_id`; returns `(conversation_id,
/// active_branch_id)`.
async fn create_conversation(server: &TestServer, user: &TestUser, model_id: &str) -> (String, String) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id }))
        .send()
        .await
        .expect("create conv");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::CREATED,
        "create conv: {}",
        resp.text().await.unwrap_or_default()
    );
    let v: Value = resp.json().await.unwrap();
    (
        v["id"].as_str().unwrap().to_string(),
        v["active_branch_id"].as_str().unwrap().to_string(),
    )
}

async fn attach_conversation_to_project(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
    conversation_id: &str,
) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{project_id}/conversations/{conversation_id}"
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("attach conv");
    assert!(resp.status().is_success(), "attach conv to project");
}

/// Send a chat message and return the full SSE response body as text. The
/// streaming endpoint runs the entire tool-use loop server-side before the
/// response completes, so the returned text spans every turn.
async fn send_and_collect(
    server: &TestServer,
    user: &TestUser,
    conversation_id: &str,
    branch_id: &str,
    model_id: &str,
    content: &str,
) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{conversation_id}/messages/stream"
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "content": content,
            "model_id": model_id,
            "branch_id": branch_id,
        }))
        .send()
        .await
        .expect("send message");
    assert!(
        resp.status().is_success(),
        "send message status {}",
        resp.status()
    );
    resp.text().await.expect("collect sse body")
}

/// Enable deployment-wide memory AND opt the user into extraction (no embedding
/// model needed for inline saves). Both must travel together: admin-enable alone
/// is insufficient — inline self-save also requires the per-user
/// `extraction_enabled` opt-in (privacy-first default OFF, migration 56), which
/// the memory extension's `before_llm_call` gate checks before attaching the
/// `remember` tool.
async fn enable_memory(server: &TestServer, user: &TestUser) {
    let client = reqwest::Client::new();
    let resp = client
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .expect("enable memory");
    assert!(
        resp.status().is_success(),
        "enable memory: {}",
        resp.text().await.unwrap_or_default()
    );
    // Per-user opt-in (the inline-self-save gate added by B-correctness-01).
    let resp = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "extraction_enabled": true }))
        .send()
        .await
        .expect("opt user into extraction");
    assert!(
        resp.status().is_success(),
        "enable extraction: {}",
        resp.text().await.unwrap_or_default()
    );
}

// ── Track A ─────────────────────────────────────────────────────────────────

/// The headline Track A behaviour: a project knowledge file is surfaced via the
/// injected manifest (not inlined), and a tool-capable model reads it on demand
/// — proving the manifest → files MCP → file storage → tool_result →
/// continuation round-trip.
#[tokio::test]
async fn manifest_injected_and_read_file_round_trips() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_a").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    // A project file whose content carries a recognizable marker.
    let project_id = create_project(&server, &user, "notes-project").await;
    let file_id = upload_text(
        &server,
        &user,
        "notes.txt",
        "SECRET_MARKER_ZX9 the quarterly numbers are confidential",
    )
    .await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    let body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file what is in my notes?",
    )
    .await;

    // The manifest was injected into the model's context.
    assert!(stub.any_manifest(), "files manifest should be injected for a tool-capable model");
    // The model issued read_file; the continuation re-sends the files tools, so
    // at least one (typically two) request carried `read_file`.
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "read_file tool should be attached + called; requests={:?}",
        stub.requests()
    );
    // The final answer reflects the file's content (true round-trip).
    assert!(
        body.contains("SECRET_MARKER_ZX9"),
        "final answer should echo file content read via read_file; body={body}"
    );
}

/// Track A §3: the built-in files server auto-attaches even when the
/// conversation has MCP disabled (`enable_mcp=false`) — it is privileged and
/// always-on for tool-capable models, NOT gated behind the user MCP toggle.
#[tokio::test]
async fn files_mcp_auto_attaches_even_when_enable_mcp_false() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_a_nomcp").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let project_id = create_project(&server, &user, "nomcp-project").await;
    let file_id = upload_text(&server, &user, "data.txt", "MARKER_NOMCP payload body").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Disable user MCP for this conversation.
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enable_mcp": false }))
        .send()
        .await
        .expect("put mcp-settings");
    assert!(
        resp.status().is_success(),
        "mcp-settings: {}",
        resp.text().await.unwrap_or_default()
    );

    let body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file read it",
    )
    .await;

    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "files server must auto-attach despite enable_mcp=false; requests={:?}",
        stub.requests()
    );
    assert!(body.contains("MARKER_NOMCP"), "answer should reflect file content; body={body}");
}

/// Capability gating: a NON-tool-capable model gets NO manifest and NO files
/// tools — the file content is inlined directly (fallback path) instead.
#[tokio::test]
async fn non_tool_capable_model_gets_no_manifest() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_a_weak").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, false, None,
    )
    .await;

    let project_id = create_project(&server, &user, "weak-project").await;
    let file_id = upload_text(&server, &user, "weak.txt", "weak model inline content").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    let _body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=text summarize",
    )
    .await;

    assert!(!stub.any_manifest(), "no manifest should be injected for a non-tool-capable model");
    assert_eq!(
        stub.requests_with_tool("read_file"),
        0,
        "no files tools should be attached for a non-tool-capable model; requests={:?}",
        stub.requests()
    );
}

// ── Track B ─────────────────────────────────────────────────────────────────

/// Track B inline self-save: a tool-capable model emits an answer AND a
/// `remember` call in one turn; the side-effect loop persists the canned result
/// and finalizes WITHOUT a second generation call. Asserts the row landed with
/// the conversation scope and that exactly one generation carried the tool.
#[tokio::test]
async fn inline_self_save_persists_memory_without_continuation() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_b").await;
    enable_memory(&server, &user).await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;

    let body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=remember The user prefers dark mode.",
    )
    .await;

    // The save rode along in the same turn — exactly one generation call carried
    // the `remember` tool (no no-op continuation).
    assert_eq!(
        stub.requests_with_tool("remember"),
        1,
        "inline self-save must not trigger a continuation call; requests={:?}",
        stub.requests()
    );
    assert!(
        body.contains("remember that"),
        "the assistant answer should be present alongside the save; body={body}"
    );

    // The memory row persisted at conversation scope.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT content, scope FROM user_memories WHERE user_id = $1 AND deleted_at IS NULL",
    )
    .bind(user_uuid)
    .fetch_all(&pool)
    .await
    .expect("query memories");
    pool.close().await;

    assert!(
        rows.iter().any(|(content, scope)| content.contains("dark mode") && scope == "conversation"),
        "a conversation-scoped memory row should be written; rows={rows:?}"
    );
}

// ── C-and-loop-07: third-party server excluded when enable_mcp=false ──────────
//
// Regression guard for C-and-loop-01: when `enable_mcp=false` and a built-in
// flag (files/memory) makes the auto-attach list non-empty, the disabled path
// must request an EXPLICIT EMPTY server list (`Some(vec![])`), NOT `None`.
// `None` routes to `validate_and_build_config`'s "no specific servers requested
// → use ALL accessible servers" branch, which would inject (and, for Always-mode
// servers, PRE-EXECUTE) every third-party MCP server the user can access despite
// MCP being turned off.
//
// This test stands up a real in-process third-party HTTP MCP server exposing a
// uniquely-named `thirdparty_ping` tool with an AtomicUsize hit counter,
// registers it as a user-owned (accessible) server, then sends a
// `read_first_file` chat with `enable_mcp=false`. With the fix: the built-in
// `read_file` still attaches (≥1), but no recorded request carries
// `thirdparty_ping`, and the third-party server records ZERO hits during the
// send. Without the fix, the third-party server is listed (a `tools/list` hit)
// and its tool name appears in the model's tool set.

/// An in-process third-party MCP server (Streamable HTTP transport). Counts
/// every JSON-RPC request it receives so a test can assert it was (or wasn't)
/// reached. `Drop` aborts the background task.
struct ThirdPartyMcpServer {
    url: String,
    hits: Arc<AtomicUsize>,
    handle: JoinHandle<()>,
}

impl Drop for ThirdPartyMcpServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl ThirdPartyMcpServer {
    async fn start() -> ThirdPartyMcpServer {
        let hits = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/", post(third_party_dispatch))
            .with_state(hits.clone());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind third-party mcp server");
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app.into_make_service()).await;
        });
        ThirdPartyMcpServer { url, hits, handle }
    }

    /// Total JSON-RPC requests received since start (or last reset).
    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    /// Zero the counter — used after registration (whose create-time connection
    /// probe legitimately hits the server) so the assertion isolates the
    /// chat-send phase.
    fn reset(&self) {
        self.hits.store(0, Ordering::SeqCst);
    }
}

/// Minimal MCP Streamable-HTTP handler: counts the hit, answers `initialize`,
/// `tools/list` (one `thirdparty_ping` tool), and `tools/call`. Enough for the
/// create-time probe to pass (so the row stays `enabled`) and for any leaked
/// tool-listing during the chat loop to register as a hit.
async fn third_party_dispatch(State(hits): State<Arc<AtomicUsize>>, body: Bytes) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let id = v.get("id").cloned().unwrap_or(json!(1));
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "thirdparty-stub", "version": "0.1.0" }
        }),
        "tools/list" => json!({
            "tools": [{
                "name": "thirdparty_ping",
                "description": "A uniquely-named third-party tool that must NOT be attached when MCP is disabled.",
                "inputSchema": { "type": "object", "properties": {} }
            }]
        }),
        "tools/call" => json!({
            "content": [{ "type": "text", "text": "pong" }],
            "isError": false
        }),
        // notifications/initialized and any other method → empty ack.
        _ => json!({}),
    };

    let payload = json!({ "jsonrpc": "2.0", "id": id, "result": result });
    use axum::http::StatusCode;
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("mcp-session-id", "thirdparty-session-1")
        .body(axum::body::Body::from(payload.to_string()))
        .unwrap()
}

/// Register `url` as a user-owned (and thus accessible) Streamable-HTTP MCP
/// server for `user`. Returns the created server id.
async fn register_user_http_mcp_server(server: &TestServer, user: &TestUser, url: &str) -> String {
    let unique = &Uuid::new_v4().to_string()[..8];
    let resp = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": format!("thirdparty_{unique}"),
            "display_name": "Third-party stub",
            "enabled": true,
            "transport_type": "http",
            "url": url,
            "timeout_seconds": 30,
        }))
        .send()
        .await
        .expect("create user mcp server");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::CREATED,
        "create third-party mcp server: {}",
        resp.text().await.unwrap_or_default()
    );
    // Response is `McpServerWithHealthWarning` → `{ server: {...},
    // connection_warning? }`. Guard that the create-time probe SUCCEEDED so the
    // row stays `enabled = true`. A downgraded (disabled) server would be
    // excluded from the chat loop for the WRONG reason (the `s.enabled` filter,
    // not the enable_mcp=false empty-list fix), making the exclusion assertion
    // pass trivially. Failing loud here keeps the test honest.
    let v: Value = resp.json().await.unwrap();
    assert!(
        v["connection_warning"].is_null(),
        "third-party stub probe failed at create — the test would pass trivially; warning={}",
        v["connection_warning"]
    );
    assert_eq!(
        v["server"]["enabled"].as_bool(),
        Some(true),
        "third-party server must be enabled (probe passed) so the only reason it's \
         excluded from the disabled-MCP send is the empty-list fix; body={v}"
    );
    v["server"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn third_party_mcp_server_excluded_when_enable_mcp_false() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let thirdparty = ThirdPartyMcpServer::start().await;
    let user = power_user(&server, "agentic_excl").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    // A user-owned third-party MCP server the user CAN access. Its create-time
    // connection probe hits the loopback (that's expected) — we reset the
    // counter afterwards so the assertion isolates the chat-send phase.
    let _server_id = register_user_http_mcp_server(&server, &user, &thirdparty.url).await;

    // A project knowledge file flags the built-in `files` server for auto-attach.
    let project_id = create_project(&server, &user, "excl-project").await;
    let file_id = upload_text(&server, &user, "data.txt", "MARKER_EXCL payload body").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Turn user MCP OFF for this conversation.
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enable_mcp": false }))
        .send()
        .await
        .expect("put mcp-settings");
    assert!(
        resp.status().is_success(),
        "mcp-settings: {}",
        resp.text().await.unwrap_or_default()
    );

    // Isolate the chat-send phase from the registration probe.
    thirdparty.reset();

    let _body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file read it",
    )
    .await;

    // 1. The built-in files server STILL auto-attaches (privileged, MCP-toggle
    //    independent) — the read tool was attached + called.
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "built-in files server must auto-attach despite enable_mcp=false; requests={:?}",
        stub.requests()
    );

    // 2. The third-party tool is NOWHERE in the model's tool set. (The chat
    //    extension prefixes third-party tool names with the server id, so we
    //    match by substring rather than an exact name.)
    let leaked: Vec<String> = stub
        .requests()
        .iter()
        .flat_map(|r| r.tool_names.clone())
        .filter(|name| name.contains("thirdparty_ping"))
        .collect();
    assert!(
        leaked.is_empty(),
        "no third-party tool may be attached when enable_mcp=false; leaked={leaked:?}, requests={:?}",
        stub.requests()
    );

    // 3. The third-party server was never reached during the send (no
    //    tools/list and no tools/call leaked to it).
    assert_eq!(
        thirdparty.hits(),
        0,
        "the third-party MCP server must record ZERO hits during a disabled-MCP send"
    );
}
