//! Tier-6: sandbox round-trip → per-turn version-back.
//!
//! Exercises the FULL production path that the unit/HTTP tests can't reach: an
//! editable text file is attached to a conversation, a chat turn drives the stub
//! model to call the code_sandbox `write_file` tool (which overwrites the
//! copied-in workspace copy), and the `after_llm_call` version-back commits a
//! new version of the BACKING file (not an orphan).
//!
//! Gated by `enabled_test_server()` — returns `None` (clean skip, NOT
//! `#[ignore]`) when no rootfs/bwrap is available; runs on Linux CI with the
//! published sandbox rootfs.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::harness::enabled_test_server;
use crate::common::TestServer;
use crate::common::chat_stream_probe::ChatStreamProbe;
use crate::common::stub_chat::{StubChat, register_stub_model};
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

/// Deterministic id of the built-in code_sandbox MCP server (matches
/// `code_sandbox::code_sandbox_server_id()`), computed inline to avoid coupling
/// the integration crate to the lib's module visibility.
fn sandbox_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
}

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
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED);
    resp.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn post(server: &TestServer, token: &str, path: &str, body: Value) -> Value {
    let resp = reqwest::Client::new()
        .post(server.api_url(path))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("post");
    assert!(
        resp.status().is_success(),
        "POST {path} -> {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.unwrap_or(Value::Null)
}

#[tokio::test]
async fn sandbox_write_file_versions_back_the_backing_file() {
    let Some(server) = enabled_test_server().await else {
        return; // no rootfs/bwrap on this host — skip cleanly
    };
    let stub = StubChat::start().await;
    let user = create_user_with_permissions(&server, "sb_version_back", &["*"]).await;
    let model_id =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;

    // Editable text file, attached to a conversation via a project so it lands in
    // the conversation's effective file set (→ copied RW into the workspace).
    let file_id = upload_text(&server, &user, "notes.txt", "original line\n").await;
    let project = post(&server, &user.token, "/projects", json!({ "name": "sb-vb" })).await;
    let project_id = project["id"].as_str().unwrap().to_string();
    post(
        &server,
        &user.token,
        &format!("/projects/{project_id}/files"),
        json!({ "file_id": file_id }),
    )
    .await;
    let conv = post(&server, &user.token, "/conversations", json!({ "model_id": model_id })).await;
    let conv_id = conv["id"].as_str().unwrap().to_string();
    let branch_id = conv["active_branch_id"].as_str().unwrap().to_string();
    post(
        &server,
        &user.token,
        &format!("/projects/{project_id}/conversations/{conv_id}"),
        json!({}),
    )
    .await;

    // Drive a turn: the stub model calls the sandbox write_file tool, overwriting
    // the copied-in `notes.txt`. The per-turn version-back then commits v2.
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();
    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv_uuid)).await;
    let payload = json!({
        "content": "STUB_PLAN=sandbox_write_file STUB_FILE=notes.txt STUB_CONTENT=edited-in-sandbox edit my notes",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": sandbox_server_id(), "tools": [] } ] },
    });
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("send message");
    assert!(
        resp.status().is_success(),
        "send: {}",
        resp.text().await.unwrap_or_default()
    );
    // Wait for the turn (incl. tool execution) to finish — version-back runs in
    // after_llm_call once the tool loop completes.
    let _ = probe
        .collect_until_terminal(conv_uuid, Duration::from_secs(120))
        .await;

    // The backing file now has a 2nd version, authored by the sandbox.
    let versions: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = versions.as_array().expect("versions array");
    assert!(
        arr.len() >= 2,
        "in-sandbox edit should version-back a new version of the same file: {versions}"
    );
    assert_eq!(
        arr[0]["created_by"].as_str(),
        Some("sandbox"),
        "the head version should be authored by the sandbox version-back"
    );

    // And it's the same file_id (not an orphan).
    assert_eq!(arr[0]["file_id"].as_str(), Some(file_id.as_str()));
}

/// Per-turn COALESCING: two `write_file` calls to the same file within ONE turn
/// (the MCP tool loop iterates) must version-back as a SINGLE new version
/// holding the FINAL content — not two versions. The version-back fires once,
/// in `after_llm_call`, after the whole tool loop completes.
#[tokio::test]
async fn sandbox_two_writes_in_one_turn_coalesce_to_one_version() {
    let Some(server) = enabled_test_server().await else {
        return; // no rootfs/bwrap on this host — skip cleanly
    };
    let stub = StubChat::start().await;
    let user = create_user_with_permissions(&server, "sb_coalesce", &["*"]).await;
    let model_id =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;

    let file_id = upload_text(&server, &user, "notes.txt", "original line\n").await;
    let project = post(&server, &user.token, "/projects", json!({ "name": "sb-coalesce" })).await;
    let project_id = project["id"].as_str().unwrap().to_string();
    post(
        &server,
        &user.token,
        &format!("/projects/{project_id}/files"),
        json!({ "file_id": file_id }),
    )
    .await;
    let conv = post(&server, &user.token, "/conversations", json!({ "model_id": model_id })).await;
    let conv_id = conv["id"].as_str().unwrap().to_string();
    let branch_id = conv["active_branch_id"].as_str().unwrap().to_string();
    post(
        &server,
        &user.token,
        &format!("/projects/{project_id}/conversations/{conv_id}"),
        json!({}),
    )
    .await;

    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();
    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv_uuid)).await;
    let payload = json!({
        "content": "STUB_PLAN=sandbox_write_file_twice STUB_FILE=notes.txt \
                    STUB_CONTENT1=first-write STUB_CONTENT2=second-and-final edit twice",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": sandbox_server_id(), "tools": [] } ] },
    });
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("send message");
    assert!(resp.status().is_success(), "send: {}", resp.text().await.unwrap_or_default());
    let _ = probe
        .collect_until_terminal(conv_uuid, Duration::from_secs(120))
        .await;

    // EXACTLY two versions: v1 (upload) + v2 (the coalesced sandbox commit). Two
    // in-turn writes must NOT produce v2 AND v3.
    let versions: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = versions.as_array().expect("versions array");
    assert_eq!(
        arr.len(),
        2,
        "two in-turn writes must coalesce to ONE new version (v1+v2), got: {versions}"
    );
    assert_eq!(arr[0]["version"].as_i64(), Some(2));
    assert_eq!(arr[0]["created_by"].as_str(), Some("sandbox"));

    // The head holds the FINAL write's content, not the first.
    let (_, text) = {
        let r = reqwest::Client::new()
            .get(server.api_url(&format!("/files/{file_id}/text")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();
        (r.status(), r.text().await.unwrap_or_default())
    };
    assert!(
        text.contains("second-and-final") && !text.contains("first-write"),
        "head must hold the final write's content: {text}"
    );
}

/// NO-SPAM: a turn that leaves the staged file BYTE-IDENTICAL to its base must
/// NOT version-back. Turn 1 writes content A (→ v2); turn 2 writes the SAME A
/// (the workspace copy persists across turns, so it's unchanged) → the
/// checksum-equal guard in `reconcile_workspace_versions` must skip it: still v2,
/// no v3. (Guards the checksum comparison that the artifact-checksum fix repairs.)
#[tokio::test]
async fn sandbox_unchanged_file_does_not_version_back() {
    let Some(server) = enabled_test_server().await else {
        return; // no rootfs/bwrap on this host — skip cleanly
    };
    let stub = StubChat::start().await;
    let user = create_user_with_permissions(&server, "sb_noop", &["*"]).await;
    let model_id =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;

    let file_id = upload_text(&server, &user, "notes.txt", "original line\n").await;
    let project = post(&server, &user.token, "/projects", json!({ "name": "sb-noop" })).await;
    let project_id = project["id"].as_str().unwrap().to_string();
    post(
        &server,
        &user.token,
        &format!("/projects/{project_id}/files"),
        json!({ "file_id": file_id }),
    )
    .await;
    let conv = post(&server, &user.token, "/conversations", json!({ "model_id": model_id })).await;
    let conv_id = conv["id"].as_str().unwrap().to_string();
    let branch_id = conv["active_branch_id"].as_str().unwrap().to_string();
    post(
        &server,
        &user.token,
        &format!("/projects/{project_id}/conversations/{conv_id}"),
        json!({}),
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // Two turns, BOTH writing the identical content "stable-body".
    for _ in 0..2 {
        let mut probe = ChatStreamProbe::open(&server, &user.token).await;
        probe.subscribe(Some(conv_uuid)).await;
        let payload = json!({
            "content": "STUB_PLAN=sandbox_write_file STUB_FILE=notes.txt STUB_CONTENT=stable-body write it",
            "model_id": model_id,
            "branch_id": branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": sandbox_server_id(), "tools": [] } ] },
        });
        let resp = reqwest::Client::new()
            .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&payload)
            .send()
            .await
            .expect("send message");
        assert!(resp.status().is_success(), "send: {}", resp.text().await.unwrap_or_default());
        let _ = probe
            .collect_until_terminal(conv_uuid, Duration::from_secs(120))
            .await;
    }

    // v1 (upload) + v2 (turn 1's write). Turn 2 was a byte-identical no-op → no v3.
    let versions: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = versions.as_array().expect("versions array");
    assert_eq!(
        arr.len(),
        2,
        "an unchanged second turn must NOT version-back (expected v1+v2, no v3): {versions}"
    );
}
