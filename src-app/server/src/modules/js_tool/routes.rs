//! js_tool routes — the JSON-RPC endpoint at /api/run-js/mcp + the admin
//! settings REST endpoints at /api/js-tool/settings.

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use axum::routing::post;

use super::handlers;

pub fn js_tool_router() -> ApiRouter {
    ApiRouter::new()
        // Plain `route()` (not aide's `api_route`) — the JSON-RPC handler
        // dispatches multiple methods over one path and is not a typed REST
        // endpoint suitable for OpenAPI docs.
        .route("/run-js/mcp", post(handlers::jsonrpc_handler))
        // Typed REST: admin-configurable limits (mirrors code_sandbox).
        .api_route(
            "/js-tool/settings",
            get_with(handlers::get_settings_handler, handlers::get_settings_docs)
                .put_with(handlers::update_settings_handler, handlers::update_settings_docs),
        )
}
