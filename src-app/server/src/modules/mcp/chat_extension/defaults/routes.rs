//! User MCP defaults routes

use aide::axum::{routing::{get_with, put_with}, ApiRouter};

use super::handlers::*;

/// MCP defaults routes
pub fn mcp_defaults_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/mcp/defaults",
            get_with(get_mcp_defaults, get_mcp_defaults_docs),
        )
        .api_route(
            "/mcp/defaults",
            put_with(update_mcp_defaults, update_mcp_defaults_docs),
        )
}
