//! Regression: the "Test connection" probe must succeed for EVERY built-in
//! (loopback) system MCP server, present and future.
//!
//! Built-in servers (Skills, Workflows, memory, files, web-search, …) are
//! gated by `RequirePermissions` on their loopback routes, so a probe with no
//! `Authorization` header 401s with MISSING_TOKEN. The fix injects a short-lived
//! per-user JWT via the SHARED `McpSessionManager::inject_builtin_context_headers`
//! (the same helper the live session path uses). This test enumerates the
//! built-in servers from the admin API and asserts each one passes
//! `POST /mcp/system-servers/test-connection` — so a NEW built-in server that
//! forgets the auth wiring (or a regression in the shared helper) fails here.

use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

#[tokio::test]
async fn builtin_system_servers_pass_test_connection() {
    let server = TestServer::start().await;
    // `*` so the minted probe JWT (resolved server-side by user id) satisfies
    // every built-in route's permission gate (skills::read, workflows::read, …).
    let admin = create_user_with_permissions(&server, "builtin_probe_admin", &["*"]).await;

    // Enumerate the registered system servers.
    let list: Value = reqwest::Client::new()
        .get(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list system servers")
        .json()
        .await
        .expect("parse list");

    let builtins: Vec<&Value> = list["servers"]
        .as_array()
        .expect("servers array")
        .iter()
        .filter(|s| {
            s["is_built_in"].as_bool().unwrap_or(false)
                && s["transport_type"] == "http"
                && s["enabled"].as_bool().unwrap_or(false)
        })
        .collect();

    assert!(
        !builtins.is_empty(),
        "expected at least the Skills + Workflows built-in servers to be registered; got: {}",
        list["servers"]
    );

    for s in builtins {
        let id = s["id"].as_str().expect("built-in id");
        let url = s["url"].as_str().expect("built-in url");
        let name = s["name"].as_str().unwrap_or("?");

        let resp = reqwest::Client::new()
            .post(server.api_url("/mcp/system-servers/test-connection"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({
                "id": id,
                "transport_type": "http",
                "url": url,
                "timeout_seconds": 10,
            }))
            .send()
            .await
            .expect("test-connection request");

        let status = resp.status();
        let body: Value = resp.json().await.expect("parse test-connection body");
        assert_eq!(status, 200, "built-in '{name}' test-connection HTTP {status}: {body}");
        assert_eq!(
            body["success"], Value::Bool(true),
            "built-in server '{name}' ({url}) must pass its connection test — a 401 here means \
             the internal JWT wasn't injected (see McpSessionManager::inject_builtin_context_headers); body: {body}"
        );
    }
}

/// TEST-22 — the built-in internal JWT must carry the user's CURRENT
/// access-token revocation epoch.
///
/// `inject_builtin_context_headers` mints a per-user token for every built-in
/// (loopback) server call. Those routes are gated by `RequirePermissions`,
/// which now also verifies `users.token_version`. If the minted token carried a
/// stale/defaulted epoch, every built-in tool call (memory, files, web_search,
/// code_sandbox, …) would 401 for any user who had EVER logged out — their
/// epoch is ≥ 1 while the token would claim 0.
///
/// So: log out (epoch 0 → 1), log back in, and assert the built-ins still probe
/// green. This fails loudly if a future caller mints with a constant.
#[tokio::test]
async fn builtin_probe_still_works_after_a_logout_bumped_the_token_epoch() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "builtin_probe_after_logout", &["*"]).await;
    let client = reqwest::Client::new();

    // Bump the epoch: log out with the first session.
    let res = client
        .post(server.api_url("/auth/logout"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("logout");
    assert_eq!(res.status(), 204);

    // Fresh session for the same user — its token carries epoch 1.
    let res = client
        .post(server.api_url("/auth/login"))
        .json(&json!({
            "username": "builtin_probe_after_logout",
            "password": "testpass123",
        }))
        .send()
        .await
        .expect("re-login");
    assert_eq!(res.status(), 200, "re-login should succeed");
    let body: Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().expect("access_token").to_string();

    let list: Value = client
        .get(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list system servers")
        .json()
        .await
        .expect("parse list");

    let builtins: Vec<&Value> = list["servers"]
        .as_array()
        .expect("servers array")
        .iter()
        .filter(|s| {
            s["is_built_in"].as_bool().unwrap_or(false)
                && s["transport_type"] == "http"
                && s["enabled"].as_bool().unwrap_or(false)
        })
        .collect();
    assert!(!builtins.is_empty(), "expected built-in servers to be registered");

    for s in builtins {
        let name = s["name"].as_str().unwrap_or("?");
        let resp = client
            .post(server.api_url("/mcp/system-servers/test-connection"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({
                "id": s["id"].as_str().expect("built-in id"),
                "transport_type": "http",
                "url": s["url"].as_str().expect("built-in url"),
                "timeout_seconds": 10,
            }))
            .send()
            .await
            .expect("test-connection request");
        assert_eq!(resp.status(), 200, "test-connection HTTP status for {name}");
        let body: Value = resp.json().await.expect("parse test-connection");
        assert_eq!(
            body["success"], true,
            "built-in '{name}' must still authenticate after a logout bumped the epoch \
             (the internal JWT must be minted with the CURRENT token_version); body: {body}"
        );
    }
}
