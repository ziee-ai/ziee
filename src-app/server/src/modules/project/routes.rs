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
        // Files
        .api_route(
            "/projects/{id}/files",
            get_with(list_project_files, list_project_files_docs),
        )
        .api_route(
            "/projects/{id}/files",
            post_with(attach_file, attach_file_docs),
        )
        .api_route(
            "/projects/{id}/files/upload",
            post_with(upload_and_attach_file, upload_and_attach_file_docs),
        )
        .api_route(
            "/projects/{id}/files/{file_id}",
            delete_with(detach_file, detach_file_docs),
        )
        // Conversations
        .api_route(
            "/projects/{id}/conversations",
            get_with(list_project_conversations, list_project_conversations_docs),
        )
        // MCP settings (subset for the settings drawer)
        .api_route(
            "/projects/{id}/mcp-settings",
            get_with(get_project_mcp_settings, get_project_mcp_settings_docs),
        )
        .api_route(
            "/projects/{id}/mcp-settings",
            put_with(update_project_mcp_settings, update_project_mcp_settings_docs),
        )
}
