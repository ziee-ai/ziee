use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;
use crate::common::stub_chat::StubChat;
use crate::common::test_helpers::TestUser;
use crate::common::test_helpers::create_user_with_permissions;
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

/// Fire-and-forget send + collect the streamed reply (realtime-sync model: POST
/// `/conversations/{id}/messages` returns ids; the reply streams over
/// `GET /api/chat/stream`). The server runs the WHOLE tool-use loop before the
/// reply terminates, so by the time this returns the stub has recorded every
/// generation request. Returns the assembled assistant text. `file_ids` attaches
/// a current-turn upload; `enable_mcp` overrides the send flag (default unset →
/// the SendMessageRequest default `false`, which still auto-attaches the
/// privileged files/memory built-ins).
async fn send_collect(
    server: &TestServer,
    user: &TestUser,
    conversation_id: &str,
    branch_id: &str,
    model_id: &str,
    content: &str,
    file_ids: &[String],
    enable_mcp: Option<bool>,
) -> String {
    use crate::common::chat_stream_probe::ChatStreamProbe;
    let conv = Uuid::parse_str(conversation_id).expect("conversation uuid");
    let mut probe = ChatStreamProbe::open(server, &user.token).await;
    probe.subscribe(Some(conv)).await;

    let mut payload = json!({
        "content": content,
        "model_id": model_id,
        "branch_id": branch_id,
    });
    if !file_ids.is_empty() {
        payload["file_ids"] = json!(file_ids);
    }
    if let Some(em) = enable_mcp {
        payload["enable_mcp"] = json!(em);
    }
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conversation_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("send message");
    assert!(
        resp.status().is_success(),
        "send status {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    let frames = probe
        .collect_until_terminal(conv, std::time::Duration::from_secs(30))
        .await;
    ChatStreamProbe::assemble_text(&frames)
}

/// Plain send + collect the assembled reply text.
async fn send_and_collect(
    server: &TestServer,
    user: &TestUser,
    conversation_id: &str,
    branch_id: &str,
    model_id: &str,
    content: &str,
) -> String {
    send_collect(server, user, conversation_id, branch_id, model_id, content, &[], None).await
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

/// Send a chat message carrying `file_ids` (a current-turn upload) and wait for
/// the whole turn to complete server-side (so the stub recorded every request).
async fn send_with_files(
    server: &TestServer,
    user: &TestUser,
    conversation_id: &str,
    branch_id: &str,
    model_id: &str,
    content: &str,
    file_ids: &[String],
) {
    let _ = send_collect(
        server,
        user,
        conversation_id,
        branch_id,
        model_id,
        content,
        file_ids,
        None,
    )
    .await;
}

// ── Track A ─────────────────────────────────────────────────────────────────

/// Track A recency-drop: a file attached on turn 1 is inlined THAT turn, but on
/// turn 2 it is an OLD attachment — dropped from the replayed history (the model
/// gets the manifest + read_file instead of the bytes re-inlined every turn).
/// This pins the dispatch + shared-resolution fix (the recency-drop was a no-op
/// while the file ext declared the wrong handled_content_types).
#[tokio::test]
async fn old_attachment_dropped_from_replay_for_tool_capable() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_recency").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    // Name carries no marker; only the CONTENT does, so the manifest (which lists
    // names, not bytes) never trips the marker assertion.
    let file_id = upload_text(
        &server,
        &user,
        "notes.txt",
        "INLINE_MARKER_QZ7 the secret figure is 42",
    )
    .await;

    // Turn 1: attach the file — the current upload is inlined this turn.
    send_with_files(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=text what is in my notes?",
        &[file_id.clone()],
    )
    .await;

    // Turn 2: no new attachment — the turn-1 file is now an OLD attachment.
    send_with_files(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=text and now?",
        &[],
    )
    .await;

    let reqs = stub.requests();
    // Turn-1 main generation (manifest + the turn-1 user text) inlined the bytes.
    let turn1 = reqs
        .iter()
        .find(|r| r.all_text.contains("what is in my notes?") && r.has_manifest)
        .expect("a turn-1 main generation request with the manifest");
    assert!(
        turn1.all_text.contains("INLINE_MARKER_QZ7"),
        "turn-1 current upload should be inlined in full"
    );
    // Turn-2 main generation (carries the turn-2 user text) must NOT re-inline the
    // old attachment's bytes, but the manifest must still list the file.
    let turn2 = reqs
        .iter()
        .find(|r| r.all_text.contains("and now?") && r.has_manifest)
        .expect("a turn-2 main generation request with the manifest");
    assert!(
        !turn2.all_text.contains("INLINE_MARKER_QZ7"),
        "turn-2 must NOT re-inline the old attachment bytes (recency-drop); got: {}",
        turn2.all_text
    );
}

/// Regression (round 8): on a tool-loop CONTINUATION (iteration >= 2) the
/// current upload must NOT be re-inlined as a stray trailing `user` turn after
/// the assistant tool round-trip (that both corrupts the tool_use→tool_result
/// structure and re-sends the bytes the manifest exists to omit). We trigger a
/// read_file round-trip (=> iteration 2) with an attachment present and assert
/// no continuation request ends with a `user` role.
#[tokio::test]
async fn current_upload_not_reinlined_on_tool_loop_continuation() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_iter2").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let file_id = upload_text(&server, &user, "notes.txt", "ITER2_MARKER the data is here").await;

    // One turn that triggers a read_file round-trip (=> a 2nd loop iteration)
    // with the attachment present as the current upload.
    send_with_files(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file summarize my notes",
        &[file_id.clone()],
    )
    .await;

    let reqs = stub.requests();
    // A tool-loop continuation actually happened (so the assertion is meaningful).
    assert!(
        reqs.iter().any(|r| r.had_tool_result),
        "expected a tool-loop continuation request; requests={reqs:?}"
    );
    // No continuation (had_tool_result) request may END with a `user` role — that
    // would be the stray re-inlined-upload turn pushed after the tool result.
    for r in &reqs {
        if r.had_tool_result {
            assert_ne!(
                r.roles.last().map(String::as_str),
                Some("user"),
                "continuation request must not end with a stray re-inlined user turn; roles={:?}",
                r.roles
            );
        }
    }
}

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

    // Send with general MCP explicitly disabled (`enable_mcp: false`). The
    // privileged files built-in must STILL auto-attach (C-and-loop-01).
    let body = send_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file read it",
        &[],
        Some(false),
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

/// End-to-end coverage of the web_search attach seam (the documented
/// silent-failure point): before_llm_call → attach_gate_open → apply →
/// auto_attach_builtin_ids → loopback tools/list. With web search enabled + a
/// configured provider on a tool-capable model, the web_search + fetch_url tools
/// must be offered to the LLM.
#[tokio::test]
async fn web_search_tools_attach_when_enabled_and_configured() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_websearch").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    // A valid base_url makes searxng "configured" (no network needed — the tool
    // only has to be ATTACHED, not called).
    let client = reqwest::Client::new();
    let r = client
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "config": { "base_url": "https://searxng.example.com" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true, "provider_chain": ["searxng"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let _ = send_and_collect(&server, &user, &conv_id, &branch_id, &model_id, "STUB_PLAN=text hello").await;

    assert!(
        stub.requests_with_tool("web_search") >= 1,
        "web_search tool must attach for a tool-capable model with web search configured; requests={:?}",
        stub.requests()
    );
    assert!(
        stub.requests_with_tool("fetch_url") >= 1,
        "fetch_url tool must attach too; requests={:?}",
        stub.requests()
    );
}

/// Negative: with NO provider configured, the attach gate is closed and the
/// web_search tools must NOT be offered — even though `enabled` defaults true.
#[tokio::test]
async fn web_search_tools_not_attached_when_unconfigured() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_websearch_off").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let _ = send_and_collect(&server, &user, &conv_id, &branch_id, &model_id, "STUB_PLAN=text hello").await;

    assert_eq!(
        stub.requests_with_tool("web_search"),
        0,
        "web_search must NOT attach with no provider configured; requests={:?}",
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
    // `/mcp/servers` returns the created `McpServer` flat. Guard that it stays
    // `enabled = true` (the stub answers the create-time health probe, so it
    // should be healthy): a disabled server would be excluded from the chat loop
    // for the WRONG reason (the `s.enabled` filter, not the enable_mcp=false
    // empty-list fix), making the exclusion assertion pass trivially.
    let v: Value = resp.json().await.unwrap();
    assert_eq!(
        v["enabled"].as_bool(),
        Some(true),
        "third-party server must be enabled so the only reason it's excluded from \
         the disabled-MCP send is the empty-list fix; body={v}"
    );
    v["id"].as_str().unwrap().to_string()
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

    // Isolate the chat-send phase from the registration probe.
    thirdparty.reset();

    // Send with general MCP OFF (`enable_mcp: false`).
    let _body = send_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file read it",
        &[],
        Some(false),
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

/// Cross-subsystem sync cascade (gap 49e65): a SINGLE chat turn that invokes a
/// built-in MCP tool (read_file) must, in addition to streaming the reply,
/// record the invocation and emit an owner-scoped `mcp_tool_call`/create frame
/// on the realtime-sync stream. Existing chat_stream tests assert the reply but
/// not the multi-entity sync cascade a tool-using turn produces.
#[tokio::test]
async fn chat_tool_turn_emits_mcp_tool_call_sync_to_owner() {
    use crate::common::sync_probe::SyncProbe;
    use std::time::Duration;

    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "cascade_owner").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let project_id = create_project(&server, &user, "cascade-project").await;
    let file_id = upload_text(&server, &user, "notes.txt", "CASCADE_MARKER alpha").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // A second user must never see the owner-scoped tool-call frame.
    let other = create_user_with_permissions(&server, "cascade_other", &[]).await;

    // Open the realtime-sync probes BEFORE the turn so we capture the cascade.
    let mut owner_probe = SyncProbe::open(&server, &user.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file what is in my notes?",
    )
    .await;
    // Real round-trip: the read_file tool actually ran.
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "read_file must be called; requests={:?}",
        stub.requests()
    );
    assert!(body.contains("CASCADE_MARKER"), "answer should echo file content: {body}");

    // The tool invocation produced an owner-scoped mcp_tool_call/create frame.
    let frame = owner_probe
        .expect_event("mcp_tool_call", "create", Duration::from_secs(10))
        .await;
    assert!(
        uuid::Uuid::parse_str(&frame.id).is_ok() && frame.id != uuid::Uuid::nil().to_string(),
        "frame must carry the tool-call row id, got {:?}",
        frame.id
    );

    // Owner-scoped: the unrelated user observes nothing.
    other_probe.expect_silence(Duration::from_secs(1)).await;
}

/// files_mcp + memory cross-subsystem co-attachment (gap cef7ec3387ce). Prior
/// tests exercise memory OR files in isolation; this drives ONE turn with BOTH
/// memory enabled AND a project knowledge file, and asserts the model's request
/// carries BOTH built-in surfaces (the files manifest + read_file, and the
/// memory `remember` tool) and that the file is actually read end-to-end.
#[tokio::test]
async fn files_and_memory_built_ins_coexist_in_one_turn() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_files_mem").await;
    enable_memory(&server, &user).await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let project_id = create_project(&server, &user, "files-mem-project").await;
    let file_id = upload_text(&server, &user, "notes.txt", "XSUBSYS_MARK shared facts").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    let body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file what's in my notes?",
    )
    .await;

    // Files subsystem: manifest injected + read_file actually invoked → answer
    // reflects the file content.
    assert!(stub.any_manifest(), "files manifest must be injected");
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "read_file (files_mcp) must be attached + called; requests={:?}",
        stub.requests()
    );
    assert!(body.contains("XSUBSYS_MARK"), "answer reflects the read file: {body}");

    // Memory subsystem co-attached in the SAME turn: the `remember` tool is
    // offered alongside the files tools (proves both built-ins coexist, not
    // mutually exclusive).
    assert!(
        stub.requests_with_tool("remember") >= 1,
        "memory `remember` tool must be co-attached with files tools; requests={:?}",
        stub.requests()
    );
}

/// audit id all-78dfbbb87877 — the model recalling a PRIOR tool result via the
/// built-in `get_tool_result` tool. Turn 1 reads a file (creating a persisted
/// read_file tool_result); we grab that block's tool_use_id, then Turn 2 drives
/// the stub to call get_tool_result(tool_use_id) and asserts the recalled
/// content (the original file body) flows back — the full model→get_tool_result
/// →tool_result_mcp→stored-history recall roundtrip. Deterministic (stub model).
#[tokio::test]
async fn model_recalls_prior_result_via_get_tool_result() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_recall").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let project_id = create_project(&server, &user, "recall-project").await;
    let file_id = upload_text(
        &server,
        &user,
        "notes.txt",
        "RECALL_MARKER_Q7 the full original tool result body",
    )
    .await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Turn 1: read the file → persists a read_file tool_result.
    let _ = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=read_first_file read it",
    )
    .await;

    // Find the persisted tool_result block's tool_use_id.
    let msgs: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut tool_use_id: Option<String> = None;
    // `GET /conversations/{id}/messages` returns a `PaginatedMessages` object
    // ({ messages, has_more_before, has_more_after }), not a bare array.
    for m in msgs["messages"].as_array().unwrap() {
        for c in m["contents"].as_array().into_iter().flatten() {
            if c["content_type"] == "tool_result" {
                if let Some(id) = c["content"]["tool_use_id"].as_str() {
                    tool_use_id = Some(id.to_string());
                }
            }
        }
    }
    let tool_use_id = tool_use_id.expect("a persisted read_file tool_result with a tool_use_id");

    // Turn 2: the model calls get_tool_result(tool_use_id) to recall the result.
    let body = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        &format!("STUB_PLAN=get_tool_result STUB_TOOLUSE={tool_use_id} recall it"),
    )
    .await;

    assert!(
        stub.requests_with_tool("get_tool_result") >= 1,
        "get_tool_result must be attached + called; requests={:?}",
        stub.requests()
    );
    assert!(
        body.contains("RECALL_MARKER_Q7"),
        "recalled result must carry the original tool output; body={body}"
    );
}

/// audit id all-d51c1faaf853 — per-assistant CORE MEMORY injection into chat
/// requests. With memory enabled + an assistant carrying a core-memory block,
/// before_llm_call (retriever::inject_core_memory_blocks) must prepend the block
/// as a system message the model receives. Asserted via the stub's recorded
/// request text (deterministic; no real LLM).
#[tokio::test]
async fn core_memory_block_is_injected_into_the_chat_request() {
    use crate::common::chat_stream_probe::ChatStreamProbe;
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_coremem").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;
    enable_memory(&server, &user).await;

    // An assistant + a core-memory block for this user+assistant.
    let assistant: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Persona Bot", "is_template": false, "enabled": true }))
        .send().await.unwrap().json().await.unwrap();
    let assistant_id = assistant["id"].as_str().unwrap().to_string();

    let put = reqwest::Client::new()
        .put(server.api_url("/assistants/core-memory"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "assistant_id": assistant_id,
            "block_label": "persona",
            "content": "CORE_MEM_MARKER_K3 always answer like a pirate",
            "char_limit": 1000,
        }))
        .send().await.unwrap();
    assert_eq!(put.status(), 200, "core-memory upsert: {}", put.text().await.unwrap_or_default());

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let conv = Uuid::parse_str(&conv_id).unwrap();
    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv)).await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "content": "STUB_PLAN=text hi",
            "model_id": model_id,
            "branch_id": branch_id,
            "assistant_id": assistant_id,
        }))
        .send().await.unwrap();
    assert!(resp.status().is_success(), "send: {}", resp.text().await.unwrap_or_default());
    let _ = probe.collect_until_terminal(conv, std::time::Duration::from_secs(30)).await;

    // The core-memory block reached the model in a system message.
    assert!(
        stub.requests().iter().any(|r| r.all_text.contains("CORE_MEM_MARKER_K3")),
        "core-memory block content must be injected into the chat request; requests={:?}",
        stub.requests()
    );
    assert!(
        stub.requests().iter().any(|r| r.all_text.contains("Assistant core memory")),
        "the core-memory system header must be present"
    );
}

/// audit id all-84c76a5591f4 — multiple subsystems exercised TOGETHER in one
/// conversation: memory (`remember`), files (`read_file`), and tool-result
/// recall (`get_tool_result`) all coexist + interoperate across turns. No prior
/// test combined 3 built-in MCP subsystems in a single chat. Deterministic stub.
#[tokio::test]
async fn multiple_builtin_subsystems_coexist_in_one_conversation() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_multi").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;
    enable_memory(&server, &user).await;

    let project_id = create_project(&server, &user, "multi-project").await;
    let file_id = upload_text(&server, &user, "notes.txt", "MULTI_MARKER_88 combined subsystems").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Turn 1 — memory: remember a fact.
    let _ = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=remember The user is testing combined subsystems.",
    ).await;
    // Turn 2 — files: read the file (creates a tool_result).
    let body2 = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=read_first_file read it",
    ).await;
    assert!(body2.contains("MULTI_MARKER_88"), "files subsystem round-trip: {body2}");

    // Both subsystems' tools were attached + used in the SAME conversation.
    assert!(stub.requests_with_tool("remember") >= 1, "memory tool used: {:?}", stub.requests());
    assert!(stub.requests_with_tool("read_file") >= 1, "files tool used: {:?}", stub.requests());

    // The remembered fact persisted (memory subsystem actually wrote).
    let mems: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap().json().await.unwrap();
    let items = mems["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().any(|m| m["content"].as_str().unwrap_or("").contains("combined subsystems")),
        "the remembered fact must persist alongside the files round-trip; mems={mems}"
    );
}

/// audit ids all-cd217222b2fa + all-cee2507f7ec7 — automatic summarization
/// during a real chat turn (the after_llm_call extension hook's background
/// refresh) AND the before_llm_call hook applying the resulting summary on the
/// next turn. Deterministic: the summarization model is the stub, so refresh
/// persists a summary without a real LLM key.
#[tokio::test]
async fn summarization_hooks_run_during_chat_and_apply_on_next_turn() {
    use crate::common::chat_stream_probe::ChatStreamProbe;
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_summ").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    // Enable summarization, point the summarizer at the stub model, and set a
    // low trigger so a couple of long turns cross it.
    let put = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "enabled": true,
            "default_summarization_model_id": model_id,
            "summarize_after_tokens": 500,
            "summarizer_keep_recent_tokens": 100,
        }))
        .send().await.unwrap();
    assert!(put.status().is_success(), "summarization settings: {}", put.text().await.unwrap_or_default());

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let conv = Uuid::parse_str(&conv_id).unwrap();

    // Two long turns (~1600 chars each) → well past the 500-token (≈2000-char)
    // trigger, so the after_llm_call hook fires its background refresh.
    let long = "STUB_PLAN=text ".to_string() + &"context filler sentence. ".repeat(70);
    for _ in 0..2 {
        let _ = send_and_collect(&server, &user, &conv_id, &branch_id, &model_id, &long).await;
    }

    // The after_llm_call hook persists a rolling summary in the background.
    let summary_url = server.api_url(&format!("/conversations/{conv_id}/summary"));
    let token = user.token.clone();
    let mut got_summary = false;
    for _ in 0..30 {
        let body: serde_json::Value = reqwest::Client::new()
            .get(&summary_url)
            .header("Authorization", format!("Bearer {token}"))
            .send().await.unwrap().json().await.unwrap();
        if !body.is_null() && body.get("summary_text").and_then(|v| v.as_str()).is_some() {
            got_summary = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(got_summary, "after_llm_call must persist a rolling summary during chat");

    // before_llm_call: the NEXT turn applies the stored summary into the request
    // the model sees (the condensed-history prefix). Assert the summary system
    // block reaches the stub.
    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv)).await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": "STUB_PLAN=text and now?", "model_id": model_id, "branch_id": branch_id }))
        .send().await.unwrap();
    assert!(resp.status().is_success());
    let _ = probe.collect_until_terminal(conv, std::time::Duration::from_secs(30)).await;

    // The applied summary surfaces as a "conversation summary" system block in
    // the request (before_llm_call → apply_summary_to_history).
    assert!(
        stub.requests().iter().any(|r| {
            let t = r.all_text.to_lowercase();
            t.contains("summary")
        }),
        "before_llm_call must inject the summary into a later turn's request; requests={:?}",
        stub.requests()
    );
}

/// Multi-turn model-authored-file workflow: in turn 1 the model AUTHORS a new
/// file via the files_mcp `create_file` tool, and in a LATER turn of the SAME
/// conversation it reads that file back by name — proving a model-created file
/// persists to the store AND stays reusable across turns (it is part of the
/// conversation's available files on the next turn). Distinct from
/// `manifest_injected_and_read_file_round_trips`, which reads a pre-uploaded
/// project file, never one the model itself created.
#[tokio::test]
async fn model_authored_file_persists_and_is_reread_across_turns() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_authored").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;

    const BEACON: &str = "AUTHORED_BEACON_K7Q";
    let filename = "authored_notes.md";

    // --- Turn 1: the model authors a brand-new file via create_file. ---
    let t1 = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        &format!(
            "STUB_PLAN=create_file STUB_FILE={filename} STUB_CONTENT={BEACON} please write my notes"
        ),
    )
    .await;
    assert!(
        stub.requests_with_tool("create_file") >= 1,
        "turn 1 must call the create_file tool; requests={:?}",
        stub.requests()
    );
    assert!(
        t1.contains("Created the file"),
        "turn 1 answer should confirm creation; body={t1}"
    );

    // The authored file is a durable, model-created (`created_by="mcp"`) store
    // artifact owned by the user — independently verifiable, not just an
    // in-flight handle.
    let listed: Value = reqwest::Client::new()
        .get(server.api_url("/files?page=1&per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list files")
        .json()
        .await
        .expect("files json");
    let authored = listed["files"]
        .as_array()
        .expect("files array")
        .iter()
        .find(|f| f["filename"].as_str() == Some(filename))
        .unwrap_or_else(|| {
            panic!("authored file must be persisted in the user's library; got {listed}")
        });
    assert_eq!(
        authored["created_by"].as_str(),
        Some("mcp"),
        "the file must be marked model-authored (created_by=mcp); file={authored}"
    );

    // --- Turn 2 (same conversation): the model reads the authored file back BY
    // NAME — succeeds only because the turn-1 file persisted and is part of this
    // conversation's available files. ---
    let t2 = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        &format!("STUB_PLAN=read_named STUB_NAME={filename} what did you write?"),
    )
    .await;
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "turn 2 must call read_file on the authored file; requests={:?}",
        stub.requests()
    );
    assert!(
        t2.contains(BEACON),
        "turn 2 must read back the content the model authored in turn 1; body={t2}"
    );
}

// ── Cross-subsystem: files_mcp + memory in one conversation ──────────────────
//
// Audit all-ac7341fd2d7a: the agentic memory tests (Track B) exercise memory
// alone, and the files tests (Track A) exercise files_mcp alone — nothing drives
// BOTH built-in subsystems in a single conversation. This stub-driven test does:
// with a project file attached AND per-user memory enabled, turn 1 has the model
// read the file via the files_mcp `read_file` tool (asserting the marker rides
// back into the answer), and turn 2 of the SAME conversation has it emit a
// `remember` call (asserting a conversation-scoped memory row persists). Both the
// files manifest→read_file round-trip and the memory side-effect loop run for
// real against the same conversation; only the model is the deterministic stub.
#[tokio::test]
async fn files_mcp_and_memory_combine_in_one_conversation() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_files_memory").await;
    enable_memory(&server, &user).await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    // A project knowledge file, surfaced via the manifest (not inlined).
    let project_id = create_project(&server, &user, "files-memory-project").await;
    let file_id = upload_text(
        &server,
        &user,
        "notes.txt",
        "XSUBSYS_MARKER_42 the launch is scheduled for Q3",
    )
    .await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Turn 1 — files_mcp: the model reads the attached file on demand.
    let t1 = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file what is in my notes?",
    )
    .await;
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "files_mcp read_file must fire in the combined conversation; requests={:?}",
        stub.requests()
    );
    assert!(
        t1.contains("XSUBSYS_MARKER_42"),
        "turn 1 answer must echo file content read via files_mcp; body={t1}"
    );

    // Turn 2 — memory: the model self-saves a fact in the SAME conversation.
    let t2 = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=remember The launch is scheduled for Q3.",
    )
    .await;
    // `requests_with_tool` counts requests that ADVERTISE the tool, not
    // invocations — and the memory extension attaches `remember` on EVERY
    // tool-capable turn (the inline self-save nudge), so it is advertised on
    // turn 1's read_file turns too. The authoritative proof that remember
    // actually FIRED (exactly once, in turn 2) is the persisted
    // conversation-scoped `user_memories` row asserted below.
    assert!(
        stub.requests_with_tool("remember") >= 1,
        "the memory remember tool must be attached in turn 2; requests={:?}",
        stub.requests()
    );
    assert!(
        t2.contains("remember that"),
        "turn 2 answer should accompany the memory save; body={t2}"
    );

    // Memory subsystem actually persisted a conversation-scoped row.
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
        rows.iter().any(|(content, scope)| content.contains("Q3") && scope == "conversation"),
        "a conversation-scoped memory row must persist alongside the files_mcp read; rows={rows:?}"
    );
}

/// 5453123 — a single conversation that chains every agentic surface end-to-end:
/// upload a file → analyze it (the model calls the files_mcp `read_file` tool) →
/// edit persistent state via MCP (the model calls the memory `remember` tool) →
/// a plain follow-up turn. Proves the multi-step flow holds together in ONE
/// conversation (deterministic via the stub model + STUB_PLAN scripting; the
/// individual surfaces are tested in isolation elsewhere, this pins the chain).
#[tokio::test]
async fn multi_step_upload_analyze_mcp_edit_then_followup() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_multistep").await;
    enable_memory(&server, &user).await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let file_id = upload_text(
        &server,
        &user,
        "report.txt",
        "ANALYZE_MARKER_5453 the quarterly total is 100 units",
    )
    .await;

    // Turn 1 — upload + analyze: the model calls files_mcp `read_file` and echoes
    // the file's content back (the read_first_file plan).
    let turn1 = send_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=read_first_file analyze the attached report",
        &[file_id.clone()],
        Some(true),
    )
    .await;
    assert!(
        turn1.contains("ANALYZE_MARKER_5453"),
        "turn 1 should round-trip the file content via read_file; got: {turn1}"
    );
    assert!(stub.requests_with_tool("read_file") >= 1, "read_file must have been called");

    // Turn 2 — MCP edit: the model calls the memory `remember` tool to persist a
    // durable fact (a real built-in MCP write in the same conversation).
    let turn2 = send_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=remember The quarterly total is 100 units.",
        &[],
        Some(true),
    )
    .await;
    assert!(turn2.contains("remember that"), "turn 2 should acknowledge the save; got: {turn2}");
    assert!(stub.requests_with_tool("remember") >= 1, "remember must have been called");

    // Turn 3 — plain follow-up completes in the same conversation.
    let turn3 = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "thanks, that's all for now",
    )
    .await;
    assert!(!turn3.is_empty(), "follow-up turn should produce a reply");

    // The MCP edit persisted: a conversation-scoped memory row exists.
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
        rows.iter().any(|(content, _)| content.contains("100 units")),
        "the MCP remember edit should have persisted a memory row; rows={rows:?}"
    );
}

/// Cross-subsystem combined flow: a FILE ATTACHMENT (file chat-extension inlines
/// its bytes) and MEMORY (the memory chat-extension attaches + the model calls
/// `remember`) are both active in the SAME chat turn. Asserts the file content
/// reached the model AND a memory row was persisted — the file+memory+chat
/// intersection none of the single-subsystem tests cover together.
#[tokio::test]
async fn file_attachment_and_memory_combine_in_one_turn() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_file_mem").await;
    enable_memory(&server, &user).await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    let file_id = upload_text(
        &server,
        &user,
        "report.txt",
        "COMBINED_FILE_MARKER the quarterly total is 100 units",
    )
    .await;

    // One turn: attach the file (inlined this turn) AND drive the remember tool.
    let reply = send_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=remember The quarterly total is 100 units.",
        &[file_id.clone()],
        Some(true),
    )
    .await;
    assert!(reply.contains("remember that"), "the turn should acknowledge the save; got: {reply}");

    // (a) The attached file's content reached the model (file chat-extension).
    let saw_file = stub
        .requests()
        .iter()
        .any(|r| r.all_text.contains("COMBINED_FILE_MARKER"));
    assert!(saw_file, "the attached file content must be inlined into the model request");

    // (b) The memory subsystem fired the remember tool in the same turn.
    assert!(
        stub.requests_with_tool("remember") >= 1,
        "the memory `remember` tool must have been attached + called"
    );

    // (c) And it persisted a memory row.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT content FROM user_memories WHERE user_id = $1 AND deleted_at IS NULL",
    )
    .bind(user_uuid)
    .fetch_all(&pool)
    .await
    .expect("query memories");
    pool.close().await;
    assert!(
        rows.iter().any(|(c,)| c.contains("100 units")),
        "the remember edit should persist a memory row; rows={rows:?}"
    );
}

/// Cross-subsystem: files_mcp AND memory built-ins active in the SAME
/// conversation. The memory tests never attach files; the files_mcp tests never
/// enable memory. Here both are wired: turn 1 reads a project file (files_mcp),
/// turn 2 remembers a fact (memory) — asserting both subsystems function
/// together without interfering.
#[tokio::test]
async fn files_mcp_and_memory_coexist_in_one_conversation() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "agentic_files_mem").await;
    enable_memory(&server, &user).await;

    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    // A project file carrying a marker, attached to the conversation.
    let project_id = create_project(&server, &user, "files-mem-project").await;
    let file_id = upload_text(
        &server,
        &user,
        "memo.txt",
        "CROSS_MARKER_42 the quarterly figures are confidential",
    )
    .await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;

    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Turn 1 — files_mcp: read the attached file and echo its content.
    let body1 = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=read_first_file what is in my memo?",
    )
    .await;
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "turn 1 should call read_file; requests={:?}",
        stub.requests()
    );
    assert!(
        body1.contains("CROSS_MARKER_42"),
        "turn 1 answer should echo the file content; body={body1}"
    );

    // Turn 2 — memory: remember a fact in the SAME conversation.
    let body2 = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id,
        "STUB_PLAN=remember The user audits figures quarterly.",
    )
    .await;
    assert!(
        body2.contains("remember that"),
        "turn 2 should acknowledge the save; body={body2}"
    );

    // Both subsystems produced their effect: the memory row persisted.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT content FROM user_memories WHERE user_id = $1 AND deleted_at IS NULL",
    )
    .bind(Uuid::parse_str(&user.user_id).unwrap())
    .fetch_all(&pool)
    .await
    .expect("query memories");
    pool.close().await;
    assert!(
        rows.iter().any(|(c,)| c.to_lowercase().contains("quarterly") || c.to_lowercase().contains("audit")),
        "memory should persist alongside the files_mcp usage; rows={rows:?}"
    );
}

/// Summarization chat-extension before_llm_call hook (summarization.rs:89-131).
/// With a persisted rolling summary for the branch, a subsequent turn must have
/// apply_summary_to_history inject the summary block (replacing the summarized
/// prefix) into the OUTBOUND request the model sees — end-to-end through the
/// chat pipeline, not just the unit-tested pure apply_summary_block. Driven by
/// the StubChat which records every request it receives.
#[tokio::test]
async fn before_llm_call_injects_persisted_rolling_summary() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "summ_before").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;

    // Turn 1 builds history (a user + an assistant message in the branch).
    let _ = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id, "first turn about a tokyo trip",
    )
    .await;

    // Seed a rolling summary covering those 2 messages (apply_summary_block uses
    // message_count + summary_text only; summarized_up_to_id may be NULL).
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    sqlx::query!(
        r#"INSERT INTO conversation_summaries
            (branch_id, summary_text, summarized_up_to_id, message_count, model_used)
            VALUES ($1, $2, NULL, 2, 'stub')"#,
        Uuid::parse_str(&branch_id).unwrap(),
        "SUMMARY_SENTINEL_XYZ — the user is planning a tokyo trip"
    )
    .execute(&pool)
    .await
    .expect("seed conversation_summaries");
    pool.close().await;

    // Turn 2 — before_llm_call must inject the summary block.
    let _ = send_and_collect(
        &server, &user, &conv_id, &branch_id, &model_id, "second turn",
    )
    .await;

    // The most recent request the model saw carried the injected summary.
    let reqs = stub.requests();
    let last = reqs.last().expect("at least one recorded request");
    assert!(
        last.all_text.contains("SUMMARY_SENTINEL_XYZ")
            && last.all_text.contains("Earlier conversation summary"),
        "before_llm_call must inject the persisted rolling summary into the outbound request; all_text={}",
        last.all_text
    );
}

/// Cross-subsystem COEXISTENCE: in ONE tool-capable conversation, the auto-
/// attached built-ins from MULTIPLE independent subsystems are all present
/// together — memory (remember), lit_search (literature_search), citations
/// (list_citations), tool_result recall (get_tool_result), and elicitation
/// (ask_user). The audit flagged that no test references these subsystems
/// together; this asserts the attach set spans all of them, the integration
/// point the lit_search→recall→citations and web_search+memory+… flows rely on.
#[tokio::test]
async fn multiple_subsystem_builtins_coexist_in_one_conversation() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "coexist_user").await;
    enable_memory(&server, &user).await; // attaches the memory `remember` tool
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;

    // A plain text turn — we only need one generation so the auto-attached
    // built-in tool set is recorded by the stub.
    let _ = send_and_collect(&server, &user, &conv_id, &branch_id, &model_id, "hello there").await;

    let reqs = stub.requests();
    let first = reqs.first().expect("at least one recorded request");
    let attached = &first.tool_names;

    for (subsystem, tool) in [
        ("memory", "remember"),
        ("lit_search", "literature_search"),
        ("citations", "list_citations"),
        ("tool_result", "get_tool_result"),
        ("elicitation", "ask_user"),
    ] {
        assert!(
            first.has_tool(tool),
            "{subsystem} built-in '{tool}' must be auto-attached alongside the others; attached={attached:?}"
        );
    }
}

/// Tool-call history records BUILT-IN MCP tool calls too: when the chat loop
/// drives a `files_mcp` tool (read_file) via the StubChat, an `mcp_tool_calls`
/// row is written with `is_built_in = true` and `server_name = 'files_mcp'`.
/// The existing tool_call_history tests only cover a user MockMcpServer
/// (is_built_in = false). Recording is fire-and-forget, so poll briefly.
#[tokio::test]
async fn files_mcp_tool_call_is_recorded_as_built_in() {
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "files_record").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;

    let project_id = create_project(&server, &user, "rec-project").await;
    let file_id = upload_text(&server, &user, "rec.txt", "RECORDED_MARKER_77 content here").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    let _ = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file what is in my notes?",
    )
    .await;

    // The built-in files_mcp call must land an mcp_tool_calls row (fire-and-forget).
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let uid = Uuid::parse_str(&user.user_id).unwrap();
    let mut found: Option<(bool, String, String)> = None;
    for _ in 0..40 {
        // The built-in files server is registered in `mcp_servers` with
        // name='files' (display_name 'Files') — see files_mcp::repository::
        // upsert_builtin_server. The journal row records that server row name,
        // NOT the module name "files_mcp".
        let row = sqlx::query_as::<_, (bool, String, String)>(
            "SELECT is_built_in, server_name, tool_name FROM mcp_tool_calls \
             WHERE user_id = $1 AND server_name = 'files' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(uid)
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some(r) = row {
            found = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    pool.close().await;

    let (is_built_in, server_name, tool_name) =
        found.expect("a files_mcp tool-call row must be recorded");
    assert!(is_built_in, "files_mcp is a built-in server → is_built_in=true");
    assert_eq!(server_name, "files");
    assert!(
        tool_name.ends_with("read_file") || tool_name == "read_file",
        "recorded tool should be read_file, got {tool_name}"
    );
}


/// B2: the chat AGENT-CORE path enforces the conversation's `disabled_servers` at
/// CALL time (`enforce_conversation_disabled = true`). Disabling the files_mcp
/// `read_file` TOOL in the conversation does NOT remove it from the attached set
/// (there is no attach-time filter for `disabled_servers` — it is enforced only in
/// `call_mcp_tool`), so the model still calls it — and the call must be REFUSED,
/// not executed. Proves the ON path honors the user's disable at call time (the
/// DEC-17 non-enforcement was a security gap). Runs flag ON.
#[tokio::test]
async fn chat_agent_core_enforces_conversation_disabled_server_at_call_time() {
    let _flag = crate::common::AgentCoreFlag::on();
    let server = TestServer::start().await;
    let stub = StubChat::start().await;
    let user = power_user(&server, "b2_disabled").await;
    let model_id = crate::common::stub_chat::register_stub_model(
        &server,
        &user.token,
        &user.user_id,
        &stub.base_url,
        true,
        None,
    )
    .await;

    // A file so files_mcp READ tools attach (manifest_available = files present).
    let project_id = create_project(&server, &user, "b2-proj").await;
    let file_id = upload_text(&server, &user, "notes.txt", "B2_SECRET_MARKER content").await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    let (conv_id, branch_id) = create_conversation(&server, &user, &model_id).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;

    // Disable the files_mcp `read_file` TOOL in THIS conversation.
    let files_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"files.ziee.internal");
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [],
            "disabled_servers": [{ "server_id": files_id, "tools": ["read_file"] }],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "set conversation disabled_servers");

    // The model calls read_file → must be refused at call time.
    let body = send_and_collect(
        &server,
        &user,
        &conv_id,
        &branch_id,
        &model_id,
        "STUB_PLAN=read_first_file read it",
    )
    .await;

    // read_file WAS attached (there is no attach-time filter for disabled_servers),
    // so the refusal below is a CALL-time enforcement, not attach filtering.
    assert!(
        stub.requests_with_tool("read_file") >= 1,
        "read_file must be ATTACHED so the refusal is call-time enforcement; requests={:?}",
        stub.requests()
    );
    // The disabled tool must NOT return the file content ...
    assert!(
        !body.contains("B2_SECRET_MARKER"),
        "a disabled tool must NOT return the file content; body={body}"
    );
    // ... and the tool result must surface the 'disabled in this conversation' refusal.
    assert!(
        body.to_lowercase().contains("disabled"),
        "the tool result must carry the disabled-in-conversation refusal; body={body}"
    );
}
