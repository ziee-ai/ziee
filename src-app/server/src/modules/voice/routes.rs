//! voice routes: transcribe (any transcribe user) + capability + admin settings,
//! plus the version + instance/model admin sub-routers.

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};
use axum::extract::DefaultBodyLimit;

use super::handlers;
use super::model_handlers;
use super::stream;
use super::transcribe;

/// Generous per-route ceiling for the transcribe upload (above the 32 MB default
/// logical cap). The handler enforces the dynamic `max_upload_bytes` from
/// settings; this just rejects absurd bodies before buffering.
const VOICE_TRANSCRIBE_BODY_LIMIT: usize = 64 * 1024 * 1024;

/// Per-route ceiling for a model upload (5 GiB cap + slack). The handler enforces
/// the logical `VOICE_MODEL_MAX_UPLOAD_BYTES`; this rejects absurd bodies first.
const VOICE_MODEL_UPLOAD_BODY_LIMIT: usize = 5 * 1024 * 1024 * 1024 + 16 * 1024 * 1024;

/// Whisper-MODEL library sub-router (catalog / download / upload / installed set).
fn voice_model_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/voice/models",
            get_with(model_handlers::list_models, model_handlers::list_models_docs),
        )
        .api_route(
            "/voice/models/catalog",
            get_with(model_handlers::get_catalog, model_handlers::get_catalog_docs),
        )
        .api_route(
            "/voice/models/download",
            post_with(model_handlers::download_model, model_handlers::download_model_docs),
        )
        .api_route(
            "/voice/models/upload",
            post_with(model_handlers::upload_model, model_handlers::upload_model_docs)
                .layer(DefaultBodyLimit::max(VOICE_MODEL_UPLOAD_BODY_LIMIT)),
        )
        .api_route(
            "/voice/models/downloads",
            get_with(
                model_handlers::list_active_model_downloads,
                model_handlers::list_active_model_downloads_docs,
            ),
        )
        .api_route(
            "/voice/models/downloads/{key}",
            get_with(model_handlers::get_model_download, model_handlers::get_model_download_docs),
        )
        .api_route(
            "/voice/models/downloads/{key}/events",
            get_with(
                model_handlers::subscribe_model_download_events,
                model_handlers::subscribe_model_download_events_docs,
            ),
        )
        .api_route(
            "/voice/models/downloads/{key}/cancel",
            post_with(
                model_handlers::cancel_model_download,
                model_handlers::cancel_model_download_docs,
            ),
        )
        .api_route(
            "/voice/models/{id}/activate",
            post_with(model_handlers::activate_model, model_handlers::activate_model_docs),
        )
        .api_route(
            "/voice/models/{id}",
            aide::axum::routing::delete_with(
                model_handlers::delete_model,
                model_handlers::delete_model_docs,
            ),
        )
}

pub fn voice_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/voice/transcribe",
            post_with(transcribe::transcribe, transcribe::transcribe_docs)
                .layer(DefaultBodyLimit::max(VOICE_TRANSCRIBE_BODY_LIMIT)),
        )
        .api_route(
            "/voice/transcribe/stream",
            post_with(stream::transcribe_stream, stream::transcribe_stream_docs)
                .layer(DefaultBodyLimit::max(VOICE_TRANSCRIBE_BODY_LIMIT)),
        )
        .api_route(
            "/voice/capability",
            get_with(handlers::get_capability, handlers::get_capability_docs),
        )
        .api_route(
            "/voice/settings",
            get_with(handlers::get_settings, handlers::get_settings_docs)
                .put_with(handlers::update_settings, handlers::update_settings_docs),
        )
        .api_route(
            "/voice/versions/sync-cache",
            post_with(handlers::sync_cache, handlers::sync_cache_docs),
        )
        // Admin: whisper runtime version registry + install/update/delete.
        .merge(super::runtime_version::voice_version_router())
        // Admin: managed instance control + model status/download.
        .merge(super::instance_handlers::voice_instance_router())
        // Admin: whisper-model library (download / upload / installed set).
        .merge(voice_model_router())
}
