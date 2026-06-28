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

/// The MCP tool result must carry BOTH channels (handlers.rs:148-151): a
/// human/text `content[]` block AND a machine-readable `structuredContent`,
/// with the text channel being the stringified structured value. Existing
/// tests only read `structuredContent`; this pins the dual-channel contract.
#[tokio::test]
async fn load_skill_response_has_dual_text_and_structured_channels() {
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
    let result = &body["result"];

    // Text channel: a `content` array whose first block is type=text.
    let block = &result["content"][0];
    assert_eq!(block["type"], "text", "first content block is text: {body}");
    let text = block["text"]
        .as_str()
        .unwrap_or_else(|| panic!("text channel present: {body}"));
    assert!(!text.is_empty(), "text channel non-empty: {body}");

    // Structured channel: a machine-readable object present alongside text.
    let structured = &result["structuredContent"];
    assert!(
        structured.is_object(),
        "structuredContent present as object: {body}"
    );

    // The text channel is exactly the stringified structured value
    // (handlers does `text: v.to_string()`, `structuredContent: v`).
    let reparsed: Value = serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("text channel is the JSON-encoded structured value ({e}): {text}"));
    assert_eq!(
        &reparsed, structured,
        "text channel mirrors structuredContent"
    );
}

/// Auth boundary on the skill_mcp JSON-RPC handler (handlers.rs:29-33,
/// `RequirePermissions<(SkillsRead,)>`). All other tests use valid admin tokens;
/// these assert the gate: NO token → 401, a token WITHOUT skills::read → 403.
#[tokio::test]
async fn jsonrpc_requires_authentication() {
    let (server, _mock) = server_with_skill_catalog().await;

    // No Authorization header → 401.
    let res = reqwest::Client::new()
        .post(server.api_url("/skills/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }))
        .send()
        .await
        .expect("request");
    assert_eq!(res.status(), 401, "missing token must be 401");
}

#[tokio::test]
async fn jsonrpc_requires_skills_read_permission() {
    let (server, _mock) = server_with_skill_catalog().await;

    // A user WITHOUT skills::read.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "skillmcp_noperm",
        &[],
    )
    .await;

    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .expect("request");
    assert_eq!(res.status(), 403, "a token lacking skills::read must be 403");
}
