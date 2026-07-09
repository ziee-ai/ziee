//! js_tool routes — single JSON-RPC endpoint at /api/run-js/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn js_tool_router() -> ApiRouter {
    ApiRouter::new()
        // Plain `route()` (not aide's `api_route`) — the JSON-RPC handler
        // dispatches multiple methods over one path and is not a typed REST
        // endpoint suitable for OpenAPI docs.
        .route("/run-js/mcp", post(handlers::jsonrpc_handler))
}
