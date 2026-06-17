//! bio_mcp routes — the streamable-HTTP MCP proxy at /api/bio/mcp.

use aide::axum::ApiRouter;
use axum::routing::{delete, get, post};

use super::handlers;

pub fn bio_mcp_router() -> ApiRouter {
    // Plain axum routes (not aide's `api_route`) — this is an opaque MCP
    // streamable-HTTP endpoint, not a typed REST endpoint. POST carries
    // the JSON-RPC messages; GET opens the standalone SSE stream; DELETE
    // tears a session down. All three forward through the same proxy.
    ApiRouter::new().route(
        "/bio/mcp",
        post(handlers::proxy_handler)
            .get(handlers::proxy_handler)
            .delete(handlers::proxy_handler),
    )
}
