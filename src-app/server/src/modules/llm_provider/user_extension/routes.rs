// Routes for the llm_provider↔user/Group bridge.
//
// Returned from `provider_group_routes()`; merged into the main
// `llm_provider_router()` via `.merge(...)`. URLs are preserved
// verbatim from the pre-inversion layout (the frontend autogen client
// methods depend on the OpenAPI `.id()` strings in `handlers.rs`, but
// also on the route shapes here).

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;

pub fn provider_group_routes() -> ApiRouter {
    ApiRouter::new()
        // Provider-centric (admin)
        .api_route(
            "/llm-providers/{provider_id}/groups",
            get_with(get_provider_groups, get_provider_groups_docs),
        )
        .api_route(
            "/llm-providers/{provider_id}/groups",
            post_with(assign_provider_to_group, assign_provider_to_group_docs),
        )
        .api_route(
            "/llm-providers/{provider_id}/groups/{group_id}",
            delete_with(remove_provider_from_group, remove_provider_from_group_docs),
        )
        // Group-centric (for UI widgets)
        .api_route(
            "/groups/{group_id}/providers",
            get_with(get_group_providers, get_group_providers_docs),
        )
        .api_route(
            "/groups/{group_id}/providers",
            put_with(update_group_providers, update_group_providers_docs),
        )
}
