// LLM Provider routes configuration

use aide::axum::{routing::{delete_with, get_with, post_with, put_with}, ApiRouter};

use super::handlers::*;

/// LLM Provider management routes
pub fn llm_provider_router() -> ApiRouter {
    ApiRouter::new()
        // Provider CRUD
        .api_route("/llm-providers", get_with(list_providers, list_providers_docs))
        .api_route("/llm-providers", post_with(create_provider, create_provider_docs))
        .api_route("/llm-providers/{provider_id}", get_with(get_provider, get_provider_docs))
        .api_route("/llm-providers/{provider_id}", post_with(update_provider, update_provider_docs))
        .api_route("/llm-providers/{provider_id}", delete_with(delete_provider, delete_provider_docs))
        // Group assignments (provider-centric - legacy/admin)
        .api_route("/llm-providers/{provider_id}/groups", get_with(get_provider_groups, get_provider_groups_docs))
        .api_route("/llm-providers/{provider_id}/groups", post_with(assign_provider_to_group, assign_provider_to_group_docs))
        .api_route("/llm-providers/{provider_id}/groups/{group_id}", delete_with(remove_provider_from_group, remove_provider_from_group_docs))
        // Group assignments (group-centric - for UI widgets)
        .api_route("/groups/{group_id}/providers", get_with(get_group_providers, get_group_providers_docs))
        .api_route("/groups/{group_id}/providers", put_with(update_group_providers, update_group_providers_docs))
}
