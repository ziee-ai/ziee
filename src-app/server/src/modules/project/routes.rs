// Project routes.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;

pub fn project_router() -> ApiRouter {
    ApiRouter::new()
        // CRUD
        .api_route("/projects", post_with(create_project, create_project_docs))
        .api_route("/projects", get_with(list_projects, list_projects_docs))
        .api_route(
            "/projects/{id}",
            get_with(get_project, get_project_docs),
        )
        .api_route(
            "/projects/{id}",
            put_with(update_project, update_project_docs),
        )
        .api_route(
            "/projects/{id}",
            delete_with(delete_project, delete_project_docs),
        )
        // Duplicate
        .api_route(
            "/projects/{id}/duplicate",
            post_with(duplicate_project, duplicate_project_docs),
        )
        // Files — relocated to the file module's project_extension
        // (project↔file inversion). The four `/api/projects/{id}/files*`
        // routes are now contributed via the PROJECT_EXTENSIONS slice;
        // `project/mod.rs::register_routes` merges them in.
        // Conversations
        .api_route(
            "/projects/{id}/conversations",
            get_with(list_project_conversations, list_project_conversations_docs),
        )
        .api_route(
            "/projects/{id}/conversations/{conversation_id}",
            post_with(attach_conversation, attach_conversation_docs),
        )
        .api_route(
            "/projects/{id}/conversations/{conversation_id}",
            delete_with(detach_conversation, detach_conversation_docs),
        )
        // Reverse lookup: "what project is this conversation in?"
        .api_route(
            "/projects/by-conversation/{conversation_id}",
            get_with(project_for_conversation, project_for_conversation_docs),
        )
        // MCP-settings routes (`GET/PUT /api/projects/{id}/mcp-settings`)
        // moved to mcp/project_extension/ — registered via
        // `ProjectExtension::register_routes` (project↔mcp inversion).
}
