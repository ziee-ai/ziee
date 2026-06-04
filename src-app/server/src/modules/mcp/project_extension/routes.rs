// Routes for the project↔mcp relationship, mounted at
// `/api/projects/{id}/mcp-settings`. Returned from
// `McpProjectExtension::register_routes` so the project module merges
// them in via the PROJECT_EXTENSIONS slice without importing mcp.

use aide::axum::{
    ApiRouter,
    routing::{get_with, put_with},
};

use super::handlers::*;

pub fn project_mcp_settings_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/projects/{id}/mcp-settings",
            get_with(get_project_mcp_settings, get_project_mcp_settings_docs),
        )
        .api_route(
            "/projects/{id}/mcp-settings",
            put_with(update_project_mcp_settings, update_project_mcp_settings_docs),
        )
}
