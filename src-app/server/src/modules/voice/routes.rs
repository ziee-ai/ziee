//! voice routes: capability (any transcribe user) + admin settings REST.
//! Version / model / instance / transcribe routes are merged in from their
//! layers as they are built.

use aide::axum::{ApiRouter, routing::get_with};

use super::handlers;

pub fn voice_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/voice/capability",
            get_with(handlers::get_capability, handlers::get_capability_docs),
        )
        .api_route(
            "/voice/settings",
            get_with(handlers::get_settings, handlers::get_settings_docs)
                .put_with(handlers::update_settings, handlers::update_settings_docs),
        )
        // Admin: whisper runtime version registry + install/update/delete.
        .merge(super::runtime_version::voice_version_router())
        // Admin: managed instance control + model status/download.
        .merge(super::instance_handlers::voice_instance_router())
}
