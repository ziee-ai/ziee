use serde_json::Value;
use serde_json::json;
use uuid::Uuid;
use crate::common::test_helpers::create_user_with_permissions;
use super::FIXTURE_SKILL_NAME;
use super::admin_and_refresh;
use super::install_fixture_skill;
use super::server_with_skill_catalog;

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

/// `tools/call` error branches in `dispatch_tool_call` (handlers.rs:122-160) —
/// every existing test drives only success/path-traversal. This pins the three
/// untested error paths, each of which must surface as a clean JSON-RPC error
/// result (200 envelope, `error` object, NO `result`/content), never a panic or
/// raw 500:
///   1. `load_skill` for a skill that is not installed → `not_found`
///      ("skill not installed", tools.rs:205) — the bad-name path.
///   2. `load_skill` with an empty `name` → `VALIDATION_ERROR` (tools.rs:87).
///   3. `tools/call` for an unknown tool name → `method_not_found`
///      ("skill tool: …", handlers.rs:148).
#[tokio::test]
async fn tools_call_error_branches_surface_as_jsonrpc_errors() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    // NOTE: deliberately do NOT install the fixture skill — so a load_skill for
    // it resolves to "not installed".

    // 1. Unknown / not-installed skill name → not_found error, no content leaked.
    let missing: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "load_skill", "arguments": { "name": "no-such-skill-xyz" } }),
    )
    .send()
    .await
    .expect("load_skill missing")
    .json()
    .await
    .expect("parse");
    assert!(
        missing["error"].is_object(),
        "loading a non-installed skill must be a JSON-RPC error: {missing}"
    );
    assert!(
        missing["result"].is_null(),
        "no result/content on a not-found skill: {missing}"
    );
    let msg = missing["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(
        msg.contains("not installed") || msg.contains("not found"),
        "rejection names the missing skill: {missing}"
    );

    // 2. Empty `name` → validation error (the empty-name guard at tools.rs:87).
    let empty: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "load_skill", "arguments": { "name": "" } }),
    )
    .send()
    .await
    .expect("load_skill empty")
    .json()
    .await
    .expect("parse");
    assert!(
        empty["error"].is_object() && empty["result"].is_null(),
        "an empty skill name must be a validation error with no result: {empty}"
    );

    // 3. Unknown tool name → method_not_found (the `other =>` arm).
    let unknown: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "definitely_not_a_real_tool", "arguments": {} }),
    )
    .send()
    .await
    .expect("unknown tool")
    .json()
    .await
    .expect("parse");
    assert!(
        unknown["error"].is_object() && unknown["result"].is_null(),
        "an unknown tool name must be a method-not-found error: {unknown}"
    );
    assert_eq!(
        unknown["error"]["code"], -32601,
        "unknown tool → JSON-RPC method-not-found code (-32601): {unknown}"
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

/// `load_skill` for a skill that is not installed returns a JSON-RPC error
/// (not_found), never a result — the "skill not installed" resolve-to-None path.
#[tokio::test]
async fn load_skill_unknown_name_errors() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    // NOTE: deliberately do NOT install the fixture skill.

    let body: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "load_skill", "arguments": { "name": "io.github.test/does-not-exist" } }),
    )
    .send()
    .await
    .expect("load_skill")
    .json()
    .await
    .expect("parse");

    assert!(
        body["error"].is_object(),
        "loading an uninstalled skill must error: {body}"
    );
    assert!(body["result"].is_null(), "no result on a not-found skill: {body}");
}

/// `read_skill_file` for a path that doesn't exist WITHIN an installed skill
/// returns a JSON-RPC error (not_found), never partial/empty content.
#[tokio::test]
async fn read_skill_file_missing_path_errors() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    install_fixture_skill(&server, &admin.token).await;

    let body: Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({
            "name": "read_skill_file",
            "arguments": { "name": FIXTURE_SKILL_NAME, "path": "references/nope-not-here.md" }
        }),
    )
    .send()
    .await
    .expect("read_skill_file")
    .json()
    .await
    .expect("parse");

    assert!(
        body["error"].is_object(),
        "reading a missing file inside a skill must error: {body}"
    );
    assert!(body["result"].is_null(), "no content on a missing file: {body}");
}

