//! The deploy-level kill switches must UNMOUNT the MCP JSON-RPC endpoints —
//! and must NOT unmount the settings/admin REST beside them.
//!
//! Before the guard these three modules skipped only the `mcp_servers` upsert
//! when their switch was off, while `register_routes` merged their router
//! unconditionally. The JSON-RPC endpoints therefore stayed MOUNTED, gated
//! solely on `*::use` permissions the Users group holds by migration, with the
//! runtime settings rows defaulting enabled. So "disabled" meant
//! "unadvertised".
//!
//! **The severity differs by module, and is stated precisely rather than
//! flattened.** `web_search` and `lit_search` DISPATCH `tools/call` over this
//! transport, so a disabled deployment still ran live web and scholarly queries
//! for any Users-group member — exactly the egress the switch exists to stop,
//! and for `lit_search` five of six connectors need no key. `js_tool` does NOT:
//! its `tools/call` arm refuses, because `run_js` is executed inline by the chat
//! runtime, not over loopback. What it leaked was the SURFACE — `initialize` and
//! `tools/list` for a switched-off feature — not code execution. All three are
//! worth closing; only two were egress.
//!
//! Each test asserts BOTH halves of the contract, because either alone is
//! satisfiable by the wrong implementation:
//!
//!   * the JSON-RPC route is **404** — an unmatched route, not the 401/403 a
//!     mounted-but-auth-gated route returns. Asserting merely "not 200" would
//!     pass against the ORIGINAL bug, where the route was mounted and answered
//!     403 to a permissionless caller.
//!   * the settings route is **NOT 404** — the deliberate split. Asserting only
//!     the 404 above would pass against a guard that unmounted the whole
//!     router, which is over-reach: those routes only read and write
//!     configuration and their admin page should keep working on a deployment
//!     that turned the feature off.
//!
//! ⚠ KNOWN INCONSISTENCY, recorded rather than papered over: the tree now holds
//! two OPPOSITE kill-switch contracts. These three modules keep their settings
//! REST mounted when disabled; `voice` unmounts its whole surface, and
//! `tests/voice/config_gate_test.rs` asserts `/voice/settings` MUST be 404. Both
//! files are green simultaneously while specifying contradictory behaviour for
//! the same class of switch. This branch does NOT unify them — picking one
//! policy is a product decision, and changing `voice` here would be exactly the
//! out-of-scope drift this port must avoid. Flagged so whoever unifies them
//! knows both tests exist and one will have to move.
//!
//! On the caller: `create_user_with_permissions(.., &[])` registers through
//! `/auth/register`, which places the user in the DEFAULT GROUP — so it is not a
//! permissionless caller at all, it holds exactly the `*::use` grants named
//! above. That is what makes the 404 assertions meaningful: this user WOULD be
//! served by a mounted route (200), so a 404 can only mean the route is absent.
//! Do not "tighten" these to `assert_ne!(status, 403)` on the assumption of a
//! permissionless caller — that premise is false here.

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

/// A minimal, well-formed JSON-RPC body. The request must be shaped well enough
/// that a MOUNTED endpoint would get past deserialization — otherwise a 404
/// could be confused with a rejected body.
fn jsonrpc_initialize() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "kill-switch-gate-test", "version": "0.0.0" }
        }
    })
}

async fn assert_gated(
    server: &TestServer,
    user_token: &str,
    mcp_path: &str,
    settings_path: &str,
) {
    let client = reqwest::Client::new();

    let rpc = client
        .post(server.api_url(mcp_path))
        .header("Authorization", format!("Bearer {user_token}"))
        .json(&jsonrpc_initialize())
        .send()
        .await
        .expect("request must complete");
    assert_eq!(
        rpc.status(),
        404,
        "{mcp_path} must be UNMOUNTED when the kill switch is off. This caller is \
         in the default group and holds the module's `use` grant, so a mounted \
         route would SERVE it (200) — which is the original defect. Any non-404 \
         here means the route is still there"
    );

    let settings = client
        .get(server.api_url(settings_path))
        .header("Authorization", format!("Bearer {user_token}"))
        .send()
        .await
        .expect("request must complete");
    assert_ne!(
        settings.status(),
        404,
        "{settings_path} must STAY MOUNTED when the kill switch is off — it only \
         reads and writes configuration, and its admin page should keep working \
         on a disabled deployment. A 404 here means the guard unmounted the whole \
         router instead of just the JSON-RPC endpoint"
    );
}

#[tokio::test]
async fn js_tool_disabled_unmounts_run_js_but_keeps_settings() {
    let server = TestServer::start_with_options(TestServerOptions {
        js_tool_enabled: Some(false),
        ..Default::default()
    })
    .await;

    let health = reqwest::Client::new()
        .get(format!("{}/api/health", server.base_url))
        .send()
        .await
        .unwrap();
    assert!(health.status().is_success(), "server should still boot");

    let user = create_user_with_permissions(&server, "js_tool_gate_user", &[]).await;
    assert_gated(&server, &user.token, "/run-js/mcp", "/js-tool/settings").await;
}

#[tokio::test]
async fn web_search_disabled_unmounts_mcp_but_keeps_settings() {
    let server = TestServer::start_with_options(TestServerOptions {
        web_search_enabled: Some(false),
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(&server, "web_search_gate_user", &[]).await;
    assert_gated(
        &server,
        &user.token,
        "/web-search/mcp",
        "/web-search/settings",
    )
    .await;
}

#[tokio::test]
async fn lit_search_disabled_unmounts_mcp_but_keeps_settings() {
    let server = TestServer::start_with_options(TestServerOptions {
        lit_search_enabled: Some(false),
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(&server, "lit_search_gate_user", &[]).await;
    assert_gated(
        &server,
        &user.token,
        "/lit-search/mcp",
        "/lit-search/settings",
    )
    .await;
}

/// POSITIVE CONTROL — with the switch at its DEFAULT (absent config section, i.e.
/// enabled), the JSON-RPC endpoints are MOUNTED.
///
/// Without this, every assertion above passes vacuously against a build in which
/// the routes never existed, or in which some unrelated change unmounted them
/// permanently. It is also the assertion that pins the promise this port makes
/// to existing deployments: adding the guard changes NOTHING for anyone who has
/// not set the switch.
#[tokio::test]
async fn defaults_leave_every_mcp_endpoint_mounted() {
    let server = TestServer::start_with_options(TestServerOptions::default()).await;
    let user = create_user_with_permissions(&server, "kill_switch_control_user", &[]).await;
    let client = reqwest::Client::new();

    for path in ["/run-js/mcp", "/web-search/mcp", "/lit-search/mcp"] {
        let resp = client
            .post(server.api_url(path))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&jsonrpc_initialize())
            .send()
            .await
            .expect("request must complete");
        assert_ne!(
            resp.status(),
            404,
            "{path} must be MOUNTED by default (an absent config section means \
             enabled) — a 404 here would mean this change silently disabled a \
             feature for every deployment that never set the switch"
        );
    }
}
