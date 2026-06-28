//! `skill_mcp` built-in MCP server (JSON-RPC at /api/skills/mcp):
//! `load_skill` returns the SKILL.md body; `read_skill_file` returns a
//! supporting file's content and REJECTS path traversal (`../...`).

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;

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

/// Auth boundary — no Authorization header.
///
/// The handler is gated by `RequirePermissions<(SkillsRead,)>`
/// (`handlers.rs:29-33`), whose extractor 401s before any JSON-RPC dispatch
/// when no bearer token is present. A POST with a well-formed `tools/list`
/// body but no `Authorization` header must never reach the tool layer. Every
/// other test in this file passes a valid token, so this boundary was
/// previously uncovered.
#[tokio::test]
async fn skill_mcp_rejects_missing_jwt_with_401() {
    let (server, _mock) = server_with_skill_catalog().await;

    let res = reqwest::Client::new()
        .post(server.api_url("/skills/mcp"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {},
        }))
        .send()
        .await
        .expect("request");

    assert_eq!(
        res.status(),
        401,
        "no Authorization header must be rejected with 401 UNAUTHORIZED"
    );
}

/// Auth boundary — authenticated but lacking `skills::read`.
///
/// 403 IS applicable: the route requires `skills::read`, resolved LIVE from the
/// DB on each request (`extractors.rs` → `get_user_groups`). A freshly-registered
/// user holds it via the default group (that's why the other tests pass a `&[]`
/// admin and still get results); stripping the user's group memberships after
/// registration leaves them authenticated but unauthorized — the gate must
/// answer 403, not 200/401.
#[tokio::test]
async fn skill_mcp_rejects_user_without_skills_read_with_403() {
    let (server, _mock) = server_with_skill_catalog().await;
    let user = create_user_with_permissions(&server, "skill_mcp_noperm", &[]).await;

    // Drop every group membership → user now has zero permissions.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let uid = Uuid::parse_str(&user.user_id).unwrap();
    sqlx::query("DELETE FROM user_groups WHERE user_id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    // The token itself is still valid; only the live permission check should fail.
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .expect("request");

    assert_eq!(
        res.status(),
        403,
        "an authenticated user lacking skills::read must be rejected with 403 FORBIDDEN"
    );
}
