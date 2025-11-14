// File routes

use aide::axum::routing::{delete_with, get_with, post_with};
use aide::axum::ApiRouter;
use axum::routing::get;

use super::handlers::*;

/// File management routes
pub fn file_router() -> ApiRouter {
    ApiRouter::new()
        // Upload
        .api_route("/files/upload", post_with(upload_file, upload_file_docs))
        // List & Get
        .api_route("/files", get_with(list_files, list_files_docs))
        .api_route("/files/{file_id}", get_with(get_file, get_file_docs))
        // Download (using plain axum routes for binary data)
        .route("/files/{file_id}/download", get(download_file))
        .api_route(
            "/files/{file_id}/download-token",
            post_with(generate_download_token, generate_download_token_docs),
        )
        .route("/files/{file_id}/download-with-token", get(download_with_token))
        // Preview & Content (using plain axum routes for binary data)
        .route("/files/{file_id}/preview", get(get_preview))
        .route("/files/{file_id}/text", get(get_text_content))
        // Delete
        .api_route(
            "/files/{file_id}",
            delete_with(delete_file, delete_file_docs),
        )
}
