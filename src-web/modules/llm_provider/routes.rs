// LLM Provider routes configuration

use aide::axum::{routing::{delete_with, get_with, post_with}, ApiRouter};
use sqlx::PgPool;

use super::handlers::*;

/// LLM Provider management routes
pub fn llm_provider_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        // Provider CRUD
        .api_route("/llm-providers", get_with(list_providers, list_providers_docs))
        .api_route("/llm-providers", post_with(create_provider, create_provider_docs))
        .api_route("/llm-providers/{provider_id}", get_with(get_provider, get_provider_docs))
        .api_route("/llm-providers/{provider_id}", post_with(update_provider, update_provider_docs))
        .api_route("/llm-providers/{provider_id}", delete_with(delete_provider, delete_provider_docs))
        // Group assignments
        .api_route("/llm-providers/{provider_id}/groups", get_with(get_provider_groups, get_provider_groups_docs))
        .api_route("/llm-providers/assign-group", post_with(assign_provider_to_group, assign_provider_to_group_docs))
        .api_route("/llm-providers/{provider_id}/{group_id}/remove-group", delete_with(remove_provider_from_group, remove_provider_from_group_docs))
}
