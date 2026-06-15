//! workflow_mcp routes — single JSON-RPC endpoint at /api/workflows/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn workflow_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // Like skill_mcp / memory_mcp: bypass aide's `api_route` because
        // the JSON-RPC handler dispatches multiple methods over the same
        // path and isn't a typed REST endpoint suitable for OpenAPI docs.
        .route("/workflows/mcp", post(handlers::jsonrpc_handler))
}
