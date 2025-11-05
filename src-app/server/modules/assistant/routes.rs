// Assistant routes configuration - separate routes for user and template assistants

use aide::axum::{routing::{delete_with, get_with, post_with, put_with}, ApiRouter};
use sqlx::PgPool;

use super::handlers::*;

/// Assistant management routes
pub fn assistant_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        // User assistant routes (/assistants)
        .api_route("/assistants", post_with(create_user_assistant, create_user_assistant_docs))
        .api_route("/assistants", get_with(list_user_assistants, list_user_assistants_docs))
        .api_route("/assistants/default", get_with(get_default_user_assistant, get_default_user_assistant_docs))
        .api_route("/assistants/{id}", get_with(get_user_assistant, get_user_assistant_docs))
        .api_route("/assistants/{id}", put_with(update_user_assistant, update_user_assistant_docs))
        .api_route("/assistants/{id}", delete_with(delete_user_assistant, delete_user_assistant_docs))

        // Template assistant routes (/assistants-template)
        .api_route("/assistants-template", post_with(create_template_assistant, create_template_assistant_docs))
        .api_route("/assistants-template", get_with(list_template_assistants, list_template_assistants_docs))
        .api_route("/assistants-template/default", get_with(get_default_template_assistant, get_default_template_assistant_docs))
        .api_route("/assistants-template/{id}", get_with(get_template_assistant, get_template_assistant_docs))
        .api_route("/assistants-template/{id}", put_with(update_template_assistant, update_template_assistant_docs))
        .api_route("/assistants-template/{id}", delete_with(delete_template_assistant, delete_template_assistant_docs))
}
