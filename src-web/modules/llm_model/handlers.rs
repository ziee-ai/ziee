// LLM Model handlers
// Source: react-test/src-tauri/src/api/models.rs
// Following ziee-chat patterns from llm_provider module

use aide::transform::TransformOperation;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use crate::{
    common::r#type::{ApiResult, AppError},
    modules::permissions::{RequirePermissions, with_permission},
};

use super::{
    models::{DownloadInstance, LlmModel},
    permissions::*,
    repository::LlmModelRepository,
    utils,
    types::{CreateLlmModelRequest, ListModelsQuery, LlmModelListResponse, UpdateLlmModelRequest},
};

// =====================================================
// Model CRUD Handlers
// =====================================================

/// List all LLM models with pagination and optional provider filtering
/// (requires llm_models::read permission)
pub async fn list_models(
    _auth: RequirePermissions<(LlmModelsRead,)>,
    Query(params): Query<ListModelsQuery>,
    Extension(repo): Extension<LlmModelRepository>,
) -> ApiResult<Json<LlmModelListResponse>> {
    // Get models based on whether provider_id filter is provided
    let all_models = if let Some(provider_id) = params.provider_id {
        // Filter by provider
        repo.list_by_provider(provider_id).await?
    } else {
        // Get all models across all providers
        repo.list_all().await?
    };

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

pub fn list_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsRead,)>(op)
        .id("LlmModel.list")
        .tag("LLM Models")
        .summary("List LLM models with pagination and optional provider filtering")
        .description("List all LLM models. Optionally filter by provider_id query parameter.")
        .response::<200, Json<LlmModelListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM model by ID (requires llm_models::read permission)
pub async fn get_model(
    _auth: RequirePermissions<(LlmModelsRead,)>,
    Path(model_id): Path<Uuid>,
    Extension(repo): Extension<LlmModelRepository>,
) -> ApiResult<Json<LlmModel>> {
    let model = repo.get_by_id(model_id).await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

pub fn get_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsRead,)>(op)
        .id("LlmModel.get")
        .tag("LLM Models")
        .summary("Get LLM model by ID")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Create a new LLM model (requires llm_models::create permission)
pub async fn create_model(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    Extension(repo): Extension<LlmModelRepository>,
    Json(request): Json<CreateLlmModelRequest>,
) -> ApiResult<Json<LlmModel>> {
    // Validate request
    utils::validate_create_request(&request)?;

    // Create model
    let model = repo.create(request).await?;

    Ok((StatusCode::CREATED, Json(model)))
}

pub fn create_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.create")
        .tag("LLM Models")
        .summary("Create a new LLM model")
        .response::<201, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update an existing LLM model (requires llm_models::edit permission)
pub async fn update_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(repo): Extension<LlmModelRepository>,
    Json(request): Json<UpdateLlmModelRequest>,
) -> ApiResult<Json<LlmModel>> {
    // Validate request
    utils::validate_update_request(&request)?;

    // Update model
    let model = repo.update(model_id, request).await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

pub fn update_model_docs(op: TransformOperation) -> TransformOperation {
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
pub async fn delete_model(
    _auth: RequirePermissions<(LlmModelsDelete,)>,
    Path(model_id): Path<Uuid>,
    Extension(repo): Extension<LlmModelRepository>,
) -> ApiResult<StatusCode> {
    let deleted = repo.delete(model_id).await?;

    if !deleted {
        return Err(AppError::not_found("Model").to_api_error());
    }

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_model_docs(op: TransformOperation) -> TransformOperation {
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
pub async fn enable_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(repo): Extension<LlmModelRepository>,
) -> ApiResult<Json<LlmModel>> {
    let request = UpdateLlmModelRequest {
        enabled: Some(true),
        ..Default::default()
    };

    let model = repo.update(model_id, request).await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

pub fn enable_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.enable")
        .tag("LLM Models")
        .summary("Enable an LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Disable an LLM model (requires llm_models::edit permission)
pub async fn disable_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(repo): Extension<LlmModelRepository>,
) -> ApiResult<Json<LlmModel>> {
    let request = UpdateLlmModelRequest {
        enabled: Some(false),
        ..Default::default()
    };

    let model = repo.update(model_id, request).await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

pub fn disable_model_docs(op: TransformOperation) -> TransformOperation {
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

pub fn upload_files_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.upload")
        .tag("LLM Models")
        .summary("Upload model files and create a new model")
        .description("Upload model weight files, config, and tokenizer files. The model is automatically created from the uploaded files.")
        .response::<200, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request or file validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn initiate_download_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.download")
        .tag("LLM Models")
        .summary("Initiate model download from a repository")
        .description("Start a background download task for a model from a Git repository (e.g., Hugging Face). Returns immediately with a download instance ID. The actual download happens in the background.")
        .response::<200, Json<DownloadInstance>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}
