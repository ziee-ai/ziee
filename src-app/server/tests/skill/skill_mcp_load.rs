//! `skill_mcp` built-in MCP server (JSON-RPC at /api/skills/mcp):
//! `load_skill` returns the SKILL.md body; `read_skill_file` returns a
//! supporting file's content and REJECTS path traversal (`../...`).

use serde_json::{Value, json};

use super::{FIXTURE_SKILL_NAME, admin_and_refresh, install_fixture_skill, server_with_skill_catalog};

fn jsonrpc(
    server: &crate::common::TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/skills/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
}

#[tokio::test]
async fn tools_list_exposes_load_and_read() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;

    let body: Value = jsonrpc(&server, &admin.token, "tools/list", json!({}))
        .send()
        .await
        .expect("tools/list")
        .json()
        .await
        .expect("parse");
    let tools = body["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"load_skill"), "load_skill exposed: {names:?}");
    assert!(
        names.contains(&"read_skill_file"),
        "read_skill_file exposed: {names:?}"
    );
}

#[tokio::test]
async fn load_skill_returns_body() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    install_fixture_skill(&server, &admin.token).await;

    let body: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "load_skill", "arguments": { "name": FIXTURE_SKILL_NAME } }),
    )
    .send()
    .await
    .expect("load_skill")
    .json()
    .await
    .expect("parse");

    assert!(body["error"].is_null(), "load_skill should succeed: {body}");
    let content = body["result"]["structuredContent"]["content"]
        .as_str()
        .unwrap_or_else(|| panic!("load_skill returns content: {body}"));
    // The body is returned with frontmatter stripped.
    assert!(
        content.contains("THIS_IS_THE_SKILL_BODY_MARKER"),
        "load_skill returns the SKILL.md body: {content}"
    );
    assert!(
        !content.contains("description: How to configure"),
        "frontmatter is stripped from the body: {content}"
    );
}

#[tokio::test]
async fn read_skill_file_reads_reference_and_rejects_traversal() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    install_fixture_skill(&server, &admin.token).await;

    // Legit reference path → content returned.
    let ok: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({
            "name": "read_skill_file",
            "arguments": { "name": FIXTURE_SKILL_NAME, "path": "references/provider-types.md" }
        }),
    )
    .send()
    .await
    .expect("read_skill_file ok")
    .json()
    .await
    .expect("parse");
    assert!(ok["error"].is_null(), "valid read should succeed: {ok}");
    let content = ok["result"]["structuredContent"]["content"]
        .as_str()
        .unwrap_or_else(|| panic!("read returns content: {ok}"));
    assert!(
        content.contains("REFERENCE_FILE_MARKER"),
        "reference file content returned: {content}"
    );

    // Path traversal → JSON-RPC error (no content leaked).
    let bad: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({
            "name": "read_skill_file",
            "arguments": { "name": FIXTURE_SKILL_NAME, "path": "../../../../etc/passwd" }
        }),
    )
    .send()
    .await
    .expect("read_skill_file traversal")
    .json()
    .await
    .expect("parse");
    assert!(
        bad["error"].is_object(),
        "path traversal must be rejected with a JSON-RPC error: {bad}"
    );
    assert!(
        bad["result"].is_null(),
        "no result/content on rejection: {bad}"
    );
    let msg = bad["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.to_lowercase().contains("..") || msg.to_lowercase().contains("path"),
        "rejection mentions path safety: {msg}"
    );
}
