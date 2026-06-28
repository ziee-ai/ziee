// ============================================================================
// Document RAG (file_rag) integration tests — Tier 2.
//
// Exercises the FULL production path over HTTP against a real Postgres test DB:
//   - eager background ingest after upload (chunk rows appear; FTS works)
//   - the `files_mcp` `semantic_search` tool: FTS-from-day-one (no embedder),
//     provenance shape, scope isolation, disabled-message
//   - re-index on a new head version, cascade cleanup on file delete
//
// The server runs as a subprocess (ingest happens there, async), so the tests
// poll the shared test DB via a raw pool for the background chunk rows. No
// embedding model is configured, so retrieval runs in FTS-only mode — which is
// exactly the zero-config day-one experience these tests pin down. Real-vector
// semantic ranking is a Tier-3 test (needs a real embedder).
// ============================================================================

use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use crate::common::test_helpers::{create_user_with_permissions, TestUser};
use crate::common::{TestServer, TestServerOptions};

// Tier-3 real-embedder tests (gated on GEMINI_API_KEY) — reuse the helpers
// below via `super::`.
mod real_test;

// ── helpers (mirrors tests/files_mcp/mod.rs) ────────────────────────────────

async fn power_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["*"]).await
}

async fn db_pool(server: &TestServer) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db")
}

async fn chunk_count(pool: &PgPool, file_id: &str) -> i64 {
    let fid = Uuid::parse_str(file_id).unwrap();
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
        .bind(fid)
        .fetch_one(pool)
        .await
        .expect("count chunks")
}

/// Poll until the file has at least `min` chunks (background ingest), or panic.
async fn wait_for_chunks(pool: &PgPool, file_id: &str, min: i64) -> i64 {
    for _ in 0..40 {
        let n = chunk_count(pool, file_id).await;
        if n >= min {
            return n;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for >= {min} chunks for file {file_id}");
}

/// Poll until a SQL predicate over this file's chunk text holds (used after a
/// re-index, where old chunks are deleted then new ones inserted).
async fn wait_for_chunk_text(pool: &PgPool, file_id: &str, needle: &str) {
    let fid = Uuid::parse_str(file_id).unwrap();
    for _ in 0..40 {
        let hit: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM file_chunks WHERE file_id = $1 AND content ILIKE $2 LIMIT 1",
        )
        .bind(fid)
        .bind(format!("%{needle}%"))
        .fetch_optional(pool)
        .await
        .expect("scan chunk text");
        if hit.is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for chunk text containing {needle:?} for file {file_id}");
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
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::CREATED,
        "upload: {}",
        resp.text().await.unwrap_or_default()
    );
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

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

async fn attach_file_to_project(server: &TestServer, user: &TestUser, project_id: &str, file_id: &str) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{project_id}/files")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "file_id": file_id }))
        .send()
        .await
        .expect("attach file");
    assert!(resp.status().is_success(), "attach file: {}", resp.text().await.unwrap_or_default());
}

async fn create_conversation(server: &TestServer, user: &TestUser) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .expect("create conv");
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED);
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

async fn attach_conversation_to_project(server: &TestServer, user: &TestUser, project_id: &str, conversation_id: &str) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{project_id}/conversations/{conversation_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("attach conv");
    assert!(resp.status().is_success(), "attach conv: {}", resp.text().await.unwrap_or_default());
}

/// Project conversation seeded with `(filename, body)` knowledge files. Returns
/// `(conversation_id, file_ids)`.
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

fn jsonrpc_call(
    server: &TestServer,
    token: &str,
    conversation_id: Option<Uuid>,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/files/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    if let Some(cid) = conversation_id {
        req = req.header("x-conversation-id", cid.to_string());
    }
    req
}

async fn call_tool(server: &TestServer, user: &TestUser, conversation_id: Uuid, name: &str, arguments: Value) -> Value {
    let res = jsonrpc_call(server, &user.token, Some(conversation_id), "tools/call", json!({ "name": name, "arguments": arguments }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "tools/call HTTP status");
    res.json().await.unwrap()
}

async fn semantic_search(server: &TestServer, user: &TestUser, conv: Uuid, query: &str) -> Value {
    call_tool(server, user, conv, "semantic_search", json!({ "query": query })).await
}

async fn set_rag_settings(server: &TestServer, user: &TestUser, body: Value) {
    let resp = reqwest::Client::new()
        .put(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&body)
        .send()
        .await
        .expect("put file-rag admin-settings");
    assert!(resp.status().is_success(), "put settings: {}", resp.text().await.unwrap_or_default());
}

async fn trigger_backfill(server: &TestServer, user: &TestUser) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/file-rag/backfill"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("post backfill");
    assert!(resp.status().is_success(), "backfill: {}", resp.text().await.unwrap_or_default());
}

async fn restore_version(server: &TestServer, user: &TestUser, file_id: &str, version: i32) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/files/{file_id}/restore")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "version": version }))
        .send()
        .await
        .expect("restore");
    assert!(resp.status().is_success(), "restore: {}", resp.text().await.unwrap_or_default());
}

async fn non_admin_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["files::read", "profile::read"]).await
}

async fn put_settings_raw(server: &TestServer, user: &TestUser, body: Value) -> reqwest::Response {
    reqwest::Client::new()
        .put(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&body)
        .send()
        .await
        .expect("put settings")
}

async fn post_raw(server: &TestServer, user: &TestUser, path: &str) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(path))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("post")
}

async fn get_settings(server: &TestServer, user: &TestUser) -> Value {
    reqwest::Client::new()
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get settings")
        .json()
        .await
        .unwrap()
}

/// Create a non-embedding (chat) model on the first seeded provider; returns
/// its id. The embed-dispatch capability check rejects it before any network
/// call, so no real provider key is needed.
async fn create_chat_model(server: &TestServer, user: &TestUser) -> String {
    let provs: Value = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_id = provs["providers"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|p| p["id"].as_str())
        .expect("at least one seeded provider")
        .to_string();
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "test-chat-model",
            "display_name": "Test Chat Model",
            "description": "non-embedder for validation",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true },
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::CREATED,
        "create chat model: {}",
        resp.text().await.unwrap_or_default()
    );
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

// ── tests ───────────────────────────────────────────────────────────────────

/// FTS-from-day-one: a fresh deployment (Document RAG ON by default, no
/// embedding model) indexes an uploaded file and answers `semantic_search` in
/// FTS mode with full provenance — zero admin configuration.
#[tokio::test]
async fn on_by_default_fts_search_with_provenance() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_default").await;
    let pool = db_pool(&server).await;

    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-default",
        &[(
            "biology.txt",
            "Photosynthesis occurs in the chloroplast. The thylakoid membrane hosts \
             the light-dependent reactions, while the Calvin cycle fixes carbon in the stroma.",
        )],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();

    // Background ingest produced chunks (FTS-ready) with NO embedder configured.
    wait_for_chunks(&pool, &file_ids[0], 1).await;

    let body = semantic_search(&server, &user, conv_uuid, "chloroplast thylakoid").await;
    assert!(body["error"].is_null(), "semantic_search should succeed; body={body}");
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["mode"].as_str().unwrap(), "fts", "no embedder → FTS-only mode");
    let results = sc["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "FTS should match; results={results:?}");
    let top = &results[0];
    assert_eq!(top["file_id"].as_str().unwrap(), file_ids[0].as_str());
    assert_eq!(top["name"].as_str().unwrap(), "biology.txt");
    assert_eq!(top["page"].as_i64().unwrap(), 1, "single-page text → page 1");
    assert!(top["char_start"].is_number() && top["char_end"].is_number(), "span provenance present");
    assert!(
        top["text"].as_str().unwrap().to_lowercase().contains("chloroplast"),
        "matched passage carries the term; top={top}"
    );
}

/// Scope isolation: a conversation only searches its own project's files; a
/// term that exists only in another project's file returns nothing.
#[tokio::test]
async fn scope_isolation_across_projects() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_scope").await;
    let pool = db_pool(&server).await;

    let (conv1, ids1) = project_conversation_with_files(
        &server,
        &user,
        "rag-p1",
        &[("a.txt", "alphazzz unique marker one for project one document body")],
    )
    .await;
    let (_conv2, ids2) = project_conversation_with_files(
        &server,
        &user,
        "rag-p2",
        &[("b.txt", "betazzz unique marker two for project two document body")],
    )
    .await;
    let conv1_uuid = Uuid::parse_str(&conv1).unwrap();
    wait_for_chunks(&pool, &ids1[0], 1).await;
    wait_for_chunks(&pool, &ids2[0], 1).await;

    // In conv1, the project-1 term is found...
    let found = semantic_search(&server, &user, conv1_uuid, "alphazzz").await;
    let r1 = found["result"]["structuredContent"]["results"].as_array().unwrap();
    assert!(!r1.is_empty(), "own-project term must be found");
    assert_eq!(r1[0]["file_id"].as_str().unwrap(), ids1[0].as_str());

    // ...but the project-2 term (chunks exist, but out of conv1's scope) is not.
    let other = semantic_search(&server, &user, conv1_uuid, "betazzz").await;
    let r2 = other["result"]["structuredContent"]["results"].as_array().unwrap();
    assert!(r2.is_empty(), "foreign-project term must NOT leak into conv1; got {r2:?}");
}

/// When an admin disables Document RAG, `semantic_search` returns a clear note
/// (and no results) rather than searching.
#[tokio::test]
async fn disabled_returns_graceful_note() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_disabled").await;
    let pool = db_pool(&server).await;

    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-disabled",
        &[("doc.txt", "searchable disabledmarker content here")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;

    set_rag_settings(&server, &user, json!({ "enabled": false })).await;

    let body = semantic_search(&server, &user, conv_uuid, "disabledmarker").await;
    assert!(body["error"].is_null(), "disabled is graceful, not an error; body={body}");
    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.to_lowercase().contains("disabled"), "note mentions disabled; text={text}");
    assert!(
        body["result"]["structuredContent"].is_null(),
        "no results structure when disabled"
    );
}

/// A new head version re-indexes: old chunks are replaced so search reflects
/// the latest content (exercises the commit_new_version → spawn_reindex hook).
#[tokio::test]
async fn reindex_on_new_version() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_reindex").await;
    let pool = db_pool(&server).await;

    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-reindex",
        &[("notes.md", "originalalpha content version one body text here")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;
    wait_for_chunk_text(&pool, &ids[0], "originalalpha").await;

    // Rewrite the file → new version → background re-index.
    let rewrite = call_tool(
        &server,
        &user,
        conv_uuid,
        "rewrite_file",
        json!({ "id": ids[0], "content": "revisedbeta content version two body text here" }),
    )
    .await;
    assert!(rewrite["error"].is_null(), "rewrite_file should succeed; body={rewrite}");

    // The new content is now indexed; the old term is gone.
    wait_for_chunk_text(&pool, &ids[0], "revisedbeta").await;
    let found = semantic_search(&server, &user, conv_uuid, "revisedbeta").await;
    let r = found["result"]["structuredContent"]["results"].as_array().unwrap();
    assert!(!r.is_empty(), "new version content must be searchable");

    let gone = semantic_search(&server, &user, conv_uuid, "originalalpha").await;
    let rg = gone["result"]["structuredContent"]["results"].as_array().unwrap();
    assert!(rg.is_empty(), "old version content must be gone after re-index; got {rg:?}");
}

/// Deleting a file cascades to its chunks (FK ON DELETE CASCADE).
#[tokio::test]
async fn delete_file_cascades_chunks() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_cascade").await;
    let pool = db_pool(&server).await;

    let file_id = upload_text(&server, &user, "ephemeral.txt", "cascademarker body content to index").await;
    wait_for_chunks(&pool, &file_id, 1).await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/files/{file_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete file");
    assert!(resp.status().is_success(), "delete: {}", resp.text().await.unwrap_or_default());

    assert_eq!(chunk_count(&pool, &file_id).await, 0, "chunks must cascade-delete with the file");
}

/// Backfill indexes files that pre-date enablement, and is idempotent.
#[tokio::test]
async fn backfill_indexes_preexisting_files() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_backfill").await;
    let pool = db_pool(&server).await;

    // Disable first, THEN upload — so the upload's ingest self-gates and the
    // file has text but no chunks (the "pre-existing file" condition).
    set_rag_settings(&server, &user, json!({ "enabled": false })).await;
    let file_id = upload_text(&server, &user, "old.txt", "backfillmarker content that should be indexed").await;
    // Give the (no-op) spawn a beat, then confirm no chunks while disabled.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(chunk_count(&pool, &file_id).await, 0, "no chunks while disabled");

    // Re-enable + run the backfill → the file gets indexed.
    set_rag_settings(&server, &user, json!({ "enabled": true })).await;
    trigger_backfill(&server, &user).await;
    let n = wait_for_chunks(&pool, &file_id, 1).await;

    // Re-running is idempotent: the file now has chunks, so it's skipped.
    trigger_backfill(&server, &user).await;
    tokio::time::sleep(Duration::from_millis(750)).await;
    assert_eq!(chunk_count(&pool, &file_id).await, n, "re-running backfill must be a no-op");
}

/// Restoring a prior version makes it the head and re-indexes to that content
/// (exercises the restore_version → spawn_reindex hook).
#[tokio::test]
async fn restore_version_reindexes() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_restore").await;
    let pool = db_pool(&server).await;

    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-restore",
        &[("doc.md", "alphaoneword original version one content body")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunk_text(&pool, &ids[0], "alphaoneword").await;

    // v2 via rewrite.
    let rewrite = call_tool(
        &server,
        &user,
        conv_uuid,
        "rewrite_file",
        json!({ "id": ids[0], "content": "betatwoword revised version two content body" }),
    )
    .await;
    assert!(rewrite["error"].is_null(), "rewrite: {rewrite}");
    wait_for_chunk_text(&pool, &ids[0], "betatwoword").await;

    // Restore v1 → head becomes v1 again → re-index back to v1 content.
    restore_version(&server, &user, &ids[0], 1).await;
    wait_for_chunk_text(&pool, &ids[0], "alphaoneword").await;

    let found = semantic_search(&server, &user, conv_uuid, "alphaoneword").await;
    assert!(!found["result"]["structuredContent"]["results"].as_array().unwrap().is_empty());
    let gone = semantic_search(&server, &user, conv_uuid, "betatwoword").await;
    assert!(
        gone["result"]["structuredContent"]["results"].as_array().unwrap().is_empty(),
        "the v2 content must be gone after restoring v1"
    );
}

/// Cross-user isolation: a second user cannot run semantic_search against the
/// owner's conversation (the conversation-ownership check rejects it).
#[tokio::test]
async fn cross_user_cannot_search_others_conversation() {
    let server = TestServer::start().await;
    let owner = power_user(&server, "file_rag_owner").await;
    let intruder = power_user(&server, "file_rag_intruder").await;
    let pool = db_pool(&server).await;

    let (conv, ids) = project_conversation_with_files(
        &server,
        &owner,
        "rag-owner",
        &[("private.txt", "ownersecretmarker confidential content body")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;

    // The intruder targets the owner's conversation → ownership check errors.
    let body = semantic_search(&server, &intruder, conv_uuid, "ownersecretmarker").await;
    let err = &body["error"];
    assert!(
        err.is_object(),
        "a foreign conversation must error, not search the owner's files; body={body}"
    );
    // 404 (foreign conversation; no existence leak) maps to JSON-RPC invalid_params.
    assert_eq!(
        err["code"].as_i64().unwrap(),
        -32602,
        "ownership rejection must be invalid_params, not a generic error; err={err}"
    );
}

/// A non-admin user is rejected from every admin endpoint (403).
#[tokio::test]
async fn non_admin_rejected_from_admin_endpoints() {
    let server = TestServer::start().await;
    let user = non_admin_user(&server, "file_rag_nonadmin").await;

    let put = put_settings_raw(&server, &user, json!({ "enabled": false })).await;
    assert_eq!(put.status(), reqwest::StatusCode::FORBIDDEN, "PUT settings must 403");
    let reembed = post_raw(&server, &user, "/file-rag/admin-settings/reembed").await;
    assert_eq!(reembed.status(), reqwest::StatusCode::FORBIDDEN, "reembed must 403");
    let backfill = post_raw(&server, &user, "/file-rag/backfill").await;
    assert_eq!(backfill.status(), reqwest::StatusCode::FORBIDDEN, "backfill must 403");
}

/// Setting a non-embedding model is rejected (400) and does not persist.
#[tokio::test]
async fn non_embedder_model_rejected() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_badmodel").await;
    let model_id = create_chat_model(&server, &user).await;

    let resp = put_settings_raw(&server, &user, json!({ "embedding_model_id": model_id })).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST, "non-embedder must be rejected");
    let body: Value = resp.json().await.unwrap();
    assert!(
        body.to_string().contains("INVALID_EMBEDDING_MODEL"),
        "error code INVALID_EMBEDDING_MODEL; body={body}"
    );
    let settings = get_settings(&server, &user).await;
    assert!(settings["embedding_model_id"].is_null(), "rejected model must not persist");
}

/// `/reembed` with no configured model returns a clear 400.
#[tokio::test]
async fn reembed_without_model_is_400() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_reembed_nomodel").await;
    let resp = post_raw(&server, &user, "/file-rag/admin-settings/reembed").await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert!(body.to_string().contains("NO_EMBEDDING_MODEL"), "code NO_EMBEDDING_MODEL; body={body}");
}

/// The `id` arg restricts the search to one file, excluding others in scope.
#[tokio::test]
async fn id_arg_restricts_search_to_one_file() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_idscope").await;
    let pool = db_pool(&server).await;
    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-idscope",
        &[
            ("a.txt", "alphascopeword unique content for file a body"),
            ("b.txt", "betascopeword unique content for file b body"),
        ],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;
    wait_for_chunks(&pool, &ids[1], 1).await;

    // Restricted to file A: A's term is found...
    let found = call_tool(
        &server,
        &user,
        conv_uuid,
        "semantic_search",
        json!({ "query": "alphascopeword", "id": ids[0] }),
    )
    .await;
    let r = found["result"]["structuredContent"]["results"].as_array().unwrap();
    assert!(
        !r.is_empty() && r.iter().all(|h| h["file_id"].as_str() == Some(ids[0].as_str())),
        "only file A's chunks; results={r:?}"
    );

    // ...but file B's term is excluded even though B is in the conversation.
    let other = call_tool(
        &server,
        &user,
        conv_uuid,
        "semantic_search",
        json!({ "query": "betascopeword", "id": ids[0] }),
    )
    .await;
    assert!(
        other["result"]["structuredContent"]["results"].as_array().unwrap().is_empty(),
        "file B's term must be excluded when scoped to file A"
    );
}

/// The per-call `top_k` is clamped to ≤50 and `truncated` is precise.
#[tokio::test]
async fn top_k_clamp_and_truncated() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_topk").await;
    let pool = db_pool(&server).await;
    // Small chunks → one doc yields many chunks, each containing "markerword".
    set_rag_settings(&server, &user, json!({ "chunk_chars": 200, "chunk_overlap_chars": 0 })).await;
    let body_text = "markerword ".repeat(300);
    let (conv, ids) =
        project_conversation_with_files(&server, &user, "rag-topk", &[("big.txt", &body_text)]).await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 3).await;

    // top_k=1 → exactly 1 result, truncated=true (more matched).
    let r1 = call_tool(
        &server,
        &user,
        conv_uuid,
        "semantic_search",
        json!({ "query": "markerword", "top_k": 1 }),
    )
    .await;
    let sc1 = &r1["result"]["structuredContent"];
    assert_eq!(sc1["results"].as_array().unwrap().len(), 1);
    assert!(sc1["truncated"].as_bool().unwrap(), "more matches existed → truncated");

    // top_k huge → clamped to ≤50; fewer than 50 chunks total → not truncated.
    let r2 = call_tool(
        &server,
        &user,
        conv_uuid,
        "semantic_search",
        json!({ "query": "markerword", "top_k": 9999 }),
    )
    .await;
    let sc2 = &r2["result"]["structuredContent"];
    let n = sc2["results"].as_array().unwrap().len();
    assert!(n > 1 && n <= 50, "clamped to <=50; got {n}");
    assert!(!sc2["truncated"].as_bool().unwrap(), "returned all matches → not truncated");
}

/// With FTS disabled and no embedder, search returns empty (Arm::None), no error.
#[tokio::test]
async fn fts_disabled_no_model_returns_empty() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_ftsoff").await;
    let pool = db_pool(&server).await;
    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-ftsoff",
        &[("doc.txt", "ftsoffmarker present in this document body")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;

    set_rag_settings(&server, &user, json!({ "fts_enabled": false })).await;
    let body = semantic_search(&server, &user, conv_uuid, "ftsoffmarker").await;
    assert!(body["error"].is_null(), "no error; body={body}");
    assert!(
        body["result"]["structuredContent"]["results"].as_array().unwrap().is_empty(),
        "fts disabled + no model → empty"
    );
}

/// The `max_chunks_per_file` cap bounds the number of chunks indexed.
#[tokio::test]
async fn max_chunks_per_file_cap_enforced() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_cap").await;
    let pool = db_pool(&server).await;
    set_rag_settings(
        &server,
        &user,
        json!({ "chunk_chars": 200, "chunk_overlap_chars": 0, "max_chunks_per_file": 3 }),
    )
    .await;
    let body_text = "capword ".repeat(400); // many 200-char chunks, capped at 3
    let file_id = upload_text(&server, &user, "capped.txt", &body_text).await;
    wait_for_chunks(&pool, &file_id, 3).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(chunk_count(&pool, &file_id).await, 3, "max_chunks_per_file cap must hold");
}

/// A blank query is rejected up front rather than scanning everything.
#[tokio::test]
async fn blank_query_errors() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_blank").await;
    let (conv, _ids) =
        project_conversation_with_files(&server, &user, "rag-blank", &[("doc.txt", "some content")]).await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    let body = call_tool(&server, &user, conv_uuid, "semantic_search", json!({ "query": "   " })).await;
    assert!(body["error"].is_object(), "blank query must error; body={body}");
}

/// Backfill handles files that have text pages but yield zero chunks
/// (whitespace-only) and still indexes a real file ordered after one. (The
/// exact ≥BACKFILL_BATCH starvation needs hundreds of files to reproduce; this
/// covers the zero-chunk path and that the scan reaches a later real file.)
#[tokio::test]
async fn backfill_handles_zero_chunk_files() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_backfill_zero").await;
    let pool = db_pool(&server).await;

    set_rag_settings(&server, &user, json!({ "enabled": false })).await;
    // Whitespace-only: has a text page (text_page_count > 0) but produces no chunks.
    let blank_id = upload_text(&server, &user, "blank.txt", "   \n\n  \t \n   ").await;
    // A genuinely indexable file uploaded AFTER the blank one.
    let real_id =
        upload_text(&server, &user, "real.txt", "realbackfillword content to index here body").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(chunk_count(&pool, &blank_id).await, 0, "no chunks while disabled");
    assert_eq!(chunk_count(&pool, &real_id).await, 0, "no chunks while disabled");

    set_rag_settings(&server, &user, json!({ "enabled": true })).await;
    trigger_backfill(&server, &user).await;

    // The real file is indexed despite the zero-chunk file in the work-list...
    wait_for_chunks(&pool, &real_id, 1).await;
    // ...and the whitespace file yields no chunks (and doesn't wedge the loop).
    assert_eq!(chunk_count(&pool, &blank_id).await, 0, "whitespace file yields no chunks");
}

/// Regression: a *full batch* of zero-chunk files must NOT starve a later
/// indexable file. The boot/manual backfill scans `files_with_text_missing_chunks`
/// in batches ordered by `created_at`; before the `exclude` fix it re-fetched
/// the same oldest batch every iteration, so ≥`BACKFILL_BATCH` whitespace files
/// (text pages but zero chunks — they match the predicate forever) walled off
/// every newer file behind them. This test shrinks the batch to 2 via the
/// debug-only `FILE_RAG_BACKFILL_BATCH` seam and seeds 3 zero-chunk files ahead
/// of one real file: the first batch is all whitespace, so an offset-less scan
/// would loop on it and never reach `real`. It FAILS against the pre-fix code
/// and passes only because the `exclude` set slides the window forward.
#[tokio::test]
async fn backfill_does_not_starve_behind_zero_chunk_wall() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("FILE_RAG_BACKFILL_BATCH".to_string(), "2".to_string())],
        ..Default::default()
    })
    .await;
    let user = power_user(&server, "file_rag_backfill_wall").await;
    let pool = db_pool(&server).await;

    // Disable so eager ingest no-ops — the files exist with text but no chunks,
    // exactly the pre-existing-corpus state the backfill must heal.
    set_rag_settings(&server, &user, json!({ "enabled": false })).await;

    // Three whitespace-only files first (each has a text page, yields zero
    // chunks → matches the work-list predicate forever). With batch=2 they more
    // than fill the first scan window. A short gap guarantees a strict
    // `created_at` ordering so the wall sorts ahead of the real file.
    let mut blanks = Vec::new();
    for i in 0..3 {
        let id = upload_text(&server, &user, &format!("wall{i}.txt"), "   \n\n \t \n  ").await;
        blanks.push(id);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // The genuinely indexable file, created strictly AFTER the whitespace wall.
    let real_id =
        upload_text(&server, &user, "behind-wall.txt", "wallbackfillword content to index here").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    for id in &blanks {
        assert_eq!(chunk_count(&pool, id).await, 0, "no chunks while disabled");
    }
    assert_eq!(chunk_count(&pool, &real_id).await, 0, "no chunks while disabled");

    set_rag_settings(&server, &user, json!({ "enabled": true })).await;
    trigger_backfill(&server, &user).await;

    // The window must slide past the full first batch of whitespace files and
    // reach `real`. (Pre-fix: the scan re-fetched the same oldest batch and
    // stalled — `real` would stay at zero chunks here.)
    wait_for_chunks(&pool, &real_id, 1).await;
    for id in &blanks {
        assert_eq!(chunk_count(&pool, id).await, 0, "whitespace files still yield no chunks");
    }
}

/// Unreadable-page tolerance during ingest (gap 7961f6432262, ingest.rs:132-141):
/// a page that fails to load is warned + skipped, and indexing continues over
/// the readable pages instead of erroring. We inflate an older version's claimed
/// text_page_count to 3 (only page 1 exists on disk for that blob), restore to
/// it (→ spawn_reindex reads the inflated count), and assert the reindex still
/// produces chunks from the readable page 1 — the missing pages 2/3 are tolerated.
#[tokio::test]
async fn reindex_tolerates_unreadable_pages() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_tol").await;
    let pool = db_pool(&server).await;

    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-tolerance",
        &[("tol.txt", "alphaunique beta gamma delta body content one two three")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;

    // Make a v2 so v1 becomes a restorable older version.
    let rewrite = call_tool(
        &server,
        &user,
        conv_uuid,
        "rewrite_file",
        json!({ "id": ids[0], "content": "secondversion entirely different body text here" }),
    )
    .await;
    assert!(rewrite["error"].is_null(), "rewrite_file should succeed; {rewrite}");

    // Inflate v1's claimed page count: it still has only page 1 on disk, so a
    // reindex of v1 will try (and fail to load) pages 2 and 3.
    let fid = Uuid::parse_str(&ids[0]).unwrap();
    sqlx::query("UPDATE file_versions SET text_page_count = 3 WHERE file_id = $1 AND version = 1")
        .bind(fid)
        .execute(&pool)
        .await
        .expect("inflate v1 page count");

    // Restore to v1 → head=v1 (text_page_count=3) → spawn_reindex over 3 pages,
    // only page 1 readable.
    restore_version(&server, &user, &ids[0], 1).await;

    // The reindex tolerated the 2 unreadable pages and still indexed page 1.
    wait_for_chunk_text(&pool, &ids[0], "alphaunique").await;
    assert!(chunk_count(&pool, &ids[0]).await >= 1, "page-1 chunks survive the skipped pages");
    let found = semantic_search(&server, &user, conv_uuid, "alphaunique").await;
    let r = found["result"]["structuredContent"]["results"].as_array().unwrap();
    assert!(!r.is_empty(), "v1 page-1 content searchable after tolerant reindex");
}

// audit id all-5b0035643293 — the file_rag admin-settings handler's VALIDATION
// branches (update_admin_settings: chunk_chars range, overlap >= 0,
// max_chunks_per_file > 0, and the cross-field overlap < chunk_chars) had no
// rejection test — existing tests only set VALID values. Drive each invalid
// case through the real PUT handler (reusing put_settings_raw above) and assert
// 400 VALIDATION_ERROR.
#[tokio::test]
async fn admin_settings_validation_rejects_bad_values() {
    let server = TestServer::start().await;
    let user = power_user(&server, "frag_validation").await;

    // chunk_chars out of range (200..=8000).
    let resp = put_settings_raw(&server, &user, json!({ "chunk_chars": 100 })).await;
    assert_eq!(resp.status(), 400, "chunk_chars below range must be 400");
    assert_eq!(
        resp.json::<Value>().await.unwrap_or_default()["error_code"],
        "VALIDATION_ERROR"
    );

    let resp = put_settings_raw(&server, &user, json!({ "chunk_chars": 9000 })).await;
    assert_eq!(resp.status(), 400, "chunk_chars above range must be 400");

    // max_chunks_per_file must be > 0.
    let resp = put_settings_raw(&server, &user, json!({ "max_chunks_per_file": 0 })).await;
    assert_eq!(resp.status(), 400, "max_chunks_per_file 0 must be 400");

    // Cross-field: overlap must be < chunk_chars.
    let resp = put_settings_raw(
        &server,
        &user,
        json!({ "chunk_chars": 1000, "chunk_overlap_chars": 1000 }),
    )
    .await;
    assert_eq!(resp.status(), 400, "overlap >= chunk_chars must be 400");
    assert_eq!(
        resp.json::<Value>().await.unwrap_or_default()["error_code"],
        "VALIDATION_ERROR"
    );
}
