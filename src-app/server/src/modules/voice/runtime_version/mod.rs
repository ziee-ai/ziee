//! Whisper runtime version management: binary download + version registry.
//!
//! Single-engine analog of `llm_local_runtime::runtime_version`. The admin REST
//! surface lives under `/voice/versions/*`; merge [`voice_version_router`] into
//! the module's top-level router.

pub mod download_task;
pub mod handlers;
pub mod models;
pub mod repository;

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};

/// Admin REST router for whisper runtime versions.
pub fn voice_version_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/voice/versions",
            get_with(handlers::list_versions, handlers::list_versions_docs),
        )
        .api_route(
            "/voice/versions/check-updates",
            get_with(handlers::check_updates, handlers::check_updates_docs),
        )
        .api_route(
            "/voice/versions/download",
            post_with(handlers::download_version, handlers::download_version_docs),
        )
        .api_route(
            "/voice/versions/downloads",
            get_with(
                handlers::list_active_downloads,
                handlers::list_active_downloads_docs,
            ),
        )
        .api_route(
            "/voice/versions/downloads/{key}",
            get_with(
                handlers::get_download_snapshot,
                handlers::get_download_snapshot_docs,
            ),
        )
        .api_route(
            "/voice/versions/downloads/{key}/events",
            get_with(
                handlers::subscribe_download_events,
                handlers::subscribe_download_events_docs,
            ),
        )
        .api_route(
            "/voice/versions/{id}",
            get_with(handlers::get_version, handlers::get_version_docs)
                .delete_with(handlers::delete_version, handlers::delete_version_docs),
        )
        .api_route(
            "/voice/versions/{id}/set-default",
            post_with(handlers::set_default, handlers::set_default_docs),
        )
}
