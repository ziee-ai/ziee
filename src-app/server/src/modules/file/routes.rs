// File routes

use aide::axum::routing::{delete_with, get_with, post_with};
use aide::axum::ApiRouter;
use axum::extract::DefaultBodyLimit;

use super::handlers::*;

/// Per-route body limit for file uploads. The global router cap is 16 MB
/// (see main.rs); this route opts into a higher ceiling. Capped at 200 MB
/// (DoS hardening) — comfortably above the 50 MB per-file content cap
/// (`MAX_FILE_SIZE`) to leave headroom for multipart envelope/metadata.
/// Per-route body limit for the whole multipart upload request (approved
/// policy value: 200 MB). The global router cap is 16 MB (see main.rs); this
/// route opts into the higher ceiling. Individual files are additionally capped
/// at 50 MB by `MAX_FILE_SIZE` in the upload handler.
/// (see main.rs); this route opts into a higher ceiling. Set to 200 MB
/// (approved policy) so the request is rejected before buffering huge bodies
/// into RAM — paired with the 50 MB per-file cap enforced in upload.rs (the
/// extra headroom covers multipart framing + multiple fields).
const FILE_UPLOAD_BODY_LIMIT: usize = 200 * 1024 * 1024;

/// File management routes
pub fn file_router() -> ApiRouter {
    ApiRouter::new()
        // Upload — explicit higher body limit per 14-core-infrastructure F-01
        .api_route(
            "/files/upload",
            post_with(upload_file, upload_file_docs)
                .layer(DefaultBodyLimit::max(FILE_UPLOAD_BODY_LIMIT)),
        )
        // List files
        .api_route("/files", get_with(list_files, list_files_docs))
        // Binary endpoints (must come BEFORE /files/{file_id} to avoid route conflicts)
        .api_route("/files/{file_id}/preview", get_with(get_preview, get_preview_docs))
        .api_route("/files/{file_id}/thumbnail", get_with(get_thumbnail, get_thumbnail_docs))
        .api_route("/files/{file_id}/text", get_with(get_text_content, get_text_content_docs))
        .api_route("/files/{file_id}/download", get_with(download_file, download_file_docs))
        .api_route("/files/{file_id}/download-with-token", get_with(download_with_token, download_with_token_docs))
        // Version endpoints (also before /files/{file_id})
        .api_route("/files/{file_id}/versions", get_with(list_versions, list_versions_docs))
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
}
