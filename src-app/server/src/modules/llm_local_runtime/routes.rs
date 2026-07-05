// Local LLM Runtime routes configuration

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with, put_with},
};

use super::handlers::*;
use super::proxy_router::proxy_router;
use super::runtime_settings::handlers as settings_handlers;
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
            "/local-runtime/models/{model_id}/clear-failed",
            post_with(clear_failed_instance, clear_failed_instance_docs),
        )
        .api_route(
            "/local-runtime/models/{model_id}/runtime-version",
            post_with(swap_model_runtime_version, swap_model_runtime_version_docs),
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
        // P2: SSE log streaming
        .api_route(
            "/local-runtime/models/{model_id}/logs/stream",
            get_with(stream_model_logs, stream_model_logs_docs),
        )
        // Provider-level instance queries
        .api_route(
            "/local-runtime/providers/{provider_id}/instances",
            get_with(get_provider_instances, get_provider_instances_docs),
        )
        // P3: GPU detection (powers the settings-page card)
        .api_route(
            "/local-runtime/detect-gpu",
            get_with(detect_gpu, detect_gpu_docs),
        )
        // Runtime version management routes
        .api_route(
            "/local-runtime/versions",
            get_with(version_handlers::list_runtime_versions, version_handlers::list_runtime_versions_docs),
        )
        // Models grouped by the engine version they effectively use. Kept off
        // the `/versions/{version_id}` path to avoid a static-vs-param router
        // conflict.
        .api_route(
            "/local-runtime/version-usage",
            get_with(version_handlers::list_version_usage, version_handlers::list_version_usage_docs),
        )
        .api_route(
            "/local-runtime/versions/{version_id}",
            get_with(version_handlers::get_runtime_version, version_handlers::get_runtime_version_docs),
        )
        .api_route(
            "/local-runtime/versions/download",
            post_with(version_handlers::download_runtime_version, version_handlers::download_runtime_version_docs),
        )
        // Detached-download progress surface (page-reload-safe).
        // `list` returns every task in the in-process registry, `get`
        // is a single-snapshot polling fallback, `events` is the SSE
        // stream the UI subscribes to for live progress.
        .api_route(
            "/local-runtime/versions/downloads",
            get_with(version_handlers::list_active_downloads, version_handlers::list_active_downloads_docs),
        )
        .api_route(
            "/local-runtime/versions/downloads/{key}",
            get_with(version_handlers::get_download_snapshot, version_handlers::get_download_snapshot_docs),
        )
        .api_route(
            "/local-runtime/versions/downloads/{key}/events",
            get_with(version_handlers::subscribe_download_events, version_handlers::subscribe_download_events_docs),
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
        // Runtime singleton settings (P1.b)
        .api_route(
            "/local-runtime/settings",
            get_with(
                settings_handlers::get_runtime_settings,
                settings_handlers::get_runtime_settings_docs,
            ),
        )
        .api_route(
            "/local-runtime/settings",
            put_with(
                settings_handlers::update_runtime_settings,
                settings_handlers::update_runtime_settings_docs,
            ),
        )
        // Same-port reverse proxy at /api/local-llm/v1/* (P1.a + P1.e).
        // Merged here so it shares the module's auth + middleware stack.
        .merge(proxy_router())
}
