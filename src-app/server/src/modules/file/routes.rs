// File routes

use aide::axum::routing::{delete_with, get_with, post_with};
use aide::axum::ApiRouter;

use super::handlers::*;

/// File management routes
pub fn file_router() -> ApiRouter {
    ApiRouter::new()
        // Upload
        .api_route("/files/upload", post_with(upload_file, upload_file_docs))
        // List files
        .api_route("/files", get_with(list_files, list_files_docs))
        // Binary endpoints (must come BEFORE /files/{file_id} to avoid route conflicts)
        .api_route("/files/{file_id}/preview", get_with(get_preview, get_preview_docs))
        .api_route("/files/{file_id}/thumbnail", get_with(get_thumbnail, get_thumbnail_docs))
        .api_route("/files/{file_id}/text", get_with(get_text_content, get_text_content_docs))
        .api_route("/files/{file_id}/download", get_with(download_file, download_file_docs))
        .api_route("/files/{file_id}/download-with-token", get_with(download_with_token, download_with_token_docs))
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
