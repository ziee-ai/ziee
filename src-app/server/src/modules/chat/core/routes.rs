// Chat routes configuration

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;

/// Chat conversation management routes
pub fn chat_router() -> ApiRouter {
    ApiRouter::new()
        // Conversation CRUD
        .api_route(
            "/conversations",
            post_with(create_conversation, create_conversation_docs),
        )
        .api_route(
            "/conversations",
            get_with(list_conversations, list_conversations_docs),
        )
        .api_route(
            "/conversations/{id}",
            get_with(get_conversation, get_conversation_docs),
        )
        .api_route(
            "/conversations/{id}",
            put_with(update_conversation, update_conversation_docs),
        )
        .api_route(
            "/conversations/{id}",
            delete_with(delete_conversation, delete_conversation_docs),
        )
        // Message operations
        .api_route(
            "/conversations/{id}/messages",
            get_with(get_conversation_history, get_conversation_history_docs),
        )
        .api_route(
            "/conversations/{id}/messages/stream",
            post_with(send_message, send_message_docs),
        )
        .api_route("/messages/{id}", get_with(get_message, get_message_docs))
        .api_route(
            "/conversations/{conversation_id}/messages/{message_id}",
            put_with(edit_message, edit_message_docs),
        )
        .api_route(
            "/messages/{id}",
            delete_with(delete_message, delete_message_docs),
        )
        // Branch operations
        .api_route(
            "/conversations/{id}/branches",
            post_with(create_branch, create_branch_docs),
        )
        .api_route(
            "/conversations/{id}/branches",
            get_with(list_branches, list_branches_docs),
        )
        .api_route(
            "/conversations/{id}/branches/{branch_id}/activate",
            post_with(activate_branch, activate_branch_docs),
        )
        // LLM Provider access
        .api_route(
            "/chat/llm-providers",
            get_with(get_user_llm_providers, get_user_llm_providers_docs),
        )
}
