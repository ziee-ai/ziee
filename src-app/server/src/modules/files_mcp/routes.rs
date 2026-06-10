//! files_mcp routes — single JSON-RPC endpoint at /api/files/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn files_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // Route via Axum's `route()` (not aide's `api_route`) — the
        // JSON-RPC handler dispatches multiple methods over the same
        // path and isn't a typed REST endpoint suitable for OpenAPI
        // docs.
        .route("/files/mcp", post(handlers::jsonrpc_handler))
}
