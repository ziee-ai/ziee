// File routes

use aide::axum::routing::{delete_with, get_with, post_with};
use aide::axum::ApiRouter;
use axum::extract::DefaultBodyLimit;

use super::deliverables::{
    list_deliverables, list_deliverables_docs, pin_deliverable, pin_deliverable_docs,
    unpin_deliverable, unpin_deliverable_docs,
};
use super::handlers::*;
use crate::core::app_state::file_upload_body_limit_bytes;

/// File management routes
pub fn file_router() -> ApiRouter {
    ApiRouter::new()
        // Upload — explicit higher body limit than the app-wide default (see
        // main.rs). Derived from the configurable per-file cap (`cap + slack`,
        // set at boot in `app_state`) so the request is rejected before
        // buffering an over-cap body into RAM; the slack covers multipart
        // framing + extra fields, keeping the raw body limit above the handler
        // cap so an over-cap file gets a clear FILE_TOO_LARGE (400), not a 413.
        .api_route(
            "/files/upload",
            post_with(upload_file, upload_file_docs)
                .layer(DefaultBodyLimit::max(file_upload_body_limit_bytes())),
        )
        // List files
        .api_route("/files", get_with(list_files, list_files_docs))
        // Binary endpoints (must come BEFORE /files/{file_id} to avoid route conflicts)
        .api_route("/files/{file_id}/preview", get_with(get_preview, get_preview_docs))
        .api_route("/files/{file_id}/raw", get_with(get_raw, get_raw_docs))
        .api_route("/files/{file_id}/thumbnail", get_with(get_thumbnail, get_thumbnail_docs))
        .api_route("/files/{file_id}/text", get_with(get_text_content, get_text_content_docs))
        .api_route("/files/{file_id}/text-rects", get_with(get_text_rects, get_text_rects_docs))
        .api_route("/files/{file_id}/download", get_with(download_file, download_file_docs))
        .api_route("/files/{file_id}/export", get_with(export_file, export_file_docs))
        .api_route("/files/{file_id}/download-with-token", get_with(download_with_token, download_with_token_docs))
        // Version endpoints (also before /files/{file_id})
        .api_route(
            "/files/{file_id}/versions",
            get_with(list_versions, list_versions_docs)
                .post_with(append_version, append_version_docs),
        )
        .api_route("/files/{file_id}/head", get_with(get_head_version, get_head_version_docs))
        .api_route("/files/{file_id}/versions/{version}", get_with(get_version, get_version_docs))
        .api_route("/files/{file_id}/versions/{version}/download", get_with(download_version, download_version_docs))
        .api_route("/files/{file_id}/versions/{version}/preview", get_with(preview_version, preview_version_docs))
        .api_route("/files/{file_id}/versions/{version}/text", get_with(text_version, text_version_docs))
        .api_route("/files/{file_id}/restore", post_with(restore_version, restore_version_docs))
        // Get file metadata
        .api_route("/files/{file_id}", get_with(get_file, get_file_docs))
        // Download token generation
        .api_route(
            "/files/{file_id}/download-token",
            post_with(generate_download_token, generate_download_token_docs),
        )
        // Delete
        .api_route(
            "/files/{file_id}",
            delete_with(delete_file, delete_file_docs),
        )
        // Conversation deliverables (derived model-authored files ∪ pinned − hidden)
        .api_route(
            "/conversations/{id}/deliverables",
            get_with(list_deliverables, list_deliverables_docs),
        )
        .api_route(
            "/conversations/{id}/deliverables/{file_id}",
            post_with(pin_deliverable, pin_deliverable_docs)
                .delete_with(unpin_deliverable, unpin_deliverable_docs),
        )
}

