// ============================================================================
// files_mcp built-in MCP server tests.
//
// Tests the JSON-RPC handler at /api/files/mcp (Track A):
//   - initialize / tools/list return the 3 read-only tools.
//   - tools/call requires the x-conversation-id header (conversation-scoped).
//   - the handler is gated on `files::read` (granted to all users by default).
//
// The `tools/call` round-trips against a REAL conversation with project files
// are exercised below (cross-cutting-04): list_files / read_file (offset+limit
// line slicing) / grep_files / the AMBIGUOUS_NAME + MISSING_TARGET + UNKNOWN_TOOL
// error classes / cross-conversation ownership. These reach the FIXED handler
// (`file_type`-based dispatch + `app_error_to_jsonrpc` mapping) directly over
// HTTP, so they don't need the stub chat provider — they POST the JSON-RPC
// `tools/call` with the `x-conversation-id` header exactly as the built-in MCP
// client does. The agentic chat-loop round-trip (manifest → read_file →
// continuation) lives in `tests/agentic_chat/mod.rs`.
// ============================================================================

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::sync_probe::SyncProbe;
use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

// File-versioning tests (edit tools → versions, restore, reproducibility,
// version-pinned reads) live in a submodule so they can reuse the private
// helpers in this module via `super::`.
mod versioning_test;

// ── small inline helpers (replicated from agentic_chat::mod, which keeps its
//    helpers private) ─────────────────────────────────────────────────────────

/// A user with a broad permission grant (`*`) so the one identity can create
/// projects, upload files, attach them, and create conversations.
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

/// Create a project; returns its id.
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
async fn attach_file_to_project(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
    file_id: &str,
) {
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

/// Create a conversation; returns its id.
async fn create_conversation(server: &TestServer, user: &TestUser) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
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
    v["id"].as_str().unwrap().to_string()
}

/// Attach a conversation to a project (so its knowledge files become the
/// conversation's effective file set).
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
    assert!(
        resp.status().is_success(),
        "attach conv to project: {}",
        resp.text().await.unwrap_or_default()
    );
}

/// Build a project conversation seeded with the given `(filename, body)` files,
/// all attached as project knowledge files. Returns `(conversation_id,
/// file_ids)` where `file_ids` are in the same order as `files`.
async fn project_conversation_with_files(
    server: &TestServer,
    user: &TestUser,
    slug: &str,
    files: &[(&str, &str)],
) -> (String, Vec<String>) {
    let project_id = create_project(server, user, slug).await;
    let mut file_ids = Vec::new();
    for (name, body) in files {
        let id = upload_text(server, user, name, body).await;
        attach_file_to_project(server, user, &project_id, &id).await;
        file_ids.push(id);
    }
    let conv_id = create_conversation(server, user).await;
    attach_conversation_to_project(server, user, &project_id, &conv_id).await;
    (conv_id, file_ids)
}

/// Send a `tools/call` JSON-RPC request and return the parsed response body.
async fn call_tool(
    server: &TestServer,
    user: &TestUser,
    conversation_id: Uuid,
    name: &str,
    arguments: Value,
) -> Value {
    let res = jsonrpc_call(
        server,
        &user.token,
        Some(conversation_id),
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200, "tools/call HTTP status");
    res.json().await.unwrap()
}

fn jsonrpc_call(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Option<Uuid>,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/files/mcp"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }));
    if let Some(cid) = conversation_id {
        req = req.header("x-conversation-id", cid.to_string());
    }
    req
}

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "files_mcp_init",
        &["files::read"],
    )
    .await;
    let res = jsonrpc_call(&server, &user.token, None, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["result"]["serverInfo"]["name"], "files");
}

#[tokio::test]
async fn test_tools_list_returns_read_and_write_tools() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "files_mcp_list",
        &["files::read"],
    )
    .await;
    let res = jsonrpc_call(&server, &user.token, None, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let tools = body["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    // 4 read tools (list/read/grep/semantic_search) + 5 write tools
    // (create/edit/edit_lines/rewrite + convert_document).
    assert_eq!(names.len(), 9, "4 read + 5 write tools: {names:?}");
    for t in [
        "list_files",
        "read_file",
        "grep_files",
        "semantic_search",
        "create_file",
        "edit_file",
        "edit_file_lines",
        "rewrite_file",
        "convert_document",
    ] {
        assert!(names.contains(&t), "missing tool {t}; got {names:?}");
    }
}

#[tokio::test]
async fn test_tools_call_requires_conversation_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "files_mcp_noconv",
        &["files::read"],
    )
    .await;
    // No x-conversation-id header → tools/call must error (these tools are
    // conversation-scoped), not silently operate on nothing.
    let res = jsonrpc_call(
        &server,
        &user.token,
        None,
        "tools/call",
        json!({ "name": "list_files", "arguments": {} }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object(),
        "tools/call without x-conversation-id should return a JSON-RPC error, got: {body}"
    );
}

// ── cross-cutting-04: tools/call round-trips over a real conversation ─────────
//
// Standard JSON-RPC error codes the FIXED `app_error_to_jsonrpc` maps to:
//   -32601 method_not_found (UNKNOWN_TOOL); -32602 invalid_params (every other
//   400 bad_request — MISSING_TARGET / AMBIGUOUS_NAME / INVALID_ARGS — AND 404
//   not_found — cross-conversation ownership, no-such-name).
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// (1) `list_files` over a project conversation returns the attached file's
/// id + name in the `structuredContent.files` manifest.
#[tokio::test]
async fn test_list_files_returns_attached_file() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_list_call").await;
    let (conv_id, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "list-call-project",
        &[("notes.txt", "alpha beta gamma\nsecond line here\n")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(&server, &user, conv_uuid, "list_files", json!({})).await;
    assert!(body["error"].is_null(), "list_files should succeed; body={body}");
    let files = body["result"]["structuredContent"]["files"]
        .as_array()
        .expect("files array");
    assert_eq!(files.len(), 1, "exactly one project file; files={files:?}");
    assert_eq!(files[0]["id"].as_str().unwrap(), file_ids[0].as_str());
    assert_eq!(files[0]["name"].as_str().unwrap(), "notes.txt");
    // The text content also serializes the manifest, so the id is human-visible.
    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains(file_ids[0].as_str()), "manifest text carries the id");
}

/// (2) `read_file` by id with `offset`/`limit` returns the right LINE slice with
/// `line_start`/`line_end`/`total_lines` metadata + a continuation marker (this
/// is the A-correctness-01 regression: a text file has `pages == 1`, so the
/// fixed dispatch must route it to the line reader, NOT the page reader).
#[tokio::test]
async fn test_read_file_offset_limit_line_slice() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_read_slice").await;

    // 400 numbered lines so offset=200/limit=100 returns lines 201..300 with a
    // "more" continuation marker (300 < 400).
    let body_text: String = (1..=400)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let (conv_id, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "read-slice-project",
        &[("big.txt", &body_text)],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(
        &server,
        &user,
        conv_uuid,
        "read_file",
        json!({ "id": file_ids[0], "offset": 200, "limit": 100 }),
    )
    .await;
    assert!(body["error"].is_null(), "read_file should succeed; body={body}");

    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["line_start"].as_i64().unwrap(), 201, "1-based start line");
    assert_eq!(sc["line_end"].as_i64().unwrap(), 300, "exclusive-style end == 300");
    assert_eq!(sc["total_lines"].as_i64().unwrap(), 400);

    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("line 201"), "slice starts at line 201; text={text}");
    assert!(text.contains("line 300"), "slice includes line 300");
    assert!(!text.contains("line 200\n"), "line 200 is BEFORE the slice");
    assert!(!text.contains("line 301"), "line 301 is AFTER the slice");
    // Continuation marker since 300 < 400.
    assert!(
        text.contains("of 400") && text.contains("offset=300"),
        "continuation marker points at the next offset; text={text}"
    );
}

/// (3a) AMBIGUOUS_NAME: two project files share a filename (distinct content →
/// distinct checksums → both survive content-dedup). `read_file(name=...)`
/// can't disambiguate → 400 bad_request → JSON-RPC invalid_params (-32602).
#[tokio::test]
async fn test_read_file_ambiguous_name_errors() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_ambiguous").await;
    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &user,
        "ambiguous-project",
        &[
            ("dup.txt", "first file body content AAA"),
            ("dup.txt", "second file body content BBB"),
        ],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(
        &server,
        &user,
        conv_uuid,
        "read_file",
        json!({ "name": "dup.txt" }),
    )
    .await;
    let err = &body["error"];
    assert!(err.is_object(), "ambiguous name must error; body={body}");
    assert_eq!(err["code"].as_i64().unwrap(), INVALID_PARAMS, "client-class error");
    assert!(
        err["message"].as_str().unwrap().contains("matches"),
        "message names the ambiguity + candidate ids; err={err}"
    );
}

/// (3b) MISSING_TARGET: neither `id` nor `name` supplied → 400 → invalid_params.
#[tokio::test]
async fn test_read_file_missing_target_errors() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_missing").await;
    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &user,
        "missing-target-project",
        &[("only.txt", "content")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(&server, &user, conv_uuid, "read_file", json!({})).await;
    let err = &body["error"];
    assert!(err.is_object(), "missing target must error; body={body}");
    assert_eq!(err["code"].as_i64().unwrap(), INVALID_PARAMS);
    assert!(
        err["message"].as_str().unwrap().contains("id")
            && err["message"].as_str().unwrap().contains("name"),
        "message tells the model to pass id or name; err={err}"
    );
}

/// convert_document rejects empty/whitespace markdown BEFORE any pandoc render
/// (infra-free validation) → invalid_params, not a server error.
#[tokio::test]
async fn test_convert_document_empty_markdown_errors() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_convert_empty").await;
    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &user,
        "convert-empty-project",
        &[("only.txt", "content")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(
        &server,
        &user,
        conv_uuid,
        "convert_document",
        json!({ "markdown": "   " }),
    )
    .await;
    let err = &body["error"];
    assert!(err.is_object(), "empty markdown must error; body={body}");
    assert_eq!(err["code"].as_i64().unwrap(), INVALID_PARAMS, "client-class error");
    assert!(
        err["message"].as_str().unwrap_or("").contains("markdown"),
        "message names the empty markdown arg; err={err}"
    );
}

/// (4a) `grep_files` returns matching lines with file/page/line references.
#[tokio::test]
async fn test_grep_files_hits() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_grep_hit").await;
    let (conv_id, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "grep-hit-project",
        &[(
            "log.txt",
            "alpha line one\nNEEDLE appears here\nbeta line three\nanother NEEDLE on this line\n",
        )],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(
        &server,
        &user,
        conv_uuid,
        "grep_files",
        json!({ "pattern": "NEEDLE" }),
    )
    .await;
    assert!(body["error"].is_null(), "grep should succeed; body={body}");
    let matches = body["result"]["structuredContent"]["matches"]
        .as_array()
        .expect("matches array");
    assert_eq!(matches.len(), 2, "two lines match NEEDLE; matches={matches:?}");
    assert_eq!(matches[0]["file_id"].as_str().unwrap(), file_ids[0].as_str());
    assert_eq!(matches[0]["name"].as_str().unwrap(), "log.txt");
    assert_eq!(
        body["result"]["structuredContent"]["truncated"].as_bool().unwrap(),
        false,
        "two matches is well under the 200 cap"
    );
}

/// (4b) A malformed regex (unbalanced `(`) must NOT error — it falls back to a
/// LITERAL (escaped) substring match. The literal `error(` text matches the body.
#[tokio::test]
async fn test_grep_files_malformed_regex_literal_fallback() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_grep_malformed").await;
    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &user,
        "grep-malformed-project",
        &[("src.txt", "line one\ncall error(code) here\nline three\n")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // `error(` is an invalid regex (unterminated group) → literal fallback.
    let body = call_tool(
        &server,
        &user,
        conv_uuid,
        "grep_files",
        json!({ "pattern": "error(" }),
    )
    .await;
    assert!(
        body["error"].is_null(),
        "malformed regex must fall back to literal, not error; body={body}"
    );
    let matches = body["result"]["structuredContent"]["matches"]
        .as_array()
        .expect("matches array");
    assert_eq!(matches.len(), 1, "the literal `error(` matches one line; matches={matches:?}");
    assert!(
        matches[0]["text"].as_str().unwrap().contains("error(code)"),
        "the matched line carries the literal token; matches={matches:?}"
    );
}

/// (4c) An empty `pattern` is rejected up front with INVALID_ARGS → 400 →
/// invalid_params (-32602), NOT a silent whole-corpus scan.
#[tokio::test]
async fn test_grep_files_empty_pattern_errors() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_grep_empty").await;
    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &user,
        "grep-empty-project",
        &[("doc.txt", "some text")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(
        &server,
        &user,
        conv_uuid,
        "grep_files",
        json!({ "pattern": "" }),
    )
    .await;
    let err = &body["error"];
    assert!(err.is_object(), "empty pattern must error; body={body}");
    assert_eq!(err["code"].as_i64().unwrap(), INVALID_PARAMS);
    assert!(
        err["message"].as_str().unwrap().contains("pattern"),
        "message names the empty pattern; err={err}"
    );
}

/// (5) Cross-conversation ownership: user B cannot read user A's conversation
/// files. The handler returns NOT_FOUND for a foreign conversation (no
/// existence leak) → 404 → JSON-RPC invalid_params (-32602).
#[tokio::test]
async fn test_cross_conversation_ownership_errors() {
    let server = TestServer::start().await;
    let owner = power_user(&server, "files_mcp_owner").await;
    let intruder = power_user(&server, "files_mcp_intruder").await;

    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &owner,
        "owner-project",
        &[("private.txt", "owner-only content")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // The intruder targets the owner's conversation via the header.
    let body = call_tool(&server, &intruder, conv_uuid, "list_files", json!({})).await;
    let err = &body["error"];
    assert!(
        err.is_object(),
        "a foreign conversation must error, not list the owner's files; body={body}"
    );
    assert_eq!(err["code"].as_i64().unwrap(), INVALID_PARAMS, "404 maps to invalid_params");
}

/// `tools/call` with an unknown tool name → UNKNOWN_TOOL → method_not_found
/// (-32601), distinct from a bad-args invalid_params.
#[tokio::test]
async fn test_unknown_tool_method_not_found() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_unknown_tool").await;
    let (conv_id, _ids) = project_conversation_with_files(
        &server,
        &user,
        "unknown-tool-project",
        &[("a.txt", "x")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let body = call_tool(&server, &user, conv_uuid, "no_such_tool", json!({})).await;
    let err = &body["error"];
    assert!(err.is_object(), "unknown tool must error; body={body}");
    assert_eq!(
        err["code"].as_i64().unwrap(),
        METHOD_NOT_FOUND,
        "an unknown tool is method_not_found, not invalid_params; err={err}"
    );
}

// ── realtime-sync emission ──────────────────────────────────────────────────
//
// Verifies that `edit_file` (the files_mcp write tool that edits an existing
// file) emits a `publish_file_changed` sync event that reaches the owner's SSE
// stream and is isolated from other users.

#[tokio::test]
async fn test_edit_file_emits_sync_event_to_owner_only() {
    let server = TestServer::start().await;
    let alice = power_user(&server, "files_mcp_edit_sync_a").await;
    let bob = power_user(&server, "files_mcp_edit_sync_b").await;

    let mut alice_probe = SyncProbe::open(&server, &alice.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let conv_id = create_conversation(&server, &alice).await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // Create a file first so we have something to edit.
    let create_body = call_tool(
        &server,
        &alice,
        conv_uuid,
        "create_file",
        json!({ "filename": "editable.txt", "content": "original content\n" }),
    )
    .await;
    assert!(create_body["error"].is_null(), "create_file should succeed; body={create_body}");
    let file_id = create_body["result"]["structuredContent"]["file_id"]
        .as_str()
        .expect("file_id")
        .to_string();

    // Now edit the file using old_str/new_str.
    let edit_body = call_tool(
        &server,
        &alice,
        conv_uuid,
        "edit_file",
        json!({ "id": file_id, "old_str": "original content", "new_str": "edited content" }),
    )
    .await;
    assert!(edit_body["error"].is_null(), "edit_file should succeed; body={edit_body}");
    let edit_file_id = edit_body["result"]["structuredContent"]["file_id"]
        .as_str()
        .expect("file_id")
        .to_string();
    assert_eq!(edit_file_id, file_id, "edit must reference the same file id");

    // Alice's tab receives the file/update sync event.
    let frame = alice_probe
        .expect_event("file", "update", Duration::from_secs(5))
        .await;
    assert_eq!(
        frame.id, file_id,
        "sync event must carry the edited file's id"
    );

    // Owner-scoped: Bob must NOT receive Alice's event.
    bob_probe.expect_silence(Duration::from_secs(1)).await;
}

/// Multi-turn model-authored-file workflow: the model AUTHORS a file via
/// `create_file`, and that file is then available to LATER tool calls in the
/// SAME conversation — `list_files` enumerates it and `read_file` returns its
/// authored content. Exercises the production files_mcp write→read loop the
/// model drives across turns (deterministic via call_tool, no LLM).
#[tokio::test]
async fn test_model_authored_file_is_readable_in_later_turn() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_authored").await;
    let conv_id = create_conversation(&server, &user).await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // Turn 1 — the model authors a file.
    let created = call_tool(
        &server,
        &user,
        conv_uuid,
        "create_file",
        json!({ "filename": "authored.md", "content": "AUTHORED_MARKER first line\nsecond line\n" }),
    )
    .await;
    assert!(created["error"].is_null(), "create_file should succeed; body={created}");
    let file_id = created["result"]["structuredContent"]["file_id"]
        .as_str()
        .expect("file_id")
        .to_string();

    // Turn 2 — the authored file is now in the conversation's manifest.
    let listed = call_tool(&server, &user, conv_uuid, "list_files", json!({})).await;
    assert!(listed["error"].is_null(), "list_files should succeed; body={listed}");
    let files = listed["result"]["structuredContent"]["files"]
        .as_array()
        .expect("files array");
    assert!(
        files.iter().any(|f| f["id"].as_str() == Some(file_id.as_str())
            && f["name"].as_str() == Some("authored.md")),
        "the model-authored file must appear in a later turn's manifest; files={files:?}"
    );

    // Turn 3 — read the authored content back by id.
    let read = call_tool(
        &server,
        &user,
        conv_uuid,
        "read_file",
        json!({ "id": file_id }),
    )
    .await;
    assert!(read["error"].is_null(), "read_file should succeed; body={read}");
    let text = read["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("AUTHORED_MARKER"),
        "read_file must return the model-authored content; got: {text}"
    );
}

/// Upload raw BYTES with an explicit mime type; returns the new file id.
async fn upload_bytes(server: &TestServer, user: &TestUser, filename: &str, bytes: Vec<u8>, mime: &str) -> String {
    use reqwest::multipart;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(bytes).file_name(filename.to_string()).mime_str(mime).unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload bytes");
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED, "upload: {}", resp.text().await.unwrap_or_default());
    resp.json::<Value>().await.unwrap()["id"].as_str().unwrap().to_string()
}

/// read_file on an IMAGE returns an image content block (handlers.rs:545-560);
/// on a BINARY (no text layer) returns a "[… no extractable text]" note
/// (handlers.rs:561-575). Prior read_file tests only covered text line-slicing.
#[tokio::test]
async fn test_read_file_image_and_binary_branches() {
    use base64::Engine;
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_types").await;
    let project_id = create_project(&server, &user, "types-project").await;
    let conv_id = create_conversation(&server, &user).await;
    attach_conversation_to_project(&server, &user, &project_id, &conv_id).await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // A 1x1 PNG → FileType::Image.
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
        .unwrap();
    let img_id = upload_bytes(&server, &user, "dot.png", png, "image/png").await;
    attach_file_to_project(&server, &user, &project_id, &img_id).await;

    let img = call_tool(&server, &user, conv_uuid, "read_file", json!({ "id": img_id })).await;
    assert!(img["error"].is_null(), "read_file(image) should succeed; body={img}");
    let block = &img["result"]["content"][0];
    assert_eq!(block["type"], "image", "an image file must read back as an image block; got {img}");
    assert_eq!(block["mimeType"], "image/png");
    assert!(block["data"].as_str().is_some_and(|d| !d.is_empty()), "image data must be base64");

    // A non-UTF8 binary blob → FileType::Binary (no extractable text).
    let bin_id = upload_bytes(&server, &user, "blob.bin", vec![0u8, 159, 146, 150, 1, 2, 3], "application/octet-stream").await;
    attach_file_to_project(&server, &user, &project_id, &bin_id).await;
    let bin = call_tool(&server, &user, conv_uuid, "read_file", json!({ "id": bin_id })).await;
    assert!(bin["error"].is_null(), "read_file(binary) should succeed; body={bin}");
    let text = bin["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("no extractable text"),
        "a binary file must read back as a no-text note; got: {text}"
    );
}

/// grep_files caps results at GREP_MAX_MATCHES (200) and sets truncated=true when
/// scanning stops early. A file with >200 matching lines exercises the cap +
/// byte-budget/truncation path (handlers.rs grep_files), previously untested.
#[tokio::test]
async fn test_grep_files_truncates_at_match_cap() {
    let server = TestServer::start().await;
    let user = power_user(&server, "files_mcp_grep_cap").await;
    // 300 lines each containing the marker → exceeds the 200-match cap.
    let body = (0..300).map(|i| format!("MATCHME line {i}")).collect::<Vec<_>>().join("\n");
    let (conv_id, _ids) =
        project_conversation_with_files(&server, &user, "grep-cap", &[("many.txt", &body)]).await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    let res = call_tool(&server, &user, conv_uuid, "grep_files", json!({ "pattern": "MATCHME" })).await;
    assert!(res["error"].is_null(), "grep_files should succeed; body={res}");
    let sc = &res["result"]["structuredContent"];
    assert_eq!(sc["truncated"], true, "grep over 300 matches must report truncated; got {sc}");
    let matches = sc["matches"].as_array().expect("matches array");
    assert!(matches.len() <= 200, "matches must be capped at GREP_MAX_MATCHES; got {}", matches.len());
}
