use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};

use super::handlers::*;

pub fn hub_router() -> ApiRouter {
    ApiRouter::new()
        // Models endpoints
        .api_route("/hub/models", get_with(get_hub_models, get_hub_models_docs))
        .api_route(
            "/hub/models/version",
            get_with(get_hub_models_version, get_hub_models_version_docs),
        )
        .api_route(
            "/hub/models/refresh",
            post_with(refresh_hub_models, refresh_hub_models_docs),
        )
        // Assistants endpoints
        .api_route(
            "/hub/assistants",
            get_with(get_hub_assistants, get_hub_assistants_docs),
        )
        .api_route(
            "/hub/assistants/version",
            get_with(get_hub_assistants_version, get_hub_assistants_version_docs),
        )
        .api_route(
            "/hub/assistants/refresh",
            post_with(refresh_hub_assistants, refresh_hub_assistants_docs),
        )
        // MCP servers endpoints
        .api_route(
            "/hub/mcp-servers",
            get_with(get_hub_mcp_servers, get_hub_mcp_servers_docs),
        )
        .api_route(
            "/hub/mcp-servers/version",
            get_with(
                get_hub_mcp_servers_version,
                get_hub_mcp_servers_version_docs,
            ),
        )
        .api_route(
            "/hub/mcp-servers/refresh",
            post_with(refresh_hub_mcp_servers, refresh_hub_mcp_servers_docs),
        )
        // Hub entity creation endpoints
        .api_route(
            "/hub/assistants/create",
            post_with(create_assistant_from_hub, create_assistant_from_hub_docs),
        )
        .api_route(
            "/hub/mcp-servers/create",
            post_with(create_mcp_server_from_hub, create_mcp_server_from_hub_docs),
        )
        .api_route(
            "/hub/models/download",
            post_with(create_model_from_hub, create_model_from_hub_docs),
        )
}
