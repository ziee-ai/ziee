// LLM Model routes configuration
// Source: react-test/src-tauri/src/api/models.rs
// Following ziee patterns from llm_provider module

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with},
};
use axum::extract::DefaultBodyLimit;

use super::handlers::*;

/// Per-route body limit for model uploads. Global router cap is 16 MB
/// (see main.rs); models can be many GB. Closes 14-core F-01.
const MODEL_UPLOAD_BODY_LIMIT: usize = 16 * 1024 * 1024 * 1024;

/// LLM Model management routes
pub fn llm_model_router() -> ApiRouter {
    ApiRouter::new()
        // Model CRUD
        .api_route("/llm-models", get_with(list_models, list_models_docs))
        .api_route("/llm-models", post_with(create_model, create_model_docs))
        .api_route(
            "/llm-models/{model_id}",
            get_with(get_model, get_model_docs),
        )
        .api_route(
            "/llm-models/{model_id}",
            post_with(update_model, update_model_docs),
        )
        .api_route(
            "/llm-models/{model_id}",
            delete_with(delete_model, delete_model_docs),
        )
        // Model actions
        .api_route(
            "/llm-models/{model_id}/enable",
            post_with(enable_model, enable_model_docs),
        )
        .api_route(
            "/llm-models/{model_id}/disable",
            post_with(disable_model, disable_model_docs),
        )
        // P1.k: manual (re-)validation trigger ("Run inference test")
        .api_route(
            "/llm-models/{model_id}/validate",
            post_with(validate_model, validate_model_docs),
        )
        // File upload/download — explicit per-route body limit per
        // 14-core-infrastructure F-01
        .api_route(
            "/llm-models/upload",
            post_with(upload_multiple_files_and_commit, upload_files_docs)
                .layer(DefaultBodyLimit::max(MODEL_UPLOAD_BODY_LIMIT)),
        )
        .api_route(
            "/llm-models/download",
            post_with(initiate_repository_download, initiate_download_docs),
        )
        // Pre-download file discovery (Hugging Face / GitHub auto-detect)
        .api_route(
            "/llm-models/repository-files",
            get_with(list_repository_files, list_repository_files_docs),
        )
        // Download management
        .api_route(
            "/llm-models/downloads",
            get_with(list_all_downloads, list_all_downloads_docs),
        )
        .api_route(
            "/llm-models/downloads/subscribe",
            get_with(
                subscribe_download_progress,
                subscribe_download_progress_docs,
            ),
        )
        .api_route(
            "/llm-models/downloads/{download_id}",
            get_with(get_download, get_download_docs),
        )
        .api_route(
            "/llm-models/downloads/{download_id}",
            delete_with(delete_download, delete_download_docs),
        )
        .api_route(
            "/llm-models/downloads/{download_id}/cancel",
            post_with(cancel_download, cancel_download_docs),
        )
}
