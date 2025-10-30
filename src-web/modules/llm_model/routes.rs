// LLM Model routes configuration
// Source: react-test/src-tauri/src/api/models.rs
// Following ziee-chat patterns from llm_provider module

use aide::axum::{routing::{delete_with, get_with, post_with}, ApiRouter};
use sqlx::PgPool;

use super::handlers::*;
use super::uploads;

/// LLM Model management routes
pub fn llm_model_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        // Model CRUD
        .api_route("/llm-models", get_with(list_models, list_models_docs))
        .api_route("/llm-models", post_with(create_model, create_model_docs))
        .api_route("/llm-models/{model_id}", get_with(get_model, get_model_docs))
        .api_route("/llm-models/{model_id}", post_with(update_model, update_model_docs))
        .api_route("/llm-models/{model_id}", delete_with(delete_model, delete_model_docs))
        // Model actions
        .api_route("/llm-models/{model_id}/enable", post_with(enable_model, enable_model_docs))
        .api_route("/llm-models/{model_id}/disable", post_with(disable_model, disable_model_docs))
        // File upload/download
        .api_route("/llm-models/upload", post_with(uploads::upload_multiple_files_and_commit, upload_files_docs))
        .api_route("/llm-models/download", post_with(uploads::initiate_repository_download, initiate_download_docs))
}
