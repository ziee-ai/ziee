// Assistant routes configuration - separate routes for user and template assistants

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;

/// Assistant management routes
pub fn assistant_router() -> ApiRouter {
    ApiRouter::new()
        // User assistant routes (/assistants)
        .api_route(
            "/assistants",
            post_with(create_user_assistant, create_user_assistant_docs),
        )
        .api_route(
            "/assistants",
            get_with(list_user_assistants, list_user_assistants_docs),
        )
        .api_route(
            "/assistants/default",
            get_with(get_default_user_assistant, get_default_user_assistant_docs),
        )
        .api_route(
            "/assistants/{id}",
            get_with(get_user_assistant, get_user_assistant_docs),
        )
        .api_route(
            "/assistants/{id}",
            put_with(update_user_assistant, update_user_assistant_docs),
        )
        .api_route(
            "/assistants/{id}",
            delete_with(delete_user_assistant, delete_user_assistant_docs),
        )
        // Template assistant routes (/assistant-templates)
        .api_route(
            "/assistant-templates",
            post_with(create_template_assistant, create_template_assistant_docs),
        )
        .api_route(
            "/assistant-templates",
            get_with(list_template_assistants, list_template_assistants_docs),
        )
        .api_route(
            "/assistant-templates/default",
            get_with(
                get_default_template_assistant,
                get_default_template_assistant_docs,
            ),
        )
        .api_route(
            "/assistant-templates/{id}",
            get_with(get_template_assistant, get_template_assistant_docs),
        )
        .api_route(
            "/assistant-templates/{id}",
            put_with(update_template_assistant, update_template_assistant_docs),
        )
        .api_route(
            "/assistant-templates/{id}",
            delete_with(delete_template_assistant, delete_template_assistant_docs),
        )
}
