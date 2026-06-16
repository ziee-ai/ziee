//! elicitation_mcp routes — single JSON-RPC endpoint at /api/elicitation/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn elicitation_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // Plain Axum `route()` (not aide's `api_route`): the JSON-RPC handler
        // dispatches multiple methods over one path and isn't a typed REST
        // endpoint suitable for OpenAPI docs (mirrors memory_mcp/files_mcp).
        .route("/elicitation/mcp", post(handlers::jsonrpc_handler))
}
