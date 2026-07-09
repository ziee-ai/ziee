use serde_json::Value;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::test_helpers::TestUser;
use crate::common::TestServer;
use crate::common::TestServerOptions;

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

/// Dimension-mismatch after a model swap (the guards at
/// `embed_worker.rs:137-145` and `ingest.rs:229-236`, which skip a chunk whose
/// freshly-embedded vector dimension != the column dimension).
///
/// Those guards exist because `file_chunks.embedding` is a fixed-width
/// `halfvec(768)` column: if a swapped-in model emits a vector of a different
/// dimension, writing it would be rejected by Postgres and would corrupt the
/// HNSW index. The guards therefore SKIP such chunks (leaving them NULL =
/// FTS-only) instead of writing them at the wrong width.
///
/// This pins that contract end-to-end against the REAL schema + the REAL
/// production write (`set_chunk_embedding`'s exact untyped
/// `UPDATE file_chunks SET embedding = $1` bind): a real ingest-produced chunk
/// rejects a wrong-dimension `halfvec` write but accepts a matching 768-dim one.
/// So a model returning a mismatched dimension can NEVER be silently stored —
/// it can only be skipped, which is precisely the guarded behavior.
#[tokio::test]
async fn embedding_dimension_mismatch_on_model_swap_is_rejected_not_silently_stored() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_dimswap").await;
    let pool = db_pool(&server).await;

    // Real ingest path produces a chunk (embedding NULL, FTS-only — no embedder).
    let file_id = upload_text(
        &server,
        &user,
        "swap.txt",
        "Mitochondria are the powerhouse of the cell; the electron transport chain \
         pumps protons across the inner membrane.",
    )
    .await;
    wait_for_chunks(&pool, &file_id, 1).await;

    let fid = Uuid::parse_str(&file_id).unwrap();
    let (chunk_id, chunk_uid): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, user_id FROM file_chunks WHERE file_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(fid)
    .fetch_one(&pool)
    .await
    .expect("fetch a chunk");

    // The production write verbatim (repository::set_chunk_embedding):
    //   UPDATE file_chunks SET embedding = $1, embedding_model = $2 WHERE id = $3 AND user_id = $4
    let write_embedding = |dims: usize, model: &'static str| {
        let pool = pool.clone();
        let hv = pgvector::HalfVector::from_f32_slice(&vec![0.0123_f32; dims]);
        async move {
            sqlx::query(
                "UPDATE file_chunks SET embedding = $1, embedding_model = $2 \
                 WHERE id = $3 AND user_id = $4",
            )
            .bind(hv)
            .bind(model)
            .bind(chunk_id)
            .bind(chunk_uid)
            .execute(&pool)
            .await
        }
    };

    // A swapped model emitting the WRONG dimension (4 != 768) must be rejected
    // by the column — this is the failure the in-loop guards short-circuit.
    let mismatch = write_embedding(4, "swapped-model-4dim").await;
    assert!(
        mismatch.is_err(),
        "writing a 4-dim vector into halfvec(768) must be rejected by Postgres, \
         not silently stored at the wrong width",
    );

    // The stale chunk is still untouched: embedding NULL, model NULL (the guard
    // left it FTS-only rather than corrupting it).
    let (emb_is_null, model_is_null): (bool, bool) = sqlx::query_as(
        "SELECT embedding IS NULL, embedding_model IS NULL FROM file_chunks WHERE id = $1",
    )
    .bind(chunk_id)
    .fetch_one(&pool)
    .await
    .expect("re-read chunk after rejected write");
    assert!(
        emb_is_null && model_is_null,
        "a rejected wrong-dim write must leave the chunk NULL (skipped/FTS-only), \
         got embedding_null={emb_is_null} model_null={model_is_null}",
    );

    // A model emitting the MATCHING 768 dimension writes cleanly and is tagged —
    // the post-swap re-embed of a same-dimension model fills the corpus.
    let ok = write_embedding(768, "swapped-model-768dim").await;
    assert!(ok.is_ok(), "a 768-dim vector must store into halfvec(768): {ok:?}");

    let (has_embedding, model_tag): (bool, Option<String>) = sqlx::query_as(
        "SELECT embedding IS NOT NULL, embedding_model FROM file_chunks WHERE id = $1",
    )
    .bind(chunk_id)
    .fetch_one(&pool)
    .await
    .expect("re-read chunk after matching write");
    assert!(has_embedding, "matching-dim embedding should now be present");
    assert_eq!(
        model_tag.as_deref(),
        Some("swapped-model-768dim"),
        "the chunk should be tagged with the swapped-in model",
    );
}

/// Permission revocation is re-resolved per request, not cached on the token.
///
/// A user granted `file_rag::admin::read` can GET the admin settings (200).
/// When that grant is removed from their group mid-flow, the *same* bearer
/// token's next GET must be refused with 403 — proving the read gate
/// (`RequirePermissions<(FileRagAdminRead,)>`) resolves group permissions live
/// on every request rather than trusting a stale snapshot from issue time.
/// (Existing `non_admin_rejected_from_admin_endpoints` only covers a user who
/// never had the permission.)
#[tokio::test]
async fn admin_read_permission_revocation_is_enforced_per_request() {
    let server = TestServer::start().await;
    // Granted exactly the admin-read permission (plus profile::read so the
    // account is otherwise normal). create_user_with_permissions puts these on
    // a dedicated, non-default group we can later empty out.
    let user = create_user_with_permissions(
        &server,
        "file_rag_revoke",
        &["file_rag::admin::read", "profile::read"],
    )
    .await;

    // Before revocation: the read gate admits the request.
    let before = reqwest::Client::new()
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get settings (granted)");
    assert_eq!(
        before.status(),
        reqwest::StatusCode::OK,
        "user holding file_rag::admin::read must read admin settings"
    );

    // Revoke mid-flow: strip every permission from the user's custom group.
    // (The default group never carried file_rag::admin::read, so this fully
    // removes the grant.)
    let pool = db_pool(&server).await;
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let affected = sqlx::query(
        "UPDATE groups SET permissions = '{}', updated_at = NOW() \
         WHERE is_default = false AND id IN \
           (SELECT group_id FROM user_groups WHERE user_id = $1)",
    )
    .bind(user_uuid)
    .execute(&pool)
    .await
    .expect("revoke custom-group permissions")
    .rows_affected();
    assert!(affected >= 1, "expected to clear at least the custom permissions group");

    // After revocation: the SAME token is now refused — perms re-resolved live.
    let after = reqwest::Client::new()
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get settings (revoked)");
    assert_eq!(
        after.status(),
        reqwest::StatusCode::FORBIDDEN,
        "after the grant is removed, the same token must be re-checked and refused (403), not served from a cached allow"
    );
}

/// Concurrent re-index of the SAME file serializes on the per-file advisory
/// xact lock (`pg_advisory_xact_lock(hashtext('file_rag_reindex:' || file_id))`
/// in `FileRagRepository::reindex_chunks`), so two simultaneous rewrites can
/// never interleave their DELETE-then-INSERT into mixed or duplicate chunks.
///
/// This drives the REAL production path (rewrite_file → commit_new_version →
/// spawn_reindex → reindex_chunks) twice CONCURRENTLY against one file, then —
/// once the dust settles — asserts the invariants that can only hold if the
/// lock truly serialized the two transactions:
///   - no duplicate `chunk_index` (a half-interleaved insert would dupe),
///   - all surviving chunks share ONE `blob_version_id` (no two-version blend),
///   - exactly one rewrite's marker is present (not both, not neither),
///   - the original content is fully gone.
/// A broken/missing lock would let the two DELETE/INSERTs interleave and leave
/// duplicate indices or a mix of both markers. Nothing is mocked.
#[tokio::test]
async fn concurrent_rewrite_same_file_serializes_via_advisory_lock() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_advlock").await;
    let pool = db_pool(&server).await;

    // Original content carries a unique marker; long enough to be a real body.
    let original = format!("origmarker {}", "lorem ipsum dolor sit amet ".repeat(40));
    let (conv, ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-advlock",
        &[("notes.md", original.as_str())],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    let fid = Uuid::parse_str(&ids[0]).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;
    wait_for_chunk_text(&pool, &ids[0], "origmarker").await;

    // Two DISTINCT rewrites of the SAME file, fired CONCURRENTLY → two
    // spawn_reindex tasks that race on the per-file advisory lock.
    let alpha = format!("alphamarker {}", "alpha body content words ".repeat(60));
    let beta = format!("betamarker {}", "beta body content words ".repeat(60));
    let (ra, rb) = tokio::join!(
        call_tool(&server, &user, conv_uuid, "rewrite_file", json!({ "id": ids[0], "content": alpha })),
        call_tool(&server, &user, conv_uuid, "rewrite_file", json!({ "id": ids[0], "content": beta })),
    );
    assert!(ra["error"].is_null(), "rewrite A should succeed; body={ra}");
    assert!(rb["error"].is_null(), "rewrite B should succeed; body={rb}");

    // Wait for re-index to settle: the original marker is gone AND the chunk
    // count is stable across two consecutive reads (no in-flight reindex).
    let mut last = -1i64;
    let mut settled = false;
    for _ in 0..80 {
        let orig_present: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM file_chunks WHERE file_id = $1 AND content ILIKE '%origmarker%' LIMIT 1",
        )
        .bind(fid)
        .fetch_optional(&pool)
        .await
        .expect("scan orig");
        let n = chunk_count(&pool, &ids[0]).await;
        if orig_present.is_none() && n > 0 && n == last {
            settled = true;
            break;
        }
        last = n;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(settled, "re-index did not settle to a stable, original-free state");

    // Invariants that hold IFF the advisory lock serialized the two reindexes.
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
        .bind(fid)
        .fetch_one(&pool)
        .await
        .expect("count");
    let distinct_idx: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT chunk_index) FROM file_chunks WHERE file_id = $1")
            .bind(fid)
            .fetch_one(&pool)
            .await
            .expect("distinct idx");
    let distinct_ver: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT blob_version_id) FROM file_chunks WHERE file_id = $1",
    )
    .bind(fid)
    .fetch_one(&pool)
    .await
    .expect("distinct ver");
    let has_alpha: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM file_chunks WHERE file_id = $1 AND content ILIKE '%alphamarker%' LIMIT 1",
    )
    .bind(fid)
    .fetch_optional(&pool)
    .await
    .expect("scan alpha");
    let has_beta: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM file_chunks WHERE file_id = $1 AND content ILIKE '%betamarker%' LIMIT 1",
    )
    .bind(fid)
    .fetch_optional(&pool)
    .await
    .expect("scan beta");

    assert!(total > 0, "file must have chunks after the concurrent rewrites");
    assert_eq!(
        distinct_idx, total,
        "duplicate chunk_index rows => the two reindex transactions interleaved (lock failed)"
    );
    assert_eq!(
        distinct_ver, 1,
        "chunks span >1 blob_version_id => a mixed/blended index (lock failed); got {distinct_ver}"
    );
    assert_ne!(
        has_alpha.is_some(),
        has_beta.is_some(),
        "exactly one rewrite must win; both markers present => corrupt interleave, neither => lost write"
    );
}

// ── embed-dispatch failure recovery (cross-module: file_rag ↔ memory) ────────
//
// file_rag delegates embedding to `memory::engine::dispatch::embed_batch`
// (ingest.rs `embed_file_chunks`) and to `dispatch::embed` (retrieval-time
// query embed + the admin-settings probe). These two tests pin the documented
// recovery contract when that cross-module dispatch is unavailable / fails:
//   1. with NO embedder, ingest stores chunks with a NULL embedding yet they
//      stay fully FTS-searchable (degrade, don't error); and
//   2. configuring an embedding model whose provider dispatch FAILS is rejected
//      gracefully (4xx, not a 5xx/panic) — embed_batch's error is handled.

/// Cross-module recovery, ingest+search: a fresh deployment has no embedding
/// model, so the `file_rag → memory::embed_batch` dispatch never populates
/// `file_chunks.embedding` (it stays NULL) — yet the chunks are stored and
/// `semantic_search` still answers via the FTS fallback. Asserts the DB-level
/// degradation invariant (embedding IS NULL) that the existing FTS tests omit,
/// tying the ingest-side dispatch outcome to the search-side FTS result.
#[tokio::test]
async fn embed_dispatch_absent_keeps_chunks_null_embedded_yet_fts_searchable() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_embedfail").await;
    let pool = db_pool(&server).await;

    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-embedfail",
        &[(
            "mito.txt",
            "The mitochondrion is the powerhouse of the cell; oxidative phosphorylation \
             on the cristae membrane drives ATP synthase to produce uniquemarkerzeta.",
        )],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    let file_uuid = Uuid::parse_str(&file_ids[0]).unwrap();

    // Background ingest produced chunks.
    let total = wait_for_chunks(&pool, &file_ids[0], 1).await;
    assert!(total >= 1, "ingest must produce at least one chunk");

    // The cross-module embed dispatch was skipped (no model) → EVERY chunk has a
    // NULL embedding. This is the ingest-side degradation: chunks are stored
    // FTS-ready, never blocked on the unavailable embedder.
    let embedded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM file_chunks WHERE file_id = $1 AND embedding IS NOT NULL",
    )
    .bind(file_uuid)
    .fetch_one(&pool)
    .await
    .expect("count embedded chunks");
    assert_eq!(
        embedded, 0,
        "with no embedder configured the file_rag→memory embed dispatch must leave \
         embedding NULL (degrade to FTS), not block or partially embed; got {embedded}"
    );

    // The search-side fallback: despite NULL embeddings, the term is found and
    // the retrieval mode is FTS (retrieval.rs degrade path), with provenance.
    let body = semantic_search(&server, &user, conv_uuid, "uniquemarkerzeta").await;
    assert!(body["error"].is_null(), "semantic_search must not error; body={body}");
    let sc = &body["result"]["structuredContent"];
    assert_eq!(
        sc["mode"].as_str().unwrap(),
        "fts",
        "no usable embedder → retrieval falls back to FTS mode"
    );
    let results = sc["results"].as_array().expect("results array");
    assert!(
        !results.is_empty(),
        "FTS fallback must still return the matching chunk; results={results:?}"
    );
    assert_eq!(results[0]["file_id"].as_str().unwrap(), file_ids[0].as_str());
}

/// Cross-module recovery, configure path: setting an embedding model whose
/// provider dispatch FAILS must be handled gracefully. The admin-settings
/// handler probe-embeds via `memory::dispatch::embed` → `embed_batch` →
/// `embed_remote`; pointing the provider at a closed loopback port makes that
/// dispatch return `Err` (connection refused). The endpoint must answer a clean
/// `400 INVALID_EMBEDDING_MODEL`, NOT a 500 / panic — and Document RAG keeps
/// working in FTS mode (the embedder was never accepted).
#[tokio::test]
async fn configured_embedder_with_dead_provider_is_rejected_not_5xx() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_deadembed").await;
    let http = reqwest::Client::new();

    // A provider whose base_url is a closed loopback port → embed dispatch gets
    // an instant connection-refused (deterministic, no real network egress).
    let prov: Value = http
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Dead Embed Provider",
            "provider_type": "openai",
            "enabled": true,
            "api_key": "sk-test-unused",
            "base_url": "http://127.0.0.1:1/v1",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_id = prov["id"].as_str().expect("created provider id").to_string();

    // A model FLAGGED embedding-capable (passes the capability check in
    // embed_batch) but whose provider can't be reached.
    let model: Value = http
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "dead-embed-model",
            "display_name": "Dead Embed Model",
            "description": "embedding-capable but provider is unreachable",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "text_embedding": true },
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let model_id = model["id"].as_str().expect("created model id").to_string();

    // Selecting it as the embedding model triggers the server-side probe embed,
    // which fails at the dead provider. The handler must convert embed_batch's
    // Err into a clean 400 (provider error logged, not leaked), not a 5xx.
    let resp = http
        .put(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "embedding_model_id": model_id,
            "semantic_enabled": true,
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status,
        reqwest::StatusCode::BAD_REQUEST,
        "a configured-but-unreachable embedder must be rejected with 400, not a 5xx/panic; \
         got {status}: {body}"
    );
    assert!(
        body.contains("INVALID_EMBEDDING_MODEL"),
        "rejection should carry the INVALID_EMBEDDING_MODEL code; body={body}"
    );

    // And the deployment is unharmed: with no embedder accepted, semantic_search
    // still works in FTS mode on a freshly uploaded file.
    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-deadembed",
        &[("note.txt", "a recoverymarkeromega term that FTS can still locate")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    let pool = db_pool(&server).await;
    wait_for_chunks(&pool, &file_ids[0], 1).await;
    let search = semantic_search(&server, &user, conv_uuid, "recoverymarkeromega").await;
    assert!(search["error"].is_null(), "search must succeed post-rejection; body={search}");
    assert_eq!(
        search["result"]["structuredContent"]["mode"].as_str().unwrap(),
        "fts",
        "rejected embedder leaves retrieval in FTS mode"
    );
    assert!(
        !search["result"]["structuredContent"]["results"]
            .as_array()
            .unwrap()
            .is_empty(),
        "FTS must still find the term after the embedder was rejected"
    );
}

/// Mid-session permission ESCALATION takes effect on the next request — the
/// grant complement of `admin_read_permission_revocation_is_enforced_per_request`.
///
/// `RequirePermissions` re-resolves the caller's group permissions live on every
/// request (`extractors.rs:130-151` → `Repos.user.get_user_groups`), so a
/// permission added to the user's group AFTER their token was minted must be
/// honored without a re-login. A regression that cached the initial (denied)
/// permission set would keep returning 403 and fail this test.
#[tokio::test]
async fn admin_read_permission_grant_takes_effect_mid_session() {
    let server = TestServer::start().await;
    // Start WITHOUT file_rag::admin::read — only a normal profile permission.
    // create_user_with_permissions seeds these onto a dedicated, non-default
    // group we can later widen.
    let user = create_user_with_permissions(
        &server,
        "file_rag_grant",
        &["profile::read"],
    )
    .await;

    // Before the grant: the admin-read gate refuses (the user genuinely lacks it).
    let before = reqwest::Client::new()
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get settings (pre-grant)");
    assert_eq!(
        before.status(),
        reqwest::StatusCode::FORBIDDEN,
        "a user lacking file_rag::admin::read must be refused before the grant"
    );

    // Grant mid-flow: add file_rag::admin::read to the user's custom group.
    // No re-login, same token — the next request must re-resolve and admit it.
    let pool = db_pool(&server).await;
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let affected = sqlx::query(
        "UPDATE groups \
         SET permissions = ARRAY['profile::read','file_rag::admin::read']::text[], \
             updated_at = NOW() \
         WHERE is_default = false AND id IN \
           (SELECT group_id FROM user_groups WHERE user_id = $1)",
    )
    .bind(user_uuid)
    .execute(&pool)
    .await
    .expect("grant file_rag::admin::read to custom group")
    .rows_affected();
    assert!(affected >= 1, "expected to widen at least the custom permissions group");

    // After the grant: the SAME token now succeeds — perms re-resolved live,
    // not served from a cached deny.
    let after = reqwest::Client::new()
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get settings (post-grant)");
    assert_eq!(
        after.status(),
        reqwest::StatusCode::OK,
        "after the grant is added, the same token must be re-checked and admitted (200), proving mid-session escalation takes effect"
    );
}

/// Concurrent re-ingest of the SAME file must not corrupt the chunk set: the
/// reindex transaction takes a per-file `pg_advisory_xact_lock(file_rag_reindex:$1)`,
/// so two simultaneous re-index passes serialize and each leaves a consistent
/// DELETE+INSERT swap (no doubled rows, no lost index). Drives the real
/// `reindex_file` entrypoint twice concurrently.
#[tokio::test]
async fn concurrent_reindex_of_same_file_keeps_chunks_consistent() {
    let server = TestServer::start().await;
    let user = power_user(&server, "rag_concurrent_ingest").await;
    let pool = db_pool(&server).await;

    set_rag_settings(&server, &user, json!({ "enabled": true })).await;
    let file_id = upload_text(
        &server,
        &user,
        "concurrent.txt",
        "alpha beta gamma delta epsilon\nsecond line of content here\nthird line too\n",
    )
    .await;
    let n = wait_for_chunks(&pool, &file_id, 1).await;

    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let file_uuid = Uuid::parse_str(&file_id).unwrap();

    // Two concurrent re-index passes on the same file.
    let (r1, r2) = tokio::join!(
        ziee::file_rag_ingest::reindex_file(user_uuid, file_uuid),
        ziee::file_rag_ingest::reindex_file(user_uuid, file_uuid),
    );
    r1.expect("first concurrent reindex must succeed");
    r2.expect("second concurrent reindex must succeed");

    // The advisory lock serialized the two swaps → the chunk count is the same
    // single index, not doubled or wiped.
    assert_eq!(
        chunk_count(&pool, &file_id).await,
        n,
        "concurrent reindex must leave exactly one consistent chunk set"
    );
}

/// Concurrent search during an embed rebuild (the half-dimensions race): while a
/// re-embed is in flight the `file_chunks.embedding` column is NULL for affected
/// rows. A search MUST stay safe — the vector arm's `embedding IS NOT NULL`
/// filter excludes those rows (so a stale/wrong-dimension query vector can't hit
/// a half-migrated row and error), while FTS keeps serving from `content_tsv`.
/// With NO embedding model configured, freshly-indexed chunks have NULL
/// embeddings — exactly the mid-rebuild state — so this reproduces it directly.
#[tokio::test]
async fn search_during_embed_rebuild_vector_arm_excludes_null_embeddings_fts_still_serves() {
    let server = TestServer::start().await;
    let user = power_user(&server, "rag_embed_race").await;
    let pool = db_pool(&server).await;

    // FTS-only deployment (no embedding model) → chunks land with NULL embeddings.
    set_rag_settings(&server, &user, json!({ "enabled": true })).await;
    let file_id = upload_text(
        &server,
        &user,
        "race.txt",
        "ZEBRAFISH genomics quarterly report with distinctive searchable tokens\n",
    )
    .await;
    let _ = wait_for_chunks(&pool, &file_id, 1).await;

    // Confirm the rows really are in the NULL-embedding (mid-rebuild) state.
    let with_embedding: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM file_chunks WHERE file_id = $1 AND embedding IS NOT NULL",
    )
    .bind(Uuid::parse_str(&file_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(with_embedding, 0, "precondition: embeddings are NULL (mid-rebuild)");

    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let scope = vec![Uuid::parse_str(&file_id).unwrap()];

    // Vector arm with an arbitrary query vector → ZERO hits (NULL rows excluded),
    // and crucially NO dimension-mismatch error.
    let vec_hits = ziee::file_rag_search::vector_search_hit_count_for_test(
        &scope,
        user_uuid,
        &vec![0.05f32; 768],
        1.0,
        10,
    )
    .await
    .expect("vector search must not error on NULL-embedding rows");
    assert_eq!(vec_hits, 0, "the vector arm must exclude NULL-embedding rows");

    // FTS arm still serves the content during the rebuild window.
    let fts_hits = ziee::file_rag_search::fts_search_hit_count_for_test(
        &scope,
        user_uuid,
        "ZEBRAFISH",
        10,
        "simple",
        0.0,
    )
    .await
    .expect("fts search must succeed");
    assert!(fts_hits >= 1, "FTS must still return results while embeddings are NULL");
}

/// Cross-module recovery (file_rag retrieval ↔ memory embed-dispatch): when the
/// deployment WANTS semantic search but the embedding dispatch can't be
/// provisioned — the memory `embed_batch` capability check REJECTS a non-embedder
/// (`INVALID_EMBEDDING_MODEL`), so `embedding_model_id` never persists — file_rag
/// must still serve queries end-to-end via the FTS arm rather than erroring. This
/// joins the two modules' failure-recovery contract that no single-module test
/// exercised together.
#[tokio::test]
async fn semantic_requested_but_embed_dispatch_unavailable_recovers_to_fts() {
    let server = TestServer::start().await;
    let user = power_user(&server, "rag_dispatch_recover").await;
    let pool = db_pool(&server).await;

    // Ask for semantic search, then try to wire a NON-embedder model. The
    // memory embed-dispatch capability check rejects it (400) — the dispatch is
    // effectively unavailable.
    set_rag_settings(&server, &user, json!({ "enabled": true, "semantic_enabled": true })).await;
    let bad_model = create_chat_model(&server, &user).await;
    let resp = put_settings_raw(&server, &user, json!({ "embedding_model_id": bad_model })).await;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "non-embedder must be rejected by the embed-dispatch capability check"
    );
    let settings = get_settings(&server, &user).await;
    assert!(
        settings["embedding_model_id"].is_null(),
        "no usable embedder is configured after the rejection"
    );

    // Upload + attach a file and search: retrieval must RECOVER to FTS, not error.
    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-dispatch-recover",
        &[(
            "doc.txt",
            "The MITOCHONDRION is the powerhouse with distinctive searchable tokens.",
        )],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &file_ids[0], 1).await;

    let body = semantic_search(&server, &user, conv_uuid, "mitochondrion powerhouse").await;
    assert!(body["error"].is_null(), "search must not error; body={body}");
    let sc = &body["result"]["structuredContent"];
    assert_eq!(
        sc["mode"].as_str().unwrap(),
        "fts",
        "embed dispatch unavailable → retrieval recovers to FTS-only"
    );
    assert!(
        !sc["results"].as_array().unwrap().is_empty(),
        "FTS must still return results during embed-dispatch unavailability"
    );
}

/// Cross-module linkage: a file produced by a workflow MCP step
/// (`created_by='workflow'`, the provenance `persist_links` stamps on
/// run-created files) flows through the SAME shared `ingest_bytes` tail as an
/// uploaded file, so it is chunked by file_rag and answerable via the
/// `semantic_search` tool. This guards the workflow→file_rag retrieval seam
/// the audit flagged as untested — proving file_rag retrieval is
/// provenance-agnostic (workflow outputs are first-class searchable content).
#[tokio::test]
async fn workflow_provenance_file_is_chunked_and_retrievable() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_workflow").await;
    let pool = db_pool(&server).await;

    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-workflow",
        &[(
            "workflow-report.txt",
            "Workflow run summary: the migration assistant analyzed 412 call sites \
             and recommends replacing the deprecated tokenizer with the streaming \
             variant to reduce peak memory during ingestion.",
        )],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();

    // Stamp the workflow provenance the run tool-step would set, then confirm
    // the background ingest still produced FTS-ready chunks for it.
    sqlx::query("UPDATE files SET created_by = 'workflow' WHERE id = $1::uuid")
        .bind(&file_ids[0])
        .execute(&pool)
        .await
        .unwrap();
    wait_for_chunks(&pool, &file_ids[0], 1).await;
    let provenance: String =
        sqlx::query_scalar("SELECT created_by FROM files WHERE id = $1::uuid")
            .bind(&file_ids[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(provenance, "workflow", "file is workflow-provenance");

    // The workflow-produced content is retrievable through file_rag.
    let body = semantic_search(&server, &user, conv_uuid, "deprecated tokenizer streaming memory").await;
    assert!(body["error"].is_null(), "semantic_search should succeed; body={body}");
    let results = body["result"]["structuredContent"]["results"]
        .as_array()
        .expect("results array");
    assert!(
        results.iter().any(|r| r["file_id"].as_str() == Some(file_ids[0].as_str())),
        "workflow-provenance file must be retrievable via file_rag; results={results:?}"
    );
}

/// Permission re-gating: a user holding `file_rag::admin::read` can read the
/// RAG admin settings, but once that permission is revoked (admin demotes them
/// / strips the group grant) the very next read is 403 — the settings cannot
/// be re-fetched with stale authority. This is the deterministic core of the
/// "permission-denied mid-stream (RAG settings become stale)" concern: the
/// gate re-resolves the caller's permissions per request, so a revoked user
/// can never pull fresh settings after losing access. (The SSE stream's own
/// 60s liveness re-check is the eventual teardown; this asserts the
/// authoritative per-request gate it relies on.)
#[tokio::test]
async fn rag_admin_settings_regate_on_permission_revocation() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "rag_revoke", &["file_rag::admin::read"]).await;
    let pool = db_pool(&server).await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", user.token);

    // With the permission → 200.
    let ok = client
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200, "holder can read RAG admin settings");

    // Admin revokes file_rag::admin::read from every group the user belongs to.
    let uid = Uuid::parse_str(&user.user_id).unwrap();
    sqlx::query(
        "UPDATE groups SET permissions = array_remove(permissions, 'file_rag::admin::read') \
         WHERE id IN (SELECT group_id FROM user_groups WHERE user_id = $1)",
    )
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();

    // Next read is denied — no stale-authority fetch.
    let denied = client
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403, "revoked user must be re-gated out (403)");
}

/// A file with NO extractable text (an image — text_page_count <= 0) must
/// produce ZERO chunks: `ingest::spawn_index` no-ops on it. Prior coverage
/// only exercised the whitespace-only-TXT case (which still has a text page);
/// this pins the image/binary path where there is no text page at all.
#[tokio::test]
async fn image_file_with_no_text_pages_yields_no_chunks() {
    let server = TestServer::start().await;
    let user = power_user(&server, "rag_image_noindex").await;
    let pool = db_pool(&server).await;

    // Upload a real PNG (binary, no extractable text) through the upload path.
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/file/test_data/test.png"
    ))
    .expect("read png fixture");
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(bytes)
            .file_name("photo.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload png");
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED, "png upload");
    let file_id = resp.json::<Value>().await.unwrap()["id"].as_str().unwrap().to_string();

    // The (no-op) ingest spawn gets a beat; an image has no text page, so no
    // chunk rows are ever produced.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        chunk_count(&pool, &file_id).await,
        0,
        "an image (no text pages) must yield zero file_rag chunks"
    );
}

/// Concurrency: two rewrites of the SAME file fired together each spawn a
/// reindex; the `pg_advisory_xact_lock('file_rag_reindex:'||file_id)` serializes
/// them so the final chunk set reflects exactly ONE rewrite — never a mix and
/// never duplicated/leaked old chunks. The existing reindex tests are
/// sequential (new-version / restore). Uses tokio::join! (shared &server, no
/// clone) for the concurrent dispatch.
#[tokio::test]
async fn concurrent_reindex_same_file_yields_one_consistent_chunk_set() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_concurrent_reindex").await;
    let pool = db_pool(&server).await;

    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-concurrent",
        &[("doc.md", "initial content version zero")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    let file_id = file_ids[0].clone();
    wait_for_chunks(&pool, &file_id, 1).await;
    let initial_count = chunk_count(&pool, &file_id).await;

    // Two concurrent rewrites of the same file (distinct content markers).
    let (r1, r2) = tokio::join!(
        call_tool(
            &server,
            &user,
            conv_uuid,
            "rewrite_file",
            json!({ "id": file_id, "content": "ALPHA marker first concurrent rewrite body" }),
        ),
        call_tool(
            &server,
            &user,
            conv_uuid,
            "rewrite_file",
            json!({ "id": file_id, "content": "BETA marker second concurrent rewrite body" }),
        ),
    );
    assert!(r1["error"].is_null(), "rewrite 1 must not error: {r1}");
    assert!(r2["error"].is_null(), "rewrite 2 must not error: {r2}");

    // Let both reindexes settle, then inspect the final chunk set.
    tokio::time::sleep(Duration::from_millis(800)).await;
    wait_for_chunks(&pool, &file_id, 1).await;
    let chunk_contents: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM file_chunks WHERE file_id = $1 ORDER BY chunk_index",
    )
    .bind(&file_id)
    .fetch_all(&pool)
    .await
    .expect("fetch final chunks");

    let final_count = chunk_contents.len() as i64;
    assert_eq!(
        final_count, initial_count,
        "final chunk count must match a single reindex (no duplicates from the race): \
         got {final_count}, expected {initial_count}; chunks={chunk_contents:?}"
    );
    let has_alpha = chunk_contents.iter().any(|c| c.contains("ALPHA"));
    let has_beta = chunk_contents.iter().any(|c| c.contains("BETA"));
    assert!(
        has_alpha ^ has_beta,
        "chunks must reflect exactly ONE rewrite, not a mix: alpha={has_alpha} beta={has_beta} \
         chunks={chunk_contents:?}"
    );
    pool.close().await;
}

/// Search safety during an embedding rebuild: when a file's chunk embeddings are
/// NULL (the transient state while `embed_worker` ALTERs the column / re-embeds,
/// or simply FTS-only mode), retrieval's `WHERE embedding IS NOT NULL` guard
/// means a concurrent `semantic_search` must NOT crash or return garbage — it
/// degrades to the FTS path and still answers. This pins that guard: we
/// explicitly NULL every embedding (simulating mid-rebuild) and assert search
/// still succeeds.
#[tokio::test]
async fn semantic_search_with_null_embeddings_degrades_gracefully() {
    let server = TestServer::start().await;
    let user = power_user(&server, "file_rag_null_embed").await;
    let pool = db_pool(&server).await;

    let (conv, file_ids) = project_conversation_with_files(
        &server,
        &user,
        "rag-null-embed",
        &[("notes.md", "The Helsinki rendezvous codeword is NULLSAFE-4242 for evacuation.")],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &file_ids[0], 1).await;

    // Simulate the mid-rebuild window: every chunk embedding is NULL.
    sqlx::query("UPDATE file_chunks SET embedding = NULL WHERE file_id = $1")
        .bind(&file_ids[0])
        .execute(&pool)
        .await
        .expect("null out embeddings");

    // A search during this window must not error (the IS NOT NULL guard +
    // FTS fallback keep it answering).
    let body = semantic_search(&server, &user, conv_uuid, "evacuation codeword").await;
    assert!(
        body["error"].is_null(),
        "search during the embedding-rebuild window must not error: {body}"
    );
    pool.close().await;
}


// ─────────────────────────── TEST-6: reranker settings (migration 135) ───────────────────────────

#[tokio::test]
async fn test_6_reranker_settings_roundtrip_and_validation() {
    let server = TestServer::start().await;
    let admin = power_user(&server, "frag_rerank_admin").await;

    // candidate_k out of range (1..=200) → clean 400, not a 500 / DB error.
    let bad = put_settings_raw(&server, &admin, json!({ "rerank_candidate_k": 201 })).await;
    assert_eq!(bad.status(), 400, "candidate_k=201 rejected: {}", bad.text().await.unwrap_or_default());

    // valid rerank tuning persists + reads back.
    let ok = put_settings_raw(
        &server,
        &admin,
        json!({ "rerank_enabled": true, "rerank_candidate_k": 50 }),
    )
    .await;
    assert!(ok.status().is_success(), "valid rerank settings: {}", ok.text().await.unwrap_or_default());
    let got = get_settings(&server, &admin).await;
    assert_eq!(got["rerank_enabled"], true);
    assert_eq!(got["rerank_candidate_k"], 50);

    // A reranker_model_id that isn't a working reranker is rejected by the probe.
    let fake_model = uuid::Uuid::new_v4().to_string();
    let rejected = put_settings_raw(
        &server,
        &admin,
        json!({ "reranker_model_id": fake_model }),
    )
    .await;
    assert_eq!(
        rejected.status(),
        400,
        "a non-existent/non-rerank model is rejected by the probe: {}",
        rejected.text().await.unwrap_or_default()
    );
}

// ─────────────────────────── TEST-11: file_index_state emit (Part I) ───────────────────────────

/// Poll `file_index_state.status` until it is a terminal value or timeout.
async fn wait_index_status(pool: &sqlx::PgPool, file_id: &str) -> String {
    let fid = Uuid::parse_str(file_id).unwrap();
    for _ in 0..40 {
        let s: Option<String> = sqlx::query_scalar(
            "SELECT status FROM file_index_state WHERE file_id = $1",
        )
        .bind(fid)
        .fetch_optional(pool)
        .await
        .expect("query index state");
        if let Some(st) = s {
            if st == "indexed" || st == "no_text" || st == "failed" {
                return st;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for a terminal file_index_state for {file_id}");
}

#[tokio::test]
async fn test_11_index_state_reaches_indexed_and_emits_owner_scoped() {
    use crate::common::sync_probe::SyncProbe;
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let owner = power_user(&server, "fis_owner").await;
    let other = power_user(&server, "fis_other").await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    // A text file is FTS-indexable with no embedder → reaches `indexed`.
    let fid = upload_text(&server, &owner, "idx.txt", "indexed content here").await;

    let frame = owner_probe
        .expect_event("file_index_state", "update", Duration::from_secs(10))
        .await;
    assert_eq!(frame.id, fid, "the index-state sync frame carries the file id");
    // The other user never sees it (owner-scoped audience).
    other_probe.expect_silence(Duration::from_secs(1)).await;

    let status = wait_index_status(&pool, &fid).await;
    assert_eq!(status, "indexed", "a text file with extractable text reaches indexed");
}

#[tokio::test]
async fn test_11b_no_text_file_lands_no_text() {
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let user = power_user(&server, "fis_notext").await;

    // A 1x1 PNG has no extractable text → the distinct `no_text` terminal state.
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    use reqwest::multipart;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(PNG_1X1.to_vec())
            .file_name("pixel.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send().await.unwrap();
    assert_eq!(resp.status(), 201, "png upload: {}", resp.text().await.unwrap_or_default());
    let fid = resp.json::<Value>().await.unwrap()["id"].as_str().unwrap().to_string();

    let status = wait_index_status(&pool, &fid).await;
    assert_eq!(status, "no_text", "an image (no extractable text) lands no_text, not indexed/failed");
}
