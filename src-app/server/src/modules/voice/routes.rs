//! voice routes: transcribe (any transcribe user) + capability + admin settings,
//! plus the version + instance/model admin sub-routers.

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};
use axum::extract::DefaultBodyLimit;

use super::handlers;
use super::stream;
use super::transcribe;

/// Generous per-route ceiling for the transcribe upload (above the 32 MB default
/// logical cap). The handler enforces the dynamic `max_upload_bytes` from
/// settings; this just rejects absurd bodies before buffering.
const VOICE_TRANSCRIBE_BODY_LIMIT: usize = 64 * 1024 * 1024;

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
}
