// MCP routes configuration
// Defines API routes for MCP server management

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;
use super::tool_calls::handlers as tool_call_handlers;
use super::user_policy::handlers as user_policy_handlers;

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
        // OAuth client_credentials config (Phase 4)
        .api_route(
            "/mcp/servers/{id}/oauth",
            get_with(get_server_oauth_config, get_server_oauth_config_docs),
        )
        .api_route(
            "/mcp/servers/{id}/oauth",
            put_with(set_server_oauth_config, set_server_oauth_config_docs),
        )
        .api_route(
            "/mcp/servers/{id}/oauth",
            delete_with(delete_server_oauth_config, delete_server_oauth_config_docs),
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
        // Tool-call history (mcp_tool_calls): the caller's own invocations.
        .api_route(
            "/mcp/tool-calls",
            get_with(
                tool_call_handlers::list_tool_calls,
                tool_call_handlers::list_tool_calls_docs,
            ),
        )
        .api_route(
            "/mcp/tool-calls/{id}",
            get_with(
                tool_call_handlers::get_tool_call,
                tool_call_handlers::get_tool_call_docs,
            ),
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
        // Connection test — probe a candidate config without persisting it
        .api_route(
            "/mcp/servers/test-connection",
            post_with(
                test_connection::test_user_connection,
                test_connection::test_user_connection_docs,
            ),
        )
        // User-policy read (any user with mcp_servers::read; needed by
        // the UI to gate the Add button + Hub MCP tab visibility).
        .api_route(
            "/mcp/user-policy",
            get_with(
                user_policy_handlers::get_user_policy,
                user_policy_handlers::get_user_policy_docs,
            ),
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
        // Connection test — probe a candidate system-server config (no persist)
        .api_route(
            "/mcp/system-servers/test-connection",
            post_with(
                test_connection::test_system_connection,
                test_connection::test_system_connection_docs,
            ),
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
        // User-policy write (admin only — perm McpUserPolicyEdit).
        .api_route(
            "/mcp/user-policy",
            put_with(
                user_policy_handlers::update_user_policy,
                user_policy_handlers::update_user_policy_docs,
            ),
        )
        // Admin per-(server, tool) approval-mode defaults (ITEM-54 / DEC-112).
        // Path uses the `/mcp/servers/{id}` shape per spec; both handlers gate
        // on the system-MCP admin perms and 404 on a foreign / non-system id.
        .api_route(
            "/mcp/servers/{id}/tool-approvals",
            get_with(
                tool_approvals::get_server_tool_approvals,
                tool_approvals::get_server_tool_approvals_docs,
            ),
        )
        .api_route(
            "/mcp/servers/{id}/tool-approvals/{tool}",
            put_with(
                tool_approvals::set_server_tool_approval,
                tool_approvals::set_server_tool_approval_docs,
            ),
        )
}
