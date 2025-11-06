// MCP routes configuration
// Defines API routes for MCP server management

use aide::axum::{
    routing::{delete_with, get_with, post_with, put_with},
    ApiRouter,
};
use sqlx::PgPool;

use super::handlers::*;

// =====================================================
// User Routes
// =====================================================

pub fn user_routes() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route(
            "/mcp/servers",
            get_with(list_accessible_servers, list_accessible_servers_docs),
        )
        .api_route(
            "/mcp/servers",
            post_with(create_user_server, create_user_server_docs),
        )
        .api_route(
            "/mcp/servers/{id}",
            get_with(get_user_server, get_user_server_docs),
        )
        .api_route(
            "/mcp/servers/{id}",
            put_with(update_user_server, update_user_server_docs),
        )
        .api_route(
            "/mcp/servers/{id}",
            delete_with(delete_user_server, delete_user_server_docs),
        )
}

// =====================================================
// Admin Routes
// =====================================================

pub fn admin_routes() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route(
            "/mcp/system-servers",
            get_with(list_system_servers, list_system_servers_docs),
        )
        .api_route(
            "/mcp/system-servers",
            post_with(create_system_server, create_system_server_docs),
        )
        .api_route(
            "/mcp/system-servers/{id}",
            get_with(get_system_server, get_system_server_docs),
        )
        .api_route(
            "/mcp/system-servers/{id}",
            put_with(update_system_server, update_system_server_docs),
        )
        .api_route(
            "/mcp/system-servers/{id}",
            delete_with(delete_system_server, delete_system_server_docs),
        )
        .api_route(
            "/mcp/groups/{group_id}/servers",
            get_with(get_group_servers, get_group_servers_docs),
        )
        .api_route(
            "/mcp/groups/{group_id}/servers",
            put_with(set_group_servers, set_group_servers_docs),
        )
        .api_route(
            "/mcp/groups/{group_id}/servers/{server_id}",
            delete_with(remove_group_server, remove_group_server_docs),
        )
}
