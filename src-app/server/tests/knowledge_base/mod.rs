//! Integration tests for the knowledge_base module — CRUD, permission/owner
//! isolation, the search_knowledge tool (FTS-only, no embedder needed), the
//! cross-user leak guard, and the MCP surface. Runs against the real TestServer
//! harness (spawned server + per-test isolated DB).

use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

use crate::common::test_helpers::{
    create_user_with_no_permissions, create_user_with_permissions, TestUser,
};
use crate::common::TestServer;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

// ─────────────────────────── helpers ───────────────────────────

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

async fn wait_for_chunks(pool: &PgPool, file_id: &str, min: i64) {
    let fid = Uuid::parse_str(file_id).unwrap();
    for _ in 0..40 {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
            .bind(fid)
            .fetch_one(pool)
            .await
            .expect("count chunks");
        if n >= min {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for >= {min} chunks for file {file_id}");
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
    assert_eq!(resp.status(), 201, "upload: {}", resp.text().await.unwrap_or_default());
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

async fn create_kb(server: &TestServer, user: &TestUser, name: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/knowledge-bases"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create kb: {}", resp.text().await.unwrap_or_default());
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

async fn attach_docs(
    server: &TestServer,
    user: &TestUser,
    kb_id: &str,
    file_ids: &[&str],
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/knowledge-bases/{kb_id}/documents")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "file_ids": file_ids }))
        .send()
        .await
        .unwrap()
}

fn kb_jsonrpc(server: &TestServer, token: &str, method: &str, params: Value) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/knowledge-base/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

/// Call search_knowledge over an explicit KB set; returns the structuredContent.
async fn search_knowledge(
    server: &TestServer,
    token: &str,
    query: &str,
    kb_ids: &[&str],
) -> Value {
    let resp = kb_jsonrpc(
        server,
        token,
        "tools/call",
        json!({ "name": "search_knowledge", "arguments": { "query": query, "knowledge_base_ids": kb_ids } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = resp.json().await.unwrap();
    body["result"]["structuredContent"].clone()
}

// ─────────────────────────── TEST-20: CRUD ───────────────────────────

#[tokio::test]
async fn test_20_kb_crud_lifecycle() {
    let server = TestServer::start().await;
    let user = power_user(&server, "kb_crud").await;
    let client = reqwest::Client::new();

    // create
    let kb_id = create_kb(&server, &user, "Lab protocols").await;

    // list → exactly one
    let list: Value = client
        .get(server.api_url("/knowledge-bases"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["document_count"], 0, "new KB has a live COUNT of 0");

    // get by id
    let got: Value = client
        .get(server.api_url(&format!("/knowledge-bases/{kb_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(got["name"], "Lab protocols");

    // update (rename)
    let up = client
        .put(server.api_url(&format!("/knowledge-bases/{kb_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Renamed KB" }))
        .send().await.unwrap();
    assert_eq!(up.status(), 200);
    let up_body: Value = up.json().await.unwrap();
    assert_eq!(up_body["name"], "Renamed KB");

    // delete → list empty
    let del = client
        .delete(server.api_url(&format!("/knowledge-bases/{kb_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert!(del.status().is_success(), "delete status {}", del.status());
    let list2: Value = client
        .get(server.api_url("/knowledge-bases"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(list2.as_array().unwrap().len(), 0);
}

// ─────────────────────────── TEST-24: permissions + owner isolation ───────────────────────────

#[tokio::test]
async fn test_24_permission_and_owner_isolation() {
    let server = TestServer::start().await;

    // A user with NO permissions cannot list KBs.
    let noperm = create_user_with_no_permissions(&server, "kb_noperm").await;
    let r = reqwest::Client::new()
        .get(server.api_url("/knowledge-bases"))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send().await.unwrap();
    assert_eq!(r.status(), 403, "no knowledge_base::use → 403");

    // A default Users-group member (no explicit perms) succeeds (migration 134).
    let member = create_user_with_permissions(&server, "kb_member", &[]).await;
    let r2 = reqwest::Client::new()
        .get(server.api_url("/knowledge-bases"))
        .header("Authorization", format!("Bearer {}", member.token))
        .send().await.unwrap();
    assert_eq!(r2.status(), 200, "default Users member holds knowledge_base::use");

    // Owner isolation: user B cannot GET user A's KB → 404 (get_by_id_and_user).
    let a = power_user(&server, "kb_owner_a").await;
    let b = power_user(&server, "kb_owner_b").await;
    let kb_a = create_kb(&server, &a, "A private KB").await;
    let foreign = reqwest::Client::new()
        .get(server.api_url(&format!("/knowledge-bases/{kb_a}")))
        .header("Authorization", format!("Bearer {}", b.token))
        .send().await.unwrap();
    assert_eq!(foreign.status(), 404, "a foreign KB is 404, never 200/403");
}

// ─────────────────────────── TEST-25: search scope + cross-user leak guard ───────────────────────────

#[tokio::test]
async fn test_25_search_knowledge_scope_and_cross_user_leak_guard() {
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let a = power_user(&server, "kb_search_a").await;
    let b = power_user(&server, "kb_search_b").await;

    // User A: a doc with an unguessable phrase, attached to KB-A.
    let phrase = "quokka telemetry 4517 anomaly";
    let fid = upload_text(&server, &a, "a.txt", &format!("Intro. {phrase}. Conclusion.")).await;
    wait_for_chunks(&pool, &fid, 1).await; // FTS-indexed (no embedder needed)
    let kb_a = create_kb(&server, &a, "A KB").await;
    let att = attach_docs(&server, &a, &kb_a, &[&fid]).await;
    assert_eq!(att.status(), 200, "attach: {}", att.text().await.unwrap_or_default());

    // A searches KB-A → finds the passage (FTS-only hybrid).
    let sc = search_knowledge(&server, &a.token, "quokka telemetry anomaly", &[&kb_a]).await;
    let hits = sc["hits"].as_array().cloned().unwrap_or_default();
    assert!(!hits.is_empty(), "owner search returns the passage: {sc}");
    assert!(hits[0]["content"].as_str().unwrap().contains("quokka"));

    // User B calls search_knowledge with A's kb_id (foreign) — must get ZERO of
    // A's chunks (resolve_scope_file_ids is owner-filtered). This is the tool
    // cross-tenant leak guard.
    let sc_b = search_knowledge(&server, &b.token, "quokka telemetry anomaly", &[&kb_a]).await;
    let hits_b = sc_b["hits"].as_array().cloned().unwrap_or_default();
    assert!(hits_b.is_empty(), "B must NOT see A's KB chunks via A's kb_id: {sc_b}");

    // Mixed array (B's own empty KB + A's foreign kb) → still zero A hits.
    let kb_b = create_kb(&server, &b, "B KB").await;
    let sc_mixed = search_knowledge(&server, &b.token, "quokka telemetry anomaly", &[&kb_b, &kb_a]).await;
    let hits_mixed = sc_mixed["hits"].as_array().cloned().unwrap_or_default();
    assert!(hits_mixed.is_empty(), "mixed own+foreign array leaks nothing from A: {sc_mixed}");
}

// ─────────────────────────── TEST-21: documents attach + duplicate skip ───────────────────────────

#[tokio::test]
async fn test_21_attach_documents_and_duplicate_skip() {
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let user = power_user(&server, "kb_docs").await;

    let fid = upload_text(&server, &user, "doc.txt", "hello knowledge world").await;
    wait_for_chunks(&pool, &fid, 1).await;
    let kb = create_kb(&server, &user, "Docs KB").await;

    // first attach → 1 attached, 0 skipped
    let r1 = attach_docs(&server, &user, &kb, &[&fid]).await;
    assert_eq!(r1.status(), 200);
    let b1: Value = r1.json().await.unwrap();
    assert_eq!(b1["attached"], 1);
    assert_eq!(b1["skipped_duplicates"], 0);

    // re-drop the SAME file → skipped as duplicate, not double-attached
    let r2 = attach_docs(&server, &user, &kb, &[&fid]).await;
    assert_eq!(r2.status(), 200);
    let b2: Value = r2.json().await.unwrap();
    assert_eq!(b2["attached"], 0, "duplicate not re-attached");
    assert_eq!(b2["skipped_duplicates"], 1, "duplicate reported");

    // document_count reflects exactly one document
    let got: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/knowledge-bases/{kb}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(got["document_count"], 1);
}

// ─────────────────────────── TEST-29: MCP surface ───────────────────────────

#[tokio::test]
async fn test_29_mcp_initialize_tools_and_gate() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "kb_mcp", &["knowledge_base::use"]).await;

    // initialize → serverInfo name
    let init = kb_jsonrpc(&server, &user.token, "initialize", json!({}))
        .send().await.unwrap();
    assert_eq!(init.status(), 200);
    let ib: Value = init.json().await.unwrap();
    assert_eq!(ib["result"]["serverInfo"]["name"], "knowledge_base");

    // tools/list → both tools present
    let tl = kb_jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send().await.unwrap();
    let tb: Value = tl.json().await.unwrap();
    let names: Vec<String> = tb["result"]["tools"].as_array().unwrap()
        .iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"search_knowledge".to_string()), "tools: {names:?}");
    assert!(names.contains(&"list_knowledge_bases".to_string()), "tools: {names:?}");

    // no-use user → 403 on the MCP endpoint
    let noperm = create_user_with_no_permissions(&server, "kb_mcp_noperm").await;
    let gated = kb_jsonrpc(&server, &noperm.token, "tools/list", json!({}))
        .send().await.unwrap();
    assert_eq!(gated.status(), 403, "search_knowledge MCP gates on knowledge_base::use");
}

// ─────────────────────────── TEST-46: docs presence ───────────────────────────

#[tokio::test]
async fn test_46_claude_md_documents_the_feature() {
    // ITEM-42: the developer docs describe the KB feature accurately.
    let claude_md = include_str!("../../../../CLAUDE.md");
    assert!(claude_md.contains("Knowledge Base"), "CLAUDE.md has a Knowledge Base header");
    assert!(claude_md.contains("search_knowledge"), "names the search_knowledge tool");
    assert!(claude_md.contains("rerank"), "names the rerank capability");
    assert!(claude_md.contains("file_index_state"), "names the index-state table");
}

// ─────────────────────────── TEST-22: shared chunks data-integrity ───────────────────────────

#[tokio::test]
async fn test_22_shared_chunks_survive_kb_removal_and_delete() {
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let user = power_user(&server, "kb_shared").await;

    let fid = upload_text(&server, &user, "shared.txt", "shared chunk survival phrase").await;
    wait_for_chunks(&pool, &fid, 1).await;
    let n0: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
        .bind(Uuid::parse_str(&fid).unwrap()).fetch_one(&pool).await.unwrap();
    assert!(n0 >= 1);

    let kb_a = create_kb(&server, &user, "KB-A").await;
    let kb_b = create_kb(&server, &user, "KB-B").await;
    assert_eq!(attach_docs(&server, &user, &kb_a, &[&fid]).await.status(), 200);
    assert_eq!(attach_docs(&server, &user, &kb_b, &[&fid]).await.status(), 200);

    // Remove F from KB-A → still retrievable via KB-B, and file_chunks UNCHANGED
    // (the join row is deleted, never the shared chunks).
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/knowledge-bases/{kb_a}/documents/{fid}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert!(del.status().is_success(), "remove doc: {}", del.status());
    let n1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
        .bind(Uuid::parse_str(&fid).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(n0, n1, "removing from KB-A must not touch shared file_chunks");
    let via_b = search_knowledge(&server, &user.token, "shared chunk survival", &[&kb_b]).await;
    assert!(!via_b["hits"].as_array().unwrap().is_empty(), "still retrievable via KB-B");

    // Delete KB-A entirely → the file + its chunks survive (only join rows cascade).
    let delkb = reqwest::Client::new()
        .delete(server.api_url(&format!("/knowledge-bases/{kb_a}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert!(delkb.status().is_success());
    let n2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
        .bind(Uuid::parse_str(&fid).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(n0, n2, "deleting KB-A must not delete the shared file's chunks");
    let file_alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE id = $1")
        .bind(Uuid::parse_str(&fid).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(file_alive, 1, "the file itself survives KB deletion");
}

// ─────────────────────────── TEST-27: conversation + project attachment ───────────────────────────

#[tokio::test]
async fn test_27_conversation_and_project_attachment() {
    let server = TestServer::start().await;
    let user = power_user(&server, "kb_attach").await;
    let client = reqwest::Client::new();
    let kb = create_kb(&server, &user, "Attach KB").await;

    // conversation attach → GET lists it → detach → empty
    let conv: Value = client.post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({})).send().await.unwrap().json().await.unwrap();
    let cid = conv["id"].as_str().unwrap();
    let put = client.put(server.api_url(&format!("/conversations/{cid}/knowledge-bases/{kb}")))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap();
    assert!(put.status().is_success(), "attach conv: {}", put.status());
    let listed: Value = client.get(server.api_url(&format!("/conversations/{cid}/knowledge-bases")))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap().json().await.unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1, "conversation lists its attached KB");
    let det = client.delete(server.api_url(&format!("/conversations/{cid}/knowledge-bases/{kb}")))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap();
    assert!(det.status().is_success());
    let listed2: Value = client.get(server.api_url(&format!("/conversations/{cid}/knowledge-bases")))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap().json().await.unwrap();
    assert_eq!(listed2.as_array().unwrap().len(), 0, "detach removes it");

    // project attach → GET lists it
    let proj: Value = client.post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "P" })).send().await.unwrap().json().await.unwrap();
    let pid = proj["id"].as_str().unwrap();
    let pput = client.put(server.api_url(&format!("/projects/{pid}/knowledge-bases/{kb}")))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap();
    assert!(pput.status().is_success());
    let plisted: Value = client.get(server.api_url(&format!("/projects/{pid}/knowledge-bases")))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap().json().await.unwrap();
    assert_eq!(plisted.as_array().unwrap().len(), 1, "project lists its attached KB");

    // foreign KB attach → 404 (another user's conversation/kb)
    let other = power_user(&server, "kb_attach_other").await;
    let foreign = client.put(server.api_url(&format!("/conversations/{cid}/knowledge-bases/{kb}")))
        .header("Authorization", format!("Bearer {}", other.token)).send().await.unwrap();
    assert_eq!(foreign.status(), 404, "attaching to another user's conversation → 404");
}

// ─────────────────────────── TEST-28: owner-scoped sync emit ───────────────────────────

#[tokio::test]
async fn test_28_mutations_emit_owner_scoped_sync() {
    use crate::common::sync_probe::SyncProbe;
    let server = TestServer::start().await;
    let owner = power_user(&server, "kb_sync_owner").await;
    let other = power_user(&server, "kb_sync_other").await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let kb_id = create_kb(&server, &owner, "Synced KB").await;

    // The owner receives a knowledge_base/create carrying the new id.
    let frame = owner_probe
        .expect_event("knowledge_base", "create", std::time::Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, kb_id, "sync frame carries the new KB id");

    // The other user receives NOTHING (owner-scoped audience).
    other_probe.expect_silence(std::time::Duration::from_secs(2)).await;
}
