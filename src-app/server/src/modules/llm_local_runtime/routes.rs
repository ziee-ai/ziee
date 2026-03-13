// Local LLM Runtime routes configuration

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with},
};

use super::handlers::*;
use super::runtime_version::handlers as version_handlers;

/// Local LLM Runtime management routes
pub fn llm_local_runtime_router() -> ApiRouter {
    ApiRouter::new()
        // Model instance management (primary endpoints)
        .api_route(
            "/local-runtime/models/{model_id}/start",
            post_with(start_model_instance, start_model_instance_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/stop",
            post_with(stop_model_instance, stop_model_instance_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/restart",
            post_with(restart_model_instance, restart_model_instance_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/instance",
            get_with(get_model_instance, get_model_instance_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/status",
            get_with(get_model_status, get_model_status_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/health",
            get_with(get_model_health, get_model_health_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/logs",
            get_with(get_model_logs, get_model_logs_docs),
        )
        // Provider-level instance queries
        .api_route(
            "/local-runtime/providers/{provider_id}/instances",
            get_with(get_provider_instances, get_provider_instances_docs),
        )
        // Runtime version management routes
        .api_route(
            "/local-runtime/versions",
            get_with(version_handlers::list_runtime_versions, version_handlers::list_runtime_versions_docs),
        )
        .api_route(
            "/local-runtime/versions/{version_id}",
            get_with(version_handlers::get_runtime_version, version_handlers::get_runtime_version_docs),
        )
        .api_route(
            "/local-runtime/versions/download",
            post_with(version_handlers::download_runtime_version, version_handlers::download_runtime_version_docs),
        )
        .api_route(
            "/local-runtime/versions/{version_id}",
            delete_with(version_handlers::delete_runtime_version, version_handlers::delete_runtime_version_docs),
        )
        .api_route(
            "/local-runtime/versions/{version_id}/set-default",
            post_with(version_handlers::set_system_default, version_handlers::set_system_default_docs),
        )
        .api_route(
            "/local-runtime/versions/{engine}/check-updates",
            get_with(version_handlers::check_for_updates, version_handlers::check_for_updates_docs),
        )
        .api_route(
            "/local-runtime/versions/sync-cache",
            post_with(version_handlers::sync_cache, version_handlers::sync_cache_docs),
        )
}
