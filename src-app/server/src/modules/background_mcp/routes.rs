//! background_mcp routes — single JSON-RPC endpoint at /api/background/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn background_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // Like workflow_mcp / memory_mcp: bypass aide's `api_route` because the
        // JSON-RPC handler dispatches multiple methods over the same path and
        // isn't a typed REST endpoint suitable for OpenAPI docs. Both POST (the
        // MCP call channel) and GET (streamable-HTTP session open) hit the same
        // handler — GET with no body is treated as a no-op accept.
        .route(
            "/background/mcp",
            post(handlers::jsonrpc_handler).get(handlers::jsonrpc_handler_get),
        )
}
