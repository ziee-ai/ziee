//! audit id all-1808eb088eaf — cross-subsystem combined flow. The skill tests
//! are otherwise skill-isolated; nothing exercises a single user/conversation
//! driving MULTIPLE built-in MCP subsystems together. This test installs a
//! skill (skill subsystem), then within ONE conversation calls skill_mcp
//! (load_skill), files_mcp (create_file), and memory_mcp (remember + recall) —
//! proving the built-in MCP servers compose under a shared conversation scope
//! and each persists independently. No LLM: the built-in JSON-RPC endpoints are
//! the real behavior under test; nothing is mocked.

use serde_json::{Value, json};
use uuid::Uuid;

use super::{FIXTURE_SKILL_NAME, install_fixture_skill, server_with_skill_catalog};
use crate::common::test_helpers::create_user_with_permissions;

/// POST a JSON-RPC tools/call to a built-in MCP endpoint with the conversation
/// header every conversation-scoped server requires.
async fn call(
    server: &crate::common::TestServer,
    token: &str,
    endpoint: &str,
    conv_id: Uuid,
    name: &str,
    arguments: Value,
) -> Value {
    let res = reqwest::Client::new()
        .post(server.api_url(endpoint))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", conv_id.to_string())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        }))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{endpoint} {name}: {e}"));
    assert_eq!(res.status(), 200, "{endpoint} {name} HTTP status");
    res.json().await.unwrap()
}

#[tokio::test]
async fn skill_files_and_memory_compose_in_one_conversation() {
    let (server, _mock) = server_with_skill_catalog().await;
    // One identity with broad perms drives every subsystem.
    let user = create_user_with_permissions(
        &server,
        "xsub_user",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "files::read",
            "files::write",
            "memory::read",
            "memory::write",
        ],
    )
    .await;
    // refresh_catalog runs inside install_fixture_skill's precondition via the
    // catalog server; ensure catalog is active before install.
    super::refresh_catalog(&server, &user.token).await;
    install_fixture_skill(&server, &user.token).await;

    // A single conversation shared across all three subsystems.
    let conv_res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .expect("create conv");
    assert_eq!(conv_res.status(), 201);
    let conv_id =
        Uuid::parse_str(conv_res.json::<Value>().await.unwrap()["id"].as_str().unwrap()).unwrap();

    // (1) skill subsystem — load the installed skill's body.
    let skill = call(
        &server,
        &user.token,
        "/skills/mcp",
        conv_id,
        "load_skill",
        json!({ "name": FIXTURE_SKILL_NAME }),
    )
    .await;
    assert!(skill["error"].is_null(), "load_skill: {skill}");
    let body = skill["result"]["structuredContent"]["content"]
        .as_str()
        .unwrap_or_else(|| panic!("skill body: {skill}"));
    assert!(
        body.contains("THIS_IS_THE_SKILL_BODY_MARKER"),
        "skill body returned in the combined flow: {body}"
    );

    // (2) files subsystem — create a file in the same conversation.
    let created = call(
        &server,
        &user.token,
        "/files/mcp",
        conv_id,
        "create_file",
        json!({ "filename": "xsub.txt", "content": "cross-subsystem payload" }),
    )
    .await;
    assert!(created["error"].is_null(), "create_file: {created}");

    // (3) memory subsystem — remember a fact, then recall it, same conversation.
    let remembered = call(
        &server,
        &user.token,
        "/memories/mcp",
        conv_id,
        "remember",
        json!({ "content": "User integrates skills, files, and memory together", "kind": "fact" }),
    )
    .await;
    assert!(remembered["error"].is_null(), "remember: {remembered}");

    let recalled = call(
        &server,
        &user.token,
        "/memories/mcp",
        conv_id,
        "recall",
        json!({ "query": "what does the user integrate", "top_k": 5 }),
    )
    .await;
    assert!(recalled["error"].is_null(), "recall: {recalled}");
    let mems = recalled["result"]["structuredContent"]["memories"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        mems.iter()
            .filter_map(|m| m["content"].as_str())
            .any(|c| c.contains("skills, files, and memory")),
        "the just-remembered fact must be recallable in the same flow: {recalled}"
    );
}
