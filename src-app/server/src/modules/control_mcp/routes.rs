//! control_mcp routes — single JSON-RPC endpoint at /api/control/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn control_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // Via Axum's `route()` (not aide's `api_route`) — the JSON-RPC handler
        // dispatches multiple methods over one path and isn't a typed REST
        // endpoint suitable for OpenAPI docs. Mirrors files_mcp / web_search.
        .route("/control/mcp", post(handlers::jsonrpc_handler))
}
