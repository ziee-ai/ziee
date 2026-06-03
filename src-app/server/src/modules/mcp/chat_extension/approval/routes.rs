//! MCP approval workflow routes

use aide::axum::{routing::{get_with, put_with}, ApiRouter};

use super::handlers::*;

/// MCP approval routes
pub fn mcp_approval_router() -> ApiRouter {
    ApiRouter::new()
        // MCP settings
        .api_route(
            "/conversations/{id}/mcp-settings",
            get_with(get_mcp_settings, get_mcp_settings_docs),
        )
        .api_route(
            "/conversations/{id}/mcp-settings",
            put_with(update_mcp_settings, update_mcp_settings_docs),
        )
        // Pending approvals
        .api_route(
            "/branches/{branch_id}/pending-approvals",
            get_with(get_pending_approvals_for_branch, get_pending_approvals_for_branch_docs),
        )
}
