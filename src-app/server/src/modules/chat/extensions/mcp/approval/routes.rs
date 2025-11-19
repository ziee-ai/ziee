//! MCP approval workflow routes

use aide::axum::{routing::{get_with, post_with, put_with}, ApiRouter};

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
        // Tool approvals
        .api_route(
            "/conversations/{conversation_id}/messages/{message_id}/pending-approvals",
            get_with(get_pending_approvals, get_pending_approvals_docs),
        )
        .api_route(
            "/conversations/{conversation_id}/messages/{message_id}/approve-tools",
            post_with(approve_tools, approve_tools_docs),
        )
}
