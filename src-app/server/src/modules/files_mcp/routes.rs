//! files_mcp routes — single JSON-RPC endpoint at /api/files/mcp.

use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;

pub fn files_mcp_router() -> ApiRouter {
    ApiRouter::new().route("/files/mcp", post(handlers::jsonrpc_handler))
}
