// MCP routes configuration
// Defines API routes for MCP server management

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;

// =====================================================
// User Routes
// =====================================================

pub fn user_routes() -> ApiRouter {
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
        // Runtime operations
        .api_route(
            "/mcp/servers/{id}/tools",
            get_with(runtime::list_server_tools, runtime::list_server_tools_docs),
        )
        .api_route(
            "/mcp/servers/{id}/tools/{name}/call",
            post_with(runtime::call_server_tool, runtime::call_server_tool_docs),
        )
        .api_route(
            "/mcp/servers/{id}/resources",
            get_with(runtime::list_server_resources, runtime::list_server_resources_docs),
        )
        .api_route(
            "/mcp/servers/{id}/resources/read",
            post_with(runtime::read_server_resource, runtime::read_server_resource_docs),
        )
        .api_route(
            "/mcp/servers/{id}/disconnect",
            delete_with(runtime::disconnect_server, runtime::disconnect_server_docs),
        )
        // Prompts (MCP spec § server/prompts)
        .api_route(
            "/mcp/servers/{id}/prompts",
            get_with(runtime::list_server_prompts, runtime::list_server_prompts_docs),
        )
        .api_route(
            "/mcp/servers/{id}/prompts/get",
            post_with(runtime::get_server_prompt, runtime::get_server_prompt_docs),
        )
        // Ping (MCP spec § utilities/ping)
        .api_route(
            "/mcp/servers/{id}/ping",
            post_with(runtime::ping_server, runtime::ping_server_docs),
        )
}

// =====================================================
// Admin Routes
// =====================================================

pub fn admin_routes() -> ApiRouter {
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
            "/mcp/system-servers/{id}/groups",
            get_with(get_server_groups, get_server_groups_docs),
        )
        .api_route(
            "/mcp/system-servers/{id}/groups",
            post_with(assign_server_to_groups, assign_server_to_groups_docs),
        )
        .api_route(
            "/mcp/system-servers/{id}/groups/{group_id}",
            delete_with(remove_server_from_group, remove_server_from_group_docs),
        )
        // Group-centric endpoints (for UI widgets)
        .api_route(
            "/groups/{group_id}/system-servers",
            get_with(get_group_system_servers, get_group_system_servers_docs),
        )
        .api_route(
            "/groups/{group_id}/system-servers",
            put_with(
                update_group_system_servers,
                update_group_system_servers_docs,
            ),
        )
}
