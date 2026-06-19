//! tool_result_mcp routes: a single JSON-RPC MCP endpoint.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn tool_result_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route` (multi-method,
        // not a typed REST endpoint).
        .route("/tool-result/mcp", post(handlers::jsonrpc_handler))
}
