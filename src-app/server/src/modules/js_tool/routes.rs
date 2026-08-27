//! js_tool routes — the JSON-RPC endpoint at /api/run-js/mcp + the admin
//! settings REST endpoints at /api/js-tool/settings.

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use axum::routing::post;

use super::handlers;

/// The admin settings REST — mounted regardless of the kill switch.
///
/// Split from the JSON-RPC endpoint deliberately: this route only reads and
/// writes configuration, it executes nothing, and the admin page that drives it
/// should keep working (and keep showing the feature as off) on a deployment
/// that has turned run_js off. Mirrors web_search / lit_search.
pub fn js_tool_router() -> ApiRouter {
    ApiRouter::new()
        // Typed REST: admin-configurable limits (mirrors code_sandbox).
        .api_route(
            "/js-tool/settings",
            get_with(handlers::get_settings_handler, handlers::get_settings_docs)
                .put_with(handlers::update_settings_handler, handlers::update_settings_docs),
        )
}

/// The MCP JSON-RPC endpoint — mounted ONLY when the kill switch is on.
///
/// NOTE this endpoint does NOT execute: its `tools/call` arm refuses, because
/// `run_js` is invoked inline by the chat runtime rather than over the loopback
/// transport (see `handlers.rs`). It serves `initialize` / `tools/list` / `ping`.
///
/// It is still gated, because leaving it mounted made the switch merely stop the
/// tools being ADVERTISED TO THE MODEL while the endpoint stayed reachable by
/// any Users-group member (`js_tool::use` is granted by migration
/// 202607146040) — so a "disabled" deployment still served the feature's
/// surface. That is the inconsistency being closed here, and it is a smaller
/// claim than the sibling modules': `web_search` and `lit_search` DO dispatch
/// `tools/call` over this transport, so for them the same hole is live egress.
///
/// Plain `route()` (not aide's `api_route`) — the JSON-RPC handler dispatches
/// multiple methods over one path and is not a typed REST endpoint suitable for
/// OpenAPI docs.
pub fn js_tool_mcp_router() -> ApiRouter {
    ApiRouter::new().route("/run-js/mcp", post(handlers::jsonrpc_handler))
}
