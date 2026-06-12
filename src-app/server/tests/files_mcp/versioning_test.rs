//! File-versioning integration tests — exercise the whole real stack:
//! `files_mcp` write tools create versions, the `/files/{id}/versions|head|
//! restore` REST API reads/restores them, and reproducibility (pinned versions
//! return their exact bytes after the head advances) holds end-to-end.
//!
//! Reuses the private helpers in `super` (`upload_text`, `call_tool`,
//! `create_conversation`, `power_user`, `jsonrpc_call`).

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{call_tool, create_conversation, power_user, upload_text};
use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

// ── REST helpers ─────────────────────────────────────────────────────────────

async fn get(server: &TestServer, token: &str, path: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url(path))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("GET")
}

async fn get_json(server: &TestServer, token: &str, path: &str) -> (StatusCode, Value) {
    let r = get(server, token, path).await;
    let s = r.status();
    let v = r.json().await.unwrap_or(Value::Null);
    (s, v)
}

async fn get_text(server: &TestServer, token: &str, path: &str) -> (StatusCode, String) {
    let r = get(server, token, path).await;
    let s = r.status();
    (s, r.text().await.unwrap_or_default())
}

async fn restore(server: &TestServer, token: &str, file_id: &str, version: i64) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/files/{file_id}/restore")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "version": version }))
        .send()
        .await
        .expect("restore")
}

/// Edit a file by id (str-replace) and return the raw JSON-RPC response.
async fn edit(server: &TestServer, user: &TestUser, conv: Uuid, file_id: &str, old: &str, new: &str) -> Value {
    call_tool(
        server,
        user,
        conv,
        "edit_file",
        json!({ "id": file_id, "old_str": old, "new_str": new }),
    )
    .await
}

fn structured(v: &Value) -> &Value {
    &v["result"]["structuredContent"]
}

// ── create / edit basic versioning ──────────────────────────────────────────

#[tokio::test]
async fn create_file_makes_v1_and_edit_advances_head() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_create").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();

    // create_file → a brand-new file at v1.
    let created = call_tool(
        &server,
        &user,
        conv,
        "create_file",
        json!({ "filename": "report.md", "content": "# Title\nalpha\n" }),
    )
    .await;
    assert!(created["error"].is_null(), "create_file: {created}");
    let file_id = structured(&created)["file_id"].as_str().unwrap().to_string();
    assert_eq!(structured(&created)["version"].as_i64(), Some(1));

    // One version so far.
    let (s, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(versions.as_array().unwrap().len(), 1);

    // edit_file (unique str-replace) → v2.
    let edited = edit(&server, &user, conv, &file_id, "alpha", "beta").await;
    assert!(edited["error"].is_null(), "edit_file: {edited}");
    assert_eq!(structured(&edited)["version"].as_i64(), Some(2));

    // /head reflects v2 and the new content.
    let (_, head) = get_json(&server, &user.token, &format!("/files/{file_id}/head")).await;
    assert_eq!(head["version"].as_i64(), Some(2));
    assert!(head["is_head"].as_bool().unwrap());
    let (_, text) = get_text(&server, &user.token, &format!("/files/{file_id}/text")).await;
    assert!(text.contains("beta") && !text.contains("alpha"), "head text: {text}");

    // /versions now lists 2, newest first.
    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    let arr = versions.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["version"].as_i64(), Some(2));
    assert_eq!(arr[1]["version"].as_i64(), Some(1));
    // Both authored by mcp tools (create_file, then edit_file).
    assert_eq!(arr[0]["created_by"].as_str(), Some("mcp"), "v2 (edit_file) authored by mcp");
    assert_eq!(arr[1]["created_by"].as_str(), Some("mcp"), "v1 (create_file) authored by mcp");
}

/// `create_file`'s v1 records the originating chat turn (`source_message_id`)
/// from the `x-message-id` header, mirroring the edit tools — so a created file
/// is as traceable to its turn as an edited one. Absent header ⇒ NULL, no crash.
#[tokio::test]
async fn create_file_stamps_source_message_id_from_header() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_provenance").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let message_id = Uuid::new_v4();

    // create_file WITH x-message-id → v1 carries that provenance.
    let res = reqwest::Client::new()
        .post(server.api_url("/files/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", conv.to_string())
        .header("x-message-id", message_id.to_string())
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "create_file",
                "arguments": { "filename": "traced.md", "content": "x\n" } },
        }))
        .send()
        .await
        .unwrap();
    let created: Value = res.json().await.unwrap();
    assert!(created["error"].is_null(), "create_file: {created}");
    let file_id = structured(&created)["file_id"].as_str().unwrap().to_string();

    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    let v1 = &versions.as_array().unwrap()[0];
    assert_eq!(
        v1["source_message_id"].as_str(),
        Some(message_id.to_string().as_str()),
        "v1 should record the originating message: {versions}"
    );

    // create_file WITHOUT x-message-id → NULL provenance, still succeeds.
    let untraced = call_tool(
        &server,
        &user,
        conv,
        "create_file",
        json!({ "filename": "untraced.md", "content": "y\n" }),
    )
    .await;
    assert!(untraced["error"].is_null(), "create_file (no header): {untraced}");
    let untraced_id = structured(&untraced)["file_id"].as_str().unwrap().to_string();
    let (_, uv) = get_json(&server, &user.token, &format!("/files/{untraced_id}/versions")).await;
    assert!(
        uv.as_array().unwrap()[0]["source_message_id"].is_null(),
        "missing header ⇒ NULL source_message_id: {uv}"
    );
}

// ── str-replace edge cases ───────────────────────────────────────────────────

#[tokio::test]
async fn edit_no_match_and_multiple_match_create_no_version() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_match").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "m.txt", "a a a\n").await;

    // 0 matches → error, no new version.
    let r = edit(&server, &user, conv, &file_id, "zzz", "q").await;
    assert!(!r["error"].is_null(), "expected NO_MATCH error: {r}");

    // >1 matches → error, no new version.
    let r = edit(&server, &user, conv, &file_id, "a", "b").await;
    assert!(!r["error"].is_null(), "expected MULTIPLE_MATCHES error: {r}");

    // Still exactly one version (v1) — neither failed edit appended.
    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(versions.as_array().unwrap().len(), 1, "no failed edit may append a version");
}

#[tokio::test]
async fn noop_edit_creates_no_version() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_noop").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "n.txt", "hello world\n").await;

    // old == new → no-op (unchanged), no version appended.
    let r = edit(&server, &user, conv, &file_id, "world", "world").await;
    assert!(r["error"].is_null(), "no-op should not error: {r}");
    assert_eq!(structured(&r)["unchanged"].as_bool(), Some(true));

    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(versions.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn rewrite_and_line_edit_append_versions() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_rw").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "r.txt", "line1\nline2\nline3\n").await;

    // line-range edit → v2.
    let r = call_tool(
        &server,
        &user,
        conv,
        "edit_file_lines",
        json!({ "id": file_id, "start_line": 2, "end_line": 2, "new_content": "LINE2" }),
    )
    .await;
    assert!(r["error"].is_null(), "edit_file_lines: {r}");
    assert_eq!(structured(&r)["version"].as_i64(), Some(2));

    // rewrite → v3.
    let r = call_tool(
        &server,
        &user,
        conv,
        "rewrite_file",
        json!({ "id": file_id, "content": "totally new\n" }),
    )
    .await;
    assert!(r["error"].is_null(), "rewrite_file: {r}");
    assert_eq!(structured(&r)["version"].as_i64(), Some(3));

    let (_, text) = get_text(&server, &user.token, &format!("/files/{file_id}/text")).await;
    assert!(text.contains("totally new"));
}

#[tokio::test]
async fn editing_user_upload_promotes_in_place() {
    // Editing a plain user upload appends v2 on the SAME file_id (promote in
    // place); v1 (the upload) stays restorable.
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_promote").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "doc.md", "original\n").await;

    let r = edit(&server, &user, conv, &file_id, "original", "edited").await;
    assert!(r["error"].is_null());
    // Same file_id, now v2.
    assert_eq!(structured(&r)["file_id"].as_str(), Some(file_id.as_str()));
    assert_eq!(structured(&r)["version"].as_i64(), Some(2));
    // v1 content is still retrievable.
    let (_, v1) = get_text(&server, &user.token, &format!("/files/{file_id}/versions/1/text")).await;
    assert!(v1.contains("original"));
    // Provenance: v1 authored by the user upload, v2 by the mcp edit tool.
    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    let arr = versions.as_array().unwrap();
    assert_eq!(arr[0]["created_by"].as_str(), Some("mcp"), "v2 (edit) authored by mcp");
    assert_eq!(arr[1]["created_by"].as_str(), Some("user"), "v1 (upload) authored by user");
}

// ── restore + reproducibility ────────────────────────────────────────────────

#[tokio::test]
async fn restore_is_append_only_and_reproducible() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_restore").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "story.md", "v-one\n").await;

    edit(&server, &user, conv, &file_id, "v-one", "v-two").await; // v2
    edit(&server, &user, conv, &file_id, "v-two", "v-three").await; // v3

    // Reproducibility: v1's text is still exactly the original after the head moved.
    let (_, v1text) = get_text(&server, &user.token, &format!("/files/{file_id}/versions/1/text")).await;
    assert!(v1text.contains("v-one") && !v1text.contains("v-three"), "v1 must be reproducible: {v1text}");
    let (_, headtext) = get_text(&server, &user.token, &format!("/files/{file_id}/text")).await;
    assert!(headtext.contains("v-three"));

    // Restore v1 → appends v4 (== v1 content), head advances; v2/v3 untouched.
    let resp = restore(&server, &user.token, &file_id, 1).await;
    assert_eq!(resp.status(), StatusCode::OK, "restore: {}", resp.text().await.unwrap_or_default());
    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(versions.as_array().unwrap().len(), 4, "restore appends, never deletes");
    // Provenance across all three HTTP/MCP authoring paths (newest-first): v4 the
    // user-initiated restore, v3/v2 the mcp edit tool, v1 the user upload.
    let arr = versions.as_array().unwrap();
    assert_eq!(arr[0]["created_by"].as_str(), Some("user"), "v4 (restore) authored by user");
    assert_eq!(arr[1]["created_by"].as_str(), Some("mcp"), "v3 (edit) authored by mcp");
    assert_eq!(arr[2]["created_by"].as_str(), Some("mcp"), "v2 (edit) authored by mcp");
    assert_eq!(arr[3]["created_by"].as_str(), Some("user"), "v1 (upload) authored by user");
    let (_, head) = get_json(&server, &user.token, &format!("/files/{file_id}/head")).await;
    assert_eq!(head["version"].as_i64(), Some(4));
    let (_, newhead) = get_text(&server, &user.token, &format!("/files/{file_id}/text")).await;
    assert!(newhead.contains("v-one"), "restored head should equal v1: {newhead}");

    // v3 still readable (append-only — restoring didn't delete the future).
    let (_, v3) = get_text(&server, &user.token, &format!("/files/{file_id}/versions/3/text")).await;
    assert!(v3.contains("v-three"));
}

#[tokio::test]
async fn restore_to_head_is_noop_and_bad_version_is_400() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_restore_edge").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "x.txt", "one\n").await;
    edit(&server, &user, conv, &file_id, "one", "two").await; // v2 (head)

    // Restore the current head (v2) → no-op, still 2 versions.
    let resp = restore(&server, &user.token, &file_id, 2).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(versions.as_array().unwrap().len(), 2, "restore-to-head is a no-op");

    // Restore a non-existent version → 400.
    let resp = restore(&server, &user.token, &file_id, 99).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── list returns head only ───────────────────────────────────────────────────

#[tokio::test]
async fn file_list_returns_one_row_per_file() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_list").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "list.txt", "a\n").await;
    edit(&server, &user, conv, &file_id, "a", "b").await;
    edit(&server, &user, conv, &file_id, "b", "c").await; // 3 versions

    let (s, body) = get_json(&server, &user.token, "/files?page=1&per_page=50").await;
    assert_eq!(s, StatusCode::OK);
    let rows: Vec<&Value> = body["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["id"].as_str() == Some(file_id.as_str()))
        .collect();
    assert_eq!(rows.len(), 1, "a 3-version file must appear exactly once in the list");
    assert_eq!(rows[0]["version"].as_i64(), Some(3), "list shows the head version");
}

// ── version-pinned download-with-token ───────────────────────────────────────

#[tokio::test]
async fn download_with_token_pins_version() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_token").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "t.txt", "first\n").await;
    edit(&server, &user, conv, &file_id, "first", "second").await; // v2 head

    // Token pinned to v1 → downloads v1 bytes even though head is v2.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/files/{file_id}/download-token?version=1")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("gen token");
    assert_eq!(resp.status(), StatusCode::OK);
    let token = resp.json::<Value>().await.unwrap()["token"].as_str().unwrap().to_string();
    let dl = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/download-with-token?token={token}")))
        .send()
        .await
        .expect("dl");
    assert_eq!(dl.status(), StatusCode::OK);
    let bytes = dl.text().await.unwrap();
    assert!(bytes.contains("first") && !bytes.contains("second"), "pinned token must serve v1: {bytes}");
}

// ── cross-user isolation + auth ──────────────────────────────────────────────

#[tokio::test]
async fn version_endpoints_are_owner_scoped() {
    let server = TestServer::start().await;
    let owner = power_user(&server, "fv_owner").await;
    let other = power_user(&server, "fv_other").await;
    let file_id = upload_text(&server, &owner, "secret.md", "owned\n").await;

    // The other user cannot list or restore the owner's file versions.
    let (s, _) = get_json(&server, &other.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "other user must not see versions");
    let resp = restore(&server, &other.token, &file_id, 1).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "other user must not restore");

    // Unauthenticated request is rejected.
    let unauth = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/versions")))
        .send()
        .await
        .unwrap();
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn standard_non_admin_user_can_use_write_tools() {
    // The write tools require `files::upload` (`require_write` in the dispatch).
    // This verifies the gate PERMITS a legitimate, NON-admin user: every
    // registered user joins the default `Users` group, which migration 27 grants
    // `files::read` + `files::upload`. So the gate must let them through on the
    // real default permission set — not only on admin / `*` wildcard.
    let server = TestServer::start().await;
    // Claim the root-admin slot (first user) with a throwaway, so the user under
    // test is genuinely non-admin and exercises the permission UNION, not the
    // is_admin short-circuit.
    let _admin = power_user(&server, "fv_admin_slot").await;
    let user = create_user_with_permissions(&server, "fv_standard", &["conversations::create"]).await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();

    let file_id = upload_text(&server, &user, "u.md", "alpha\n").await;
    let edited = edit(&server, &user, conv, &file_id, "alpha", "beta").await;
    assert!(edited["error"].is_null(), "non-admin user with default perms must be allowed: {edited}");
    assert_eq!(structured(&edited)["version"].as_i64(), Some(2));
}

#[tokio::test]
async fn deleting_a_versioned_file_removes_all_versions() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_delete").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "d.txt", "one\n").await;
    edit(&server, &user, conv, &file_id, "one", "two").await;
    // Restore v1 → a 3rd version that SHARES v1's blob (exercises the
    // dedupe-distinct-blobs path in delete).
    assert_eq!(restore(&server, &user.token, &file_id, 1).await.status(), StatusCode::OK);

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/files/{file_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The file (and all its versions) is gone.
    let (s, _) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s2, _) = get_json(&server, &user.token, &format!("/files/{file_id}")).await;
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn concurrent_edits_create_distinct_versions() {
    // Two non-overlapping edits issued CONCURRENTLY must each get a unique
    // version (the per-file row lock in append_version serializes them so they
    // can't collide on UNIQUE(file_id, version) and surface a 500).
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_concurrent").await;
    let conv = Uuid::parse_str(&create_conversation(&server, &user).await).unwrap();
    let file_id = upload_text(&server, &user, "c.txt", "AAA and BBB\n").await;

    let (r1, r2) = tokio::join!(
        edit(&server, &user, conv, &file_id, "AAA", "XXX"),
        edit(&server, &user, conv, &file_id, "BBB", "YYY"),
    );
    assert!(
        r1["error"].is_null() && r2["error"].is_null(),
        "both concurrent edits must succeed (no UNIQUE-collision 500): {r1} / {r2}"
    );
    let a = structured(&r1)["version"].as_i64().unwrap();
    let b = structured(&r2)["version"].as_i64().unwrap();
    assert_ne!(a, b, "concurrent appends must get distinct version numbers");

    // Exactly three versions: v1 (upload) + the two edits.
    let (_, versions) = get_json(&server, &user.token, &format!("/files/{file_id}/versions")).await;
    assert_eq!(versions.as_array().unwrap().len(), 3, "no version lost/duplicated under concurrency");
}

#[tokio::test]
async fn download_token_rejects_nonexistent_version() {
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_token_bad").await;
    let file_id = upload_text(&server, &user, "t2.txt", "x\n").await;
    // A token pinned to a version that doesn't exist is refused at mint time.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/files/{file_id}/download-token?version=99")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── ambiguous name resolution ────────────────────────────────────────────────

#[tokio::test]
async fn edit_by_ambiguous_name_is_rejected() {
    use super::{attach_file_to_project, create_project, attach_conversation_to_project};
    let server = TestServer::start().await;
    let user = power_user(&server, "fv_ambig").await;
    // Two files with the SAME name attached to one conversation's project.
    let project = create_project(&server, &user, "ambig-proj").await;
    let a = upload_text(&server, &user, "dup.txt", "AAA\n").await;
    let b = upload_text(&server, &user, "dup.txt", "BBB\n").await;
    attach_file_to_project(&server, &user, &project, &a).await;
    attach_file_to_project(&server, &user, &project, &b).await;
    let conv_s = create_conversation(&server, &user).await;
    attach_conversation_to_project(&server, &user, &project, &conv_s).await;
    let conv = Uuid::parse_str(&conv_s).unwrap();

    let r = call_tool(
        &server,
        &user,
        conv,
        "edit_file",
        json!({ "name": "dup.txt", "old_str": "AAA", "new_str": "ZZZ" }),
    )
    .await;
    assert!(!r["error"].is_null(), "ambiguous name must be rejected: {r}");
}
