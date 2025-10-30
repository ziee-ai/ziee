// LLM Model routes and handlers
// Source: react-test/src-tauri/src/api/models.rs
// Following ziee-chat patterns from llm_provider module

use aide::axum::{routing::{delete_with, get_with, post_with}, ApiRouter};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::r#type::{ApiResult, AppError},
    modules::permissions::{RequirePermissions, with_permission},
};

use super::{
    models::{CreateLlmModelRequest, ListModelsQuery, LlmModel, LlmModelListResponse, UpdateLlmModelRequest},
    permissions::*,
    repository,
    service,
    uploads,
};

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

// =====================================================
// Model CRUD Handlers
// =====================================================

/// List all LLM models with pagination and optional provider filtering
/// (requires llm_models::read permission)
async fn list_models(
    _auth: RequirePermissions<(LlmModelsRead,)>,
    Query(params): Query<ListModelsQuery>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<LlmModelListResponse>> {
    // Get models based on whether provider_id filter is provided
    let all_models = if let Some(provider_id) = params.provider_id {
        // Filter by provider
        repository::list_llm_models_by_provider(&pool, provider_id).await
    } else {
        // Get all models across all providers
        repository::list_all_llm_models(&pool).await
    }
    .map_err(|e| {
        eprintln!("Failed to get models: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Calculate pagination
    let total = all_models.len() as i64;
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(all_models.len());

    let paginated_models = if start < all_models.len() {
        all_models[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok((
        StatusCode::OK,
        Json(LlmModelListResponse {
            models: paginated_models,
            total,
            page: params.page as i32,
            per_page: params.per_page as i32,
        }),
    ))
}

fn list_models_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsRead,)>(op)
        .id("LlmModel.list")
        .tag("LLM Models")
        .summary("List LLM models with pagination and optional provider filtering")
        .description("List all LLM models. Optionally filter by provider_id query parameter.")
        .response::<200, Json<LlmModelListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM model by ID (requires llm_models::read permission)
async fn get_model(
    _auth: RequirePermissions<(LlmModelsRead,)>,
    Path(model_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<LlmModel>> {
    let model = repository::get_llm_model_by_id(&pool, model_id).await
        .map_err(|e| {
            eprintln!("Failed to get model {}: {}", model_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

fn get_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsRead,)>(op)
        .id("LlmModel.get")
        .tag("LLM Models")
        .summary("Get LLM model by ID")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Create a new LLM model (requires llm_models::create permission)
async fn create_model(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    State(pool): State<PgPool>,
    Json(request): Json<CreateLlmModelRequest>,
) -> ApiResult<Json<LlmModel>> {
    // Validate request
    service::validate_create_request(&request)?;

    // Create model
    let model = repository::create_llm_model(&pool, request).await
        .map_err(|e| {
            eprintln!("Failed to create model: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::CREATED, Json(model)))
}

fn create_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.create")
        .tag("LLM Models")
        .summary("Create a new LLM model")
        .response::<201, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update an existing LLM model (requires llm_models::edit permission)
async fn update_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(request): Json<UpdateLlmModelRequest>,
) -> ApiResult<Json<LlmModel>> {
    // Validate request
    service::validate_update_request(&request)?;

    // Update model
    let model = repository::update_llm_model(&pool, model_id, request).await
        .map_err(|e| {
            eprintln!("Failed to update model {}: {}", model_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

fn update_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.update")
        .tag("LLM Models")
        .summary("Update an existing LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Delete an LLM model (requires llm_models::delete permission)
async fn delete_model(
    _auth: RequirePermissions<(LlmModelsDelete,)>,
    Path(model_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<StatusCode> {
    let deleted = repository::delete_llm_model(&pool, model_id).await
        .map_err(|e| {
            eprintln!("Failed to delete model {}: {}", model_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    if !deleted {
        return Err(AppError::not_found("Model").to_api_error());
    }

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

fn delete_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsDelete,)>(op)
        .id("LlmModel.delete")
        .tag("LLM Models")
        .summary("Delete an LLM model")
        .response_with::<204, (), _>(|res| res.description("Model deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

// =====================================================
// Model Action Handlers
// =====================================================

/// Enable an LLM model (requires llm_models::edit permission)
async fn enable_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<LlmModel>> {
    let request = UpdateLlmModelRequest {
        enabled: Some(true),
        ..Default::default()
    };

    let model = repository::update_llm_model(&pool, model_id, request).await
        .map_err(|e| {
            eprintln!("Failed to enable model {}: {}", model_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

fn enable_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.enable")
        .tag("LLM Models")
        .summary("Enable an LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Disable an LLM model (requires llm_models::edit permission)
async fn disable_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<LlmModel>> {
    let request = UpdateLlmModelRequest {
        enabled: Some(false),
        ..Default::default()
    };

    let model = repository::update_llm_model(&pool, model_id, request).await
        .map_err(|e| {
            eprintln!("Failed to disable model {}: {}", model_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

fn disable_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.disable")
        .tag("LLM Models")
        .summary("Disable an LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

// =====================================================
// File Upload/Download Documentation
// =====================================================

fn upload_files_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.upload")
        .tag("LLM Models")
        .summary("Upload model files and create a new model")
        .description("Upload model weight files, config, and tokenizer files. The model is automatically created from the uploaded files.")
        .response::<200, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request or file validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

fn initiate_download_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.download")
        .tag("LLM Models")
        .summary("Initiate model download from a repository")
        .description("Start a background download task for a model from a Git repository (e.g., Hugging Face). Returns immediately with a download instance ID. The actual download happens in the background.")
        .response::<200, Json<super::models::DownloadInstance>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}
