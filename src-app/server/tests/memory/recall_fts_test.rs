//! audit id all-f0a4a15d42da — the memory retrieval path (retriever.rs
//! `recall_memories`, the engine behind the chat extension's
//! retrieve-and-inject flow) had no test that actually RETRIEVES seeded
//! memories. This drives the FTS-only arm end-to-end through the real server
//! process via the MCP `recall` tool (no embedding model / LLM required):
//! enable memory (FTS, semantic off), seed memories via the REST API, then
//! recall by a matching query and assert the seeded memory comes back.
//! Core-memory block CRUD (the other half of the combined flow) is covered by
//! core_memory_test.rs.

use serde_json::{Value, json};

async fn enable_fts_memory(server: &crate::common::TestServer, admin_token: &str) {
    // FTS-only: no embedding model needed; semantic off.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "enabled": true,
            "fts_enabled": true,
            "semantic_enabled": false,
        }))
        .send()
        .await
        .expect("PUT memory/admin-settings");
    assert!(res.status().is_success(), "enable FTS memory failed: {}", res.status());
}

async fn create_memory(server: &crate::common::TestServer, token: &str, content: &str) {
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "content": content, "kind": "fact" }))
        .send()
        .await
        .expect("POST /memories");
    assert_eq!(res.status(), 201, "create memory failed: {:?}", res.text().await);
}

#[tokio::test]
async fn test_fts_recall_returns_seeded_memories() {
    let server = crate::common::TestServer::start().await;
    // Enabling FTS via PUT /memory/admin-settings requires memory::admin::manage
    // (see handlers::update_admin_settings); the user also needs memory::read /
    // memory::write to create + recall its own memories in the same test.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "fts_recall_admin",
        &[
            "memory::read",
            "memory::write",
            "memory::admin::read",
            "memory::admin::manage",
        ],
    )
    .await;
    enable_fts_memory(&server, &admin.token).await;

    // Enable retrieval for this user.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "retrieval_enabled": true }))
        .send()
        .await
        .expect("PUT /memory/settings");
    assert!(res.status().is_success());

    // Seed several memories; one carries a distinctive lexical token.
    create_memory(&server, &admin.token, "User's favorite programming language is Rust").await;
    create_memory(&server, &admin.token, "User lives in Oslo, Norway").await;
    create_memory(&server, &admin.token, "User prefers dark mode editors").await;

    // Recall via the MCP tool (drives recall_memories' FTS arm in-process).
    let res = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "recall", "arguments": { "query": "what programming language does the user like", "top_k": 5 } }
        }))
        .send()
        .await
        .expect("MCP recall POST");
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.expect("recall body");
    assert!(body["error"].is_null(), "recall should succeed with memory enabled: {body}");

    let memories = body["result"]["structuredContent"]["memories"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let contents: Vec<String> = memories
        .iter()
        .filter_map(|m| m["content"].as_str().map(String::from))
        .collect();
    assert!(
        contents.iter().any(|c| c.contains("Rust")),
        "FTS recall must surface the lexically-matching seeded memory; got: {contents:?}"
    );
}
