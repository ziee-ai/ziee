// Routes for the project↔chat relationship. Returned from
// `ProjectExtension::register_routes` so chat's auto-registration merges
// them in via the CHAT_EXTENSIONS slice without project needing to
// import chat's router.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with},
};

use super::handlers::*;

pub fn project_conversation_routes() -> ApiRouter {
    ApiRouter::new()
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
}
