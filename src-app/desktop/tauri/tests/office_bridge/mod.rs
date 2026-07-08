//! Integration tests for the `office_bridge` module.
//!
//! - `settings_mcp_test` (TEST-2 / TEST-3 / TEST-5) — the JSON-RPC MCP endpoint
//!   (initialize + tools/list, permission-gated) and the admin settings REST
//!   surface (singleton defaults proving migrations 132/133, authz gating, and
//!   the no-secret-in-body guarantee), against the `TestServer` harness.
//! - `bridge_test` (TEST-7) exercises the standalone HTTPS + WSS bridge listener
//!   (ITEM-5) end-to-end over real TLS, trusting the minted bridge cert.
//! - `windows_com_test` (TEST-9, `#[cfg(windows)]` + `#[ignore]`) is the live
//!   Windows COM enumeration + act-on-document test (ITEM-7); it is opt-in and
//!   requires a real, non-elevated Office document open on this session.

use serde_json::{Value, json};

mod bridge_test;
mod settings_mcp_test;
mod migrations_test;
mod attach_test;
#[cfg(windows)]
mod windows_com_test;

/// Build a JSON-RPC request to the office_bridge MCP endpoint
/// (`POST /api/office-bridge/mcp`). Mirrors the web_search `jsonrpc` helper.
pub fn jsonrpc(
    server: &crate::common::TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/office-bridge/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}
