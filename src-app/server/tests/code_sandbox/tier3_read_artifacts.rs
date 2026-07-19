//! Tier 3 — code_sandbox read tools resolve tool-produced ("model-authored")
//! artifacts.
//!
//! The reported bug: a tool returns an artifact file (created via MCP
//! `resource_link` / `save_as_artifact`, `created_by IN ('mcp','llm')`), and
//! code_sandbox `read_file({filename})` fails with the opaque
//! `{"code":-32603,"message":"tool read_file failed"}`. These tests drive the
//! REAL HTTP path (`POST /api/code-sandbox`) against a live TestServer — the
//! only place the model-authored resolution + storage load + JSON-RPC error
//! mapping run for real. `read_file`/`edit_file`/`list_files` need NO
//! rootfs/bwrap (only `execute_command` does), so these run without a sandbox.
//!
//! `mcp` fixtures are authored via the real files-MCP `create_file` path (with
//! the `x-message-id` assistant-turn provenance the chat path injects); the
//! `llm` arm + the two-same-name ambiguity case are produced with `upload_text`
//! + a SQL "promote to model-authored" (real storage blob, provenance re-pointed
//! at the conversation) — the resolver treats `mcp`/`llm` identically.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::test_helpers::{create_user_with_permissions, TestUser};
use crate::common::{TestServer, TestServerOptions};

// ── fixtures ────────────────────────────────────────────────────────────────

/// Boot a TestServer with code_sandbox ENABLED so `/api/code-sandbox` is
/// registered (state set) and `read_file`/`edit_file`/`list_files` dispatch.
/// These tools resolve files + workspace directly and never touch bwrap/rootfs
/// (only `execute_command` does), and the boot defers all rootfs probes to the
/// first `execute_command` — so a placeholder `sandbox_rootfs` dir (never
/// mounted here) is all that's needed. No network, no rootfs download.
async fn enabled_server() -> TestServer {
    // One shared placeholder rootfs dir — never mounted (the read tools defer
    // all rootfs work to execute_command), created idempotently so repeated
    // test boots don't leak a fresh dir each.
    let rootfs = std::env::temp_dir().join("ziee-cs-artifacts-rootfs-placeholder");
    std::fs::create_dir_all(&rootfs).expect("placeholder rootfs dir");
    TestServer::start_with_options(TestServerOptions {
        sandbox_enabled: true,
        sandbox_rootfs: Some(rootfs),
        ..Default::default()
    })
    .await
}

async fn power_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["*"]).await
}

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::PgPool::connect(&server.database_url).await.unwrap()
}

async fn create_conversation(server: &TestServer, user: &TestUser) -> Uuid {
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
    Uuid::parse_str(v["id"].as_str().unwrap()).unwrap()
}

/// Insert an assistant message joined to the conversation's default branch and
/// return its id — the provenance stamp (`x-message-id` →
/// `file_versions.source_message_id`) that scopes a model-authored file to this
/// conversation. Mirrors `tests/files_mcp/mod.rs::assistant_message`.
async fn assistant_message(server: &TestServer, conversation_id: Uuid) -> Uuid {
    let pool = pool(server).await;
    let branch_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM branches WHERE conversation_id = $1 ORDER BY created_at LIMIT 1",
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await
    .expect("conversation must have a default branch");
    let msg_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, role, originated_from_id, created_at) \
         VALUES ($1, 'assistant', $1, NOW())",
    )
    .bind(msg_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO branch_messages (branch_id, message_id, created_at) VALUES ($1, $2, NOW())",
    )
    .bind(branch_id)
    .bind(msg_id)
    .execute(&pool)
    .await
    .unwrap();
    msg_id
}

/// Author a model-authored `mcp` file via the real files-MCP `create_file`
/// (stages a storage blob + stamps `created_by='mcp'` + `source_message_id`).
async fn create_file_mcp(
    server: &TestServer,
    user: &TestUser,
    conversation_id: Uuid,
    message_id: Uuid,
    filename: &str,
    content: &str,
) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", conversation_id.to_string())
        .header("x-message-id", message_id.to_string())
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "create_file", "arguments": { "filename": filename, "content": content } },
        }))
        .send()
        .await
        .expect("create_file");
    assert_eq!(resp.status(), 200, "create_file HTTP status");
    let body: Value = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "create_file should succeed; body={body}");
}

/// Upload a text file (real blob), then "promote" it to a model-authored
/// artifact scoped to the conversation: flip `files.created_by` and point the
/// head version's `source_message_id` at `message_id`. Returns the file id.
async fn upload_and_promote(
    server: &TestServer,
    user: &TestUser,
    message_id: Uuid,
    created_by: &str,
    filename: &str,
    body: &str,
) -> Uuid {
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
    let file_id = Uuid::parse_str(v["id"].as_str().unwrap()).unwrap();

    let pool = pool(server).await;
    sqlx::query("UPDATE files SET created_by = $2 WHERE id = $1")
        .bind(file_id)
        .bind(created_by)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE file_versions SET source_message_id = $2 WHERE file_id = $1 AND is_head = true",
    )
    .bind(file_id)
    .bind(message_id)
    .execute(&pool)
    .await
    .unwrap();
    file_id
}

fn endpoint(server: &TestServer) -> String {
    format!("{}/api/code-sandbox", server.base_url)
}

/// POST a code_sandbox `tools/call` and return the parsed JSON-RPC body.
async fn cs_call(
    server: &TestServer,
    user: &TestUser,
    conversation_id: Uuid,
    name: &str,
    arguments: Value,
) -> Value {
    let resp = reqwest::Client::new()
        .post(endpoint(server))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", conversation_id.to_string())
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        }))
        .send()
        .await
        .expect("code-sandbox tools/call");
    assert_eq!(resp.status(), 200, "tools/call HTTP status");
    resp.json().await.unwrap()
}

/// The text `read_file` returns (its `structuredContent.text`, numbered lines).
fn read_text(body: &Value) -> String {
    body["result"]["structuredContent"]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

// ── TEST-1 (ITEM-1): read a model-authored `mcp` artifact ───────────────────

#[tokio::test]
async fn read_file_reads_model_authored_mcp_artifact() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_read_mcp").await;
    let conv = create_conversation(&server, &user).await;
    let msg = assistant_message(&server, conv).await;
    create_file_mcp(&server, &user, conv, msg, "evaluation.json", "MCP_MARKER line one\nline two\n").await;

    let body = cs_call(&server, &user, conv, "read_file", json!({ "filename": "evaluation.json" })).await;
    assert!(body["error"].is_null(), "read_file must succeed on a tool artifact; body={body}");
    assert!(
        read_text(&body).contains("MCP_MARKER"),
        "read_file must return the artifact content; got: {}",
        read_text(&body)
    );
}

// ── TEST-2 (ITEM-1): read a model-authored `llm` artifact ───────────────────

#[tokio::test]
async fn read_file_reads_model_authored_llm_artifact() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_read_llm").await;
    let conv = create_conversation(&server, &user).await;
    let msg = assistant_message(&server, conv).await;
    upload_and_promote(&server, &user, msg, "llm", "report.txt", "LLM_MARKER content here\n").await;

    let body = cs_call(&server, &user, conv, "read_file", json!({ "filename": "report.txt" })).await;
    assert!(body["error"].is_null(), "read_file must succeed on an llm artifact; body={body}");
    assert!(read_text(&body).contains("LLM_MARKER"), "got: {}", read_text(&body));
}

// ── TEST-3 (ITEM-1): edit_file on a model-authored artifact ─────────────────

#[tokio::test]
async fn edit_file_edits_model_authored_artifact() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_edit").await;
    let conv = create_conversation(&server, &user).await;
    let msg = assistant_message(&server, conv).await;
    create_file_mcp(&server, &user, conv, msg, "notes.md", "ORIGINAL first\nORIGINAL second\n").await;

    // Replace line 1. The first edit copies the artifact into the workspace,
    // then edits the workspace copy.
    let edit = cs_call(
        &server,
        &user,
        conv,
        "edit_file",
        json!({ "filename": "notes.md", "start_line": 1, "end_line": 1, "new_content": "EDITED first" }),
    )
    .await;
    assert!(edit["error"].is_null(), "edit_file must succeed on a tool artifact; body={edit}");

    let read = cs_call(&server, &user, conv, "read_file", json!({ "filename": "notes.md" })).await;
    let text = read_text(&read);
    assert!(text.contains("EDITED first"), "edit must be reflected; got: {text}");
    assert!(text.contains("ORIGINAL second"), "untouched line kept; got: {text}");
}

// ── TEST-4 (ITEM-1): conversation-scope guard (no cross-conversation leak) ───

#[tokio::test]
async fn read_file_does_not_read_artifact_from_another_conversation() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_scope").await;
    let conv_a = create_conversation(&server, &user).await;
    let conv_b = create_conversation(&server, &user).await;
    let msg_a = assistant_message(&server, conv_a).await;
    create_file_mcp(&server, &user, conv_a, msg_a, "secret.txt", "CONV_A_ONLY marker\n").await;

    // Readable in A (the fix).
    let read_a = cs_call(&server, &user, conv_a, "read_file", json!({ "filename": "secret.txt" })).await;
    assert!(read_a["error"].is_null(), "must read in its own conversation; body={read_a}");
    assert!(read_text(&read_a).contains("CONV_A_ONLY"));

    // NOT readable in B — same user, different conversation.
    let read_b = cs_call(&server, &user, conv_b, "read_file", json!({ "filename": "secret.txt" })).await;
    assert!(read_b["error"].is_object(), "artifact from A must not read in B; body={read_b}");
    assert!(
        !serde_json::to_string(&read_b).unwrap().contains("CONV_A_ONLY"),
        "B must never see A's content; body={read_b}"
    );
}

// ── TEST-5 (ITEM-1): workspace-first wins over an artifact of the same name ──

#[tokio::test]
async fn read_file_prefers_workspace_over_model_authored() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_wsfirst").await;
    let conv = create_conversation(&server, &user).await;
    let msg = assistant_message(&server, conv).await;
    create_file_mcp(&server, &user, conv, msg, "dup.txt", "ARTIFACT_VERSION\n").await;

    // Write a workspace file of the same name.
    let w = cs_call(
        &server,
        &user,
        conv,
        "write_file",
        json!({ "filename": "dup.txt", "content": "WORKSPACE_VERSION\n" }),
    )
    .await;
    assert!(w["error"].is_null(), "write_file; body={w}");

    let read = cs_call(&server, &user, conv, "read_file", json!({ "filename": "dup.txt" })).await;
    let text = read_text(&read);
    assert!(text.contains("WORKSPACE_VERSION"), "workspace copy must win; got: {text}");
    assert!(!text.contains("ARTIFACT_VERSION"), "must not read the artifact; got: {text}");

    // And the fallback IS live for a name with no workspace shadow — so this
    // test fails if the model-authored fallback is removed (not just a
    // pre-existing workspace-first regression guard).
    create_file_mcp(&server, &user, conv, msg, "artifact-only.txt", "ONLY_ARTIFACT_VERSION\n").await;
    let read2 = cs_call(&server, &user, conv, "read_file", json!({ "filename": "artifact-only.txt" })).await;
    assert!(read2["error"].is_null(), "artifact-only name must resolve via the fallback; body={read2}");
    assert!(read_text(&read2).contains("ONLY_ARTIFACT_VERSION"), "got: {}", read_text(&read2));
}

// ── TEST-6 (ITEM-2, ITEM-3): missing file → actionable invalid_params ───────

#[tokio::test]
async fn read_file_missing_returns_actionable_error_no_host_path() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_missing").await;
    let conv = create_conversation(&server, &user).await;

    let body = cs_call(&server, &user, conv, "read_file", json!({ "filename": "nope.json" })).await;
    let err = &body["error"];
    assert!(err.is_object(), "a missing file must error; body={body}");
    // Not the pre-fix opaque -32603 "tool read_file failed".
    assert_eq!(err["code"].as_i64().unwrap(), -32602, "must be invalid_params, not -32603; err={err}");
    let msg = err["message"].as_str().unwrap_or_default();
    assert!(msg.contains("nope.json"), "names the missing file; err={err}");
    assert!(msg.contains("list_files"), "points at list_files; err={err}");
    assert_ne!(msg, "tool read_file failed", "must not be the opaque generic; err={err}");
    // No host path leaked.
    assert!(!msg.contains('/'), "must not leak a host path; err={err}");
}

// ── TEST-7 (ITEM-4): list_files surfaces model-authored artifacts ───────────

#[tokio::test]
async fn list_files_includes_model_authored_artifacts() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_list").await;
    let conv = create_conversation(&server, &user).await;
    let msg = assistant_message(&server, conv).await;
    create_file_mcp(&server, &user, conv, msg, "from-mcp.json", "MCP\n").await;
    upload_and_promote(&server, &user, msg, "llm", "from-llm.txt", "LLM\n").await;
    // A workspace file with the same name as a (separate) artifact → no dupe.
    upload_and_promote(&server, &user, msg, "mcp", "shared.txt", "ARTIFACT\n").await;
    let w = cs_call(&server, &user, conv, "write_file", json!({ "filename": "shared.txt", "content": "WS\n" })).await;
    assert!(w["error"].is_null(), "write_file; body={w}");
    // Two DISTINCT artifacts sharing a name (the AMBIGUOUS case read_file
    // rejects) must collapse to ONE list row — list must not advertise a name
    // twice that read_file can't resolve by name.
    upload_and_promote(&server, &user, msg, "mcp", "twins.txt", "one\n").await;
    upload_and_promote(&server, &user, msg, "llm", "twins.txt", "two\n").await;

    let body = cs_call(&server, &user, conv, "list_files", json!({})).await;
    assert!(body["error"].is_null(), "list_files must succeed; body={body}");
    let files = body["result"]["structuredContent"]["files"].as_array().expect("files array");
    let names: Vec<&str> = files.iter().filter_map(|f| f["name"].as_str()).collect();

    assert!(names.contains(&"from-mcp.json"), "mcp artifact must be listed; names={names:?}");
    assert!(names.contains(&"from-llm.txt"), "llm artifact must be listed; names={names:?}");
    // Exactly one `shared.txt` (workspace wins, no duplicate row).
    let shared = names.iter().filter(|n| **n == "shared.txt").count();
    assert_eq!(shared, 1, "same-named workspace file must not duplicate the artifact; names={names:?}");
    // Exactly one `twins.txt` (two same-named artifacts collapse to one row).
    let twins = names.iter().filter(|n| **n == "twins.txt").count();
    assert_eq!(twins, 1, "two same-named artifacts must not produce duplicate list rows; names={names:?}");
}

// ── TEST-9 (ITEM-1): two same-named artifacts → AMBIGUOUS (invalid_params) ───

#[tokio::test]
async fn read_file_ambiguous_model_authored_names_errors() {
    let server = enabled_server().await;
    let user = power_user(&server, "cs_ambig").await;
    let conv = create_conversation(&server, &user).await;
    let msg = assistant_message(&server, conv).await;
    // Two DISTINCT files sharing a filename, both model-authored + scoped here.
    upload_and_promote(&server, &user, msg, "mcp", "dup.csv", "first,copy\n").await;
    upload_and_promote(&server, &user, msg, "mcp", "dup.csv", "second,copy\n").await;

    let body = cs_call(&server, &user, conv, "read_file", json!({ "filename": "dup.csv" })).await;
    let err = &body["error"];
    assert!(err.is_object(), "ambiguous name must error; body={body}");
    assert_eq!(err["code"].as_i64().unwrap(), -32602, "surfaced as invalid_params; err={err}");
    let msg = err["message"].as_str().unwrap_or_default();
    assert!(msg.contains("dup.csv"), "names the ambiguous file; err={err}");
    // Positively assert the AMBIGUOUS branch, not just any -32602 (FILE_NOT_FOUND
    // is also -32602 and also names the file) — this wording is unique to it.
    assert!(
        msg.contains("cannot tell them apart") && msg.contains("tool-produced files"),
        "must be the AMBIGUOUS_FILENAME message, not not-found; err={err}"
    );
    assert_ne!(msg, "tool read_file failed", "must not be the opaque generic; err={err}");
}
